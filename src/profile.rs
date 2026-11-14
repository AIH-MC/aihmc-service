use serde_json::json;

use crate::{
    auth, db,
    error::{ApiError, ApiResult},
    state::AppState,
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
    if auth::check_name(state, username).await?.is_some() {
        return Err(ApiError::message(400, "改名失败，可能该玩家名已存在"));
    }
    db::execute(
        state,
        "UPDATE namelink SET username = ? WHERE uuid = ?",
        &[json!(username), json!(uuid)],
    )
    .await;
    Ok(())
}

pub async fn reset_name(state: &AppState, uuid: &str) -> ApiResult<()> {
    let rows = db::query(state, "SELECT * FROM users WHERE uuid = ?", &[json!(uuid)]).await;
    let row = rows
        .first()
        .ok_or_else(|| ApiError::internal("no user to reset"))?;
    db::execute(
        state,
        "UPDATE namelink SET username = ? WHERE uuid = ?",
        &[json!(db::text(row, "username")), json!(uuid)],
    )
    .await;
    Ok(())
}
