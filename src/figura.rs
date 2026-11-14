use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use sqlx::Row;

use crate::{
    db,
    error::{ApiError, ApiResult},
    state::AppState,
    upstream, utils,
};

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub async fn has_joined(
    state: &AppState,
    username: &str,
    server_id: &str,
    ip: Option<&str>,
) -> ApiResult<Option<Value>> {
    for source in state.config.servers.iter().filter(|s| s.is_enabled()) {
        if let Some(profile) = upstream::join(state, source, username, server_id, None).await? {
            return Ok(Some(profile));
        }
    }
    let synced = db::query(
        state,
        "SELECT uuid, username FROM figura_server WHERE username = ?",
        &[json!(username)],
    )
    .await;
    if let Some(row) = synced.first() {
        return Ok(Some(
            json!({"id": db::text(row, "uuid"), "name": username, "properties": []}),
        ));
    }
    let sessions = db::query(
        state,
        "SELECT username, ip, last_time FROM figura WHERE username = ?",
        &[json!(username)],
    )
    .await;
    if let Some(row) = sessions.first() {
        let session_ip = db::text(row, "ip");
        let last_time: Option<i64> = row.get("last_time");
        let last_time = last_time.ok_or_else(|| ApiError::internal("missing Figura timestamp"))?;
        if session_ip.as_deref() != ip
            || now() - last_time > utils::parse_duration(&state.config.figura_session) as i64
        {
            return Ok(None);
        }
        db::execute(
            state,
            "UPDATE figura SET ip = ?, last_time = ? WHERE username = ?",
            &[json!(ip), json!(now()), json!(username)],
        )
        .await;
    }
    Ok(Some(
        json!({"id": utils::offline_uuid(username), "name": username, "properties": []}),
    ))
}

pub async fn register(
    state: &AppState,
    username: &str,
    password: &str,
    confirmation: &str,
    ip: &str,
) -> ApiResult<()> {
    if password != confirmation {
        return Err(ApiError::message(400, "密码和确认密码不一致"));
    }
    let hash = utils::hash_password(password).map_err(ApiError::internal)?;
    if !db::execute(
        state,
        "INSERT INTO figura (username, uuid, password, ip, last_time) VALUES (?, ?, ?, ?, ?)",
        &[
            json!(username),
            json!(utils::offline_uuid(username)),
            json!(hash),
            json!(ip),
            json!(now()),
        ],
    )
    .await
    {
        return Err(ApiError::message(
            400,
            format!("注册失败，玩家 {username} 已存在"),
        ));
    }
    Ok(())
}

pub async fn refresh(state: &AppState, username: &str, password: &str, ip: &str) -> ApiResult<()> {
    let rows = db::query(
        state,
        "SELECT * FROM figura WHERE username = ?",
        &[json!(username)],
    )
    .await;
    let row = rows.first().ok_or_else(|| {
        ApiError::message(404, format!("刷新失败，玩家 {username} 不存在，请先注册！"))
    })?;
    let hash =
        db::text(row, "password").ok_or_else(|| ApiError::internal("missing password hash"))?;
    if !utils::check_password(password, &hash).map_err(ApiError::internal)? {
        return Err(ApiError::message(403, "密码错误！"));
    }
    db::execute(
        state,
        "UPDATE figura SET ip = ?, last_time = ? WHERE username = ?",
        &[json!(ip), json!(now()), json!(username)],
    )
    .await;
    Ok(())
}

pub async fn sync(state: &AppState, uuid: &str, username: &str) -> String {
    let uuid = uuid.replace('-', "");
    let rows = db::query(
        state,
        "SELECT uuid FROM figura_server WHERE uuid = ?",
        &[json!(uuid)],
    )
    .await;
    if rows.is_empty() {
        db::execute(
            state,
            "INSERT INTO figura_server (uuid, username) VALUES (?, ?)",
            &[json!(uuid), json!(username)],
        )
        .await;
        "写入成功".into()
    } else {
        db::execute(
            state,
            "UPDATE figura_server SET username = ? WHERE uuid = ?",
            &[json!(username), json!(uuid)],
        )
        .await;
        "更新成功".into()
    }
}
