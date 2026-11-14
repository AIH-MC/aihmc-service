use serde_json::{Value, json};

use crate::{
    db,
    error::{ApiError, ApiResult},
    names,
    state::AppState,
    upstream,
};

pub async fn metadata(state: &AppState) -> ApiResult<Value> {
    let key = tokio::fs::read_to_string(state.resource(&state.config.rsakey.public_keyfile))
        .await
        .map_err(ApiError::internal)?
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut domains = state.config.extra_skin_domains.clone();
    for source in &state.config.servers {
        if source.enabled == Some(true) {
            let (_, domain) = upstream::domain(source)?;
            domains.push(format!(".{domain}"));
            domains.push(domain);
        }
    }
    Ok(
        json!({"meta": {"implementationName": "aihmc-auth-server", "implementationVersion": "1.0.0", "serverName": "AIHMC Auth"}, "skinDomains": domains, "signaturePublickey": key}),
    )
}

pub async fn save_player(
    state: &AppState,
    profile: &Value,
    source: &str,
    ip: Option<&str>,
) -> ApiResult<()> {
    let uuid = profile["id"]
        .as_str()
        .ok_or_else(|| ApiError::internal("profile has no id"))?;
    let name = profile["name"]
        .as_str()
        .ok_or_else(|| ApiError::internal("profile has no name"))?;
    let final_name = names::online_name(state, uuid, name).await?;
    if let Some(texture) = profile["properties"]
        .as_array()
        .and_then(|properties| properties.iter().find(|p| p["name"] == "textures"))
    {
        let sql = "
            INSERT INTO users
                (uuid, username, last_source, last_ip, textures_value, textures_signature)
            VALUES (?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE username=?, last_source=?, last_ip=?,
                textures_value=?, textures_signature=?
        ";
        db::execute(
            state,
            sql,
            &[
                json!(uuid),
                json!(final_name),
                json!(source),
                json!(ip),
                texture["value"].clone(),
                texture["signature"].clone(),
                json!(final_name),
                json!(source),
                json!(ip),
                texture["value"].clone(),
                texture["signature"].clone(),
            ],
        )
        .await;
    }
    Ok(())
}

pub async fn final_profile(state: &AppState, profile: Value) -> Value {
    let sql = "
        SELECT u.*, l.new_uuid, n.username AS linked_name
        FROM users u
        LEFT JOIN userlink l ON u.uuid = l.uuid
        LEFT JOIN namelink n ON n.uuid = COALESCE(l.new_uuid, u.uuid)
        WHERE u.uuid = ?
    ";
    let rows = db::query(state, sql, &[profile["id"].clone()]).await;
    if let Some(row) = rows.first() {
        json!({
            "id": db::text(row, "new_uuid").filter(|s| !s.is_empty()).or_else(|| db::text(row, "uuid")),
            "name": db::text(row, "linked_name").filter(|s| !s.is_empty()).or_else(|| db::text(row, "username")),
            "properties": [{"name": "textures", "value": db::text(row, "textures_value"), "signature": db::text(row, "textures_signature")}],
        })
    } else {
        profile
    }
}

pub async fn has_joined(
    state: &AppState,
    name: &str,
    server_id: &str,
    ip: Option<&str>,
) -> ApiResult<Option<Value>> {
    for source in state.config.servers.iter().filter(|s| s.is_enabled()) {
        if let Some(profile) = upstream::join(state, source, name, server_id, ip).await? {
            save_player(state, &profile, &source.name, ip).await?;
            return Ok(Some(final_profile(state, profile).await));
        }
    }
    Ok(None)
}

pub async fn profile(state: &AppState, uuid: &str) -> ApiResult<Option<Value>> {
    for source in state.config.servers.iter().filter(|s| s.is_enabled()) {
        let url = format!("{}/{uuid}", upstream::endpoint(source, "profile")?);
        if let Some(profile) = upstream::get_json(state, &url).await? {
            return Ok(Some(final_profile(state, profile).await));
        }
    }
    Ok(None)
}

pub async fn check_name(state: &AppState, username: &str) -> ApiResult<Option<Value>> {
    for source in state.config.servers.iter().filter(|s| s.is_enabled()) {
        let url = format!("{}/{username}", upstream::endpoint(source, "name")?);
        if let Some(mut profile) = upstream::get_json(state, &url).await? {
            profile["source"] = json!(source.name);
            return Ok(Some(profile));
        }
    }
    Ok(None)
}
