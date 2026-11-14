use serde_json::{Value, json};
use sqlx::Row;

use crate::{
    db,
    error::{ApiError, ApiResult},
    names,
    state::AppState,
    utils,
};

pub async fn register(
    state: &AppState,
    username: &str,
    password: &str,
    confirmation: &str,
    ip: Option<&str>,
    nickname: Option<&str>,
) -> ApiResult<String> {
    if !state.config.allow_offline {
        return Err(ApiError::message(400, "注册失败，已关闭离线登录注册"));
    }
    if password != confirmation {
        return Err(ApiError::message(400, "密码和确认密码不一致"));
    }
    let nickname = nickname.unwrap_or(username);
    names::validate_nickname(nickname)?;
    let uuid = utils::offline_uuid(username);
    let hash = utils::hash_password(password).map_err(ApiError::internal)?;
    if !db::execute(
        state,
        "INSERT INTO offline (username, uuid, password, ip) VALUES (?, ?, ?, ?)",
        &[json!(username), json!(uuid), json!(hash), json!(ip)],
    )
    .await
    {
        return Err(ApiError::message(
            400,
            format!("注册失败，玩家 {username} 已存在"),
        ));
    }
    sqlx::query("INSERT IGNORE INTO userlink (uuid, new_uuid) VALUES (?, ?)")
        .bind(&uuid)
        .bind(&uuid)
        .execute(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    names::offline_name(state, &uuid, nickname, true).await?;
    Ok("注册成功".into())
}

pub async fn profile(state: &AppState, username: &str) -> ApiResult<Value> {
    let account = sqlx::query("SELECT username, uuid FROM offline WHERE username = ?")
        .bind(username)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let registered = account.is_some();
    let (username, uuid) = match account {
        Some(row) => {
            let username: String = row.get("username");
            let uuid: Option<String> = row.get("uuid");
            (
                username,
                uuid.ok_or_else(|| ApiError::internal("offline account has no stored UUID"))?,
            )
        }
        None => (username.to_string(), utils::offline_uuid(username)),
    };
    let existing = sqlx::query("SELECT username FROM namelink WHERE uuid = ?")
        .bind(&uuid)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let requested = existing
        .and_then(|row| row.get::<Option<String>, _>("username"))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| username.clone());
    let nickname = names::offline_name(state, &uuid, &requested, registered).await?;
    Ok(
        json!({"id": uuid, "name": nickname, "username": username, "registered": registered, "properties": []}),
    )
}

pub async fn login(state: &AppState, username: &str, password: &str) -> ApiResult<String> {
    if !state.config.allow_offline {
        return Err(ApiError::message(400, "登录失败，已关闭离线登录注册"));
    }
    let rows = db::query(
        state,
        "SELECT * FROM offline WHERE username = ?",
        &[json!(username)],
    )
    .await;
    let row = rows.first().ok_or_else(|| {
        ApiError::message(404, format!("登录失败，玩家 {username} 不存在，请先注册！"))
    })?;
    let hash =
        db::text(row, "password").ok_or_else(|| ApiError::internal("missing password hash"))?;
    if !utils::check_password(password, &hash).map_err(ApiError::internal)? {
        return Err(ApiError::message(403, "密码错误！"));
    }
    Ok("登录成功！".into())
}

pub async fn check(state: &AppState, username: &str) -> ApiResult<String> {
    if !state.config.allow_offline {
        return Err(ApiError::message(400, "查询失败，已关闭离线登录注册"));
    }
    if db::query(
        state,
        "SELECT * FROM offline WHERE username = ?",
        &[json!(username)],
    )
    .await
    .is_empty()
    {
        return Err(ApiError::message(
            404,
            format!("玩家 {username} 尚未注册，请先注册！"),
        ));
    }
    Ok(format!("玩家 {username} 已注册，请登录！"))
}

pub async fn change_password(
    state: &AppState,
    username: &str,
    password: &str,
    new_password: &str,
) -> ApiResult<String> {
    login(state, username, password).await?;
    let hash = utils::hash_password(new_password).map_err(ApiError::internal)?;
    db::execute(
        state,
        "UPDATE offline SET password = ? WHERE username = ?",
        &[json!(hash), json!(username)],
    )
    .await;
    Ok("密码修改成功！".into())
}
