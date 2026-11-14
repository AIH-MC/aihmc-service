use serde_json::json;
use sqlx::Row;

use crate::{
    auth, db,
    error::{ApiError, ApiResult},
    names,
    state::AppState,
    upstream,
};

pub async fn link(state: &AppState, uuid: &str, new_uuid: &str) -> ApiResult<()> {
    if !db::query(
        state,
        "SELECT * FROM userlink WHERE uuid = ?",
        &[json!(uuid)],
    )
    .await
    .is_empty()
    {
        db::execute(
            state,
            "UPDATE userlink SET new_uuid = ? WHERE uuid = ?",
            &[json!(new_uuid), json!(uuid)],
        )
        .await;
    }
    // Preserve the characterized legacy HTTP response after the write.
    Err(ApiError::message(400, "数据迁移失败，可能该玩家没有进过服"))
}

pub async fn rename(state: &AppState, uuid: &str, username: &str) -> ApiResult<()> {
    let uuid = uuid.replace('-', "");
    let offline = sqlx::query("SELECT uuid FROM offline WHERE uuid = ?")
        .bind(&uuid)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    if offline.is_some() {
        names::offline_name(state, &uuid, username, true).await?;
        return Ok(());
    }
    if state.config.check_auth_source_names && auth::check_name(state, username).await?.is_some() {
        return Err(ApiError::message(400, "改名失败，可能该玩家名已存在"));
    }
    let account = sqlx::query(
        "SELECT uuid FROM users WHERE uuid = ?
         UNION ALL SELECT uuid FROM userlink WHERE new_uuid = ? LIMIT 1",
    )
    .bind(&uuid)
    .bind(&uuid)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    if account.is_none() {
        return Err(ApiError::message(404, "没有可改名的认证账户"));
    }
    names::set_online_name(state, &uuid, username).await
}

pub async fn reset_name(state: &AppState, uuid: &str) -> ApiResult<()> {
    let uuid = uuid.replace('-', "");
    let offline = sqlx::query("SELECT username FROM offline WHERE uuid = ?")
        .bind(&uuid)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    // The player's effective UUID may differ from the authentication source UUID.
    let rows = sqlx::query(
        "SELECT u.uuid, u.last_source, COALESCE(NULLIF(l.new_uuid, ''), u.uuid) AS target_uuid
         FROM users u LEFT JOIN userlink l ON l.uuid = u.uuid
         WHERE (u.uuid = ? OR l.new_uuid = ?)
         AND (u.last_source IS NOT NULL OR NOT EXISTS
              (SELECT 1 FROM offline o WHERE o.uuid = u.uuid))",
    )
    .bind(&uuid)
    .bind(&uuid)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    if rows.is_empty()
        && let Some(account) = offline
    {
        let username: String = account.get("username");
        names::offline_name(state, &uuid, &username, true).await?;
        return Ok(());
    }
    if rows.len() > 1 {
        return Err(ApiError::message(
            409,
            "多个认证账户关联此 UUID，无法确定要重置的账户",
        ));
    }
    let row = rows
        .first()
        .ok_or_else(|| ApiError::message(404, "没有可重置的认证账户"))?;
    let source_uuid: String = row.get("uuid");
    let target_uuid: String = row.get("target_uuid");
    let source_name: Option<String> = row.get("last_source");
    // Read the raw upstream profile, never the locally renamed final_profile.
    for source in state.config.servers.iter().filter(|source| {
        source.is_enabled() && source_name.as_ref().is_none_or(|name| name == &source.name)
    }) {
        let url = format!("{}/{}", upstream::endpoint(source, "profile")?, source_uuid);
        if let Some(profile) = upstream::get_json(state, &url).await? {
            let id = profile["id"].as_str().unwrap_or_default().replace('-', "");
            if !id.eq_ignore_ascii_case(&source_uuid) {
                return Err(ApiError::message(502, "认证源返回了不匹配的 UUID"));
            }
            let name = profile["name"]
                .as_str()
                .ok_or_else(|| ApiError::internal("upstream profile has no name"))?;
            return names::set_online_name(state, &target_uuid, name).await;
        }
    }
    Err(ApiError::message(
        502,
        "无法从认证源取得原名，未修改当前昵称",
    ))
}
