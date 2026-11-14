use rand::RngExt;
use sqlx::{MySql, Row, Transaction};

use crate::{
    auth,
    error::{ApiError, ApiResult},
    state::AppState,
};

pub fn validate_nickname(name: &str) -> ApiResult<()> {
    let valid = (1..=16).contains(&name.chars().count())
        && name.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '_' || ('\u{0080}'..='\u{d7ff}').contains(&c)
        });
    if valid {
        Ok(())
    } else {
        Err(ApiError::message(
            400,
            "昵称必须为 1–16 位字母、数字、下划线或支持的 Unicode 字符",
        ))
    }
}

fn random_name() -> String {
    let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    let suffix: String = (0..8)
        .map(|_| alphabet[rng.random_range(0..alphabet.len())] as char)
        .collect();
    format!("AIHMC_{suffix}")
}

async fn available(state: &AppState, uuid: &str, name: &str) -> ApiResult<bool> {
    let sql = "
        SELECT uuid FROM namelink WHERE username = ? AND (uuid IS NULL OR uuid <> ?)
        UNION ALL
        SELECT uuid FROM users WHERE username = ? AND (uuid IS NULL OR uuid <> ?)
        LIMIT 1
    ";
    let owner = sqlx::query(sql)
        .bind(name)
        .bind(uuid)
        .bind(name)
        .bind(uuid)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    if owner.is_some() {
        return Ok(false);
    }
    Ok(auth::check_name(state, name).await?.is_none())
}

fn database_error_is(error: &sqlx::Error, code: &str) -> bool {
    error
        .as_database_error()
        .and_then(|error| error.code())
        .as_deref()
        == Some(code)
}

async fn write_name(
    transaction: &mut Transaction<'_, MySql>,
    uuid: &str,
    name: &str,
) -> Result<(), sqlx::Error> {
    let existing = sqlx::query("SELECT uuid FROM namelink WHERE uuid = ? FOR UPDATE")
        .bind(uuid)
        .fetch_optional(&mut **transaction)
        .await?;
    if existing.is_some() {
        sqlx::query("UPDATE namelink SET username = ? WHERE uuid = ?")
            .bind(name)
            .bind(uuid)
            .execute(&mut **transaction)
            .await?;
    } else {
        sqlx::query("INSERT INTO namelink (uuid, username) VALUES (?, ?)")
            .bind(uuid)
            .bind(name)
            .execute(&mut **transaction)
            .await?;
    }
    Ok(())
}

pub async fn offline_name(
    state: &AppState,
    uuid: &str,
    requested: &str,
    persist: bool,
) -> ApiResult<String> {
    validate_nickname(requested)?;
    let mut candidate = requested.to_string();
    for _ in 0..32 {
        if !available(state, uuid, &candidate).await? {
            candidate = random_name();
            continue;
        }
        if !persist {
            return Ok(candidate);
        }
        let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
        match write_name(&mut transaction, uuid, &candidate).await {
            Ok(()) => {
                transaction.commit().await.map_err(ApiError::internal)?;
                return Ok(candidate);
            }
            Err(error) if database_error_is(&error, "1062") => {
                transaction.rollback().await.map_err(ApiError::internal)?;
                candidate = random_name();
            }
            Err(error) if database_error_is(&error, "1213") => {
                // InnoDB can choose this transaction as a deadlock victim even
                // when each account has its own UUID. Retry the same requested
                // name; a deadlock alone does not establish a name conflict.
                transaction.rollback().await.map_err(ApiError::internal)?;
            }
            Err(error) => return Err(ApiError::internal(error)),
        }
    }
    Err(ApiError::message(409, "暂时无法分配可用昵称，请重试"))
}

pub async fn online_name(state: &AppState, uuid: &str, source_name: &str) -> ApiResult<String> {
    let existing = sqlx::query("SELECT username FROM namelink WHERE uuid = ?")
        .bind(uuid)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    let display: String = existing
        .and_then(|row| row.get::<Option<String>, _>("username"))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| source_name.to_string());
    reserve_online_name(state, uuid, &display, true).await?;
    Ok(display)
}

pub async fn set_online_name(state: &AppState, uuid: &str, display: &str) -> ApiResult<()> {
    validate_nickname(display)?;
    reserve_online_name(state, uuid, display, false).await
}

async fn reserve_online_name(
    state: &AppState,
    uuid: &str,
    display: &str,
    create_identity_link: bool,
) -> ApiResult<()> {
    let mut replacement: Option<String> = None;
    for _ in 0..32 {
        // Only a displaced offline account needs a replacement. Resolve it
        // before opening the transaction so upstream I/O holds no row locks.
        if let Some(candidate) = &replacement
            && !available(state, uuid, candidate).await?
        {
            replacement = Some(random_name());
            continue;
        }
        let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
        let owner = sqlx::query("SELECT uuid FROM namelink WHERE username = ? FOR UPDATE")
            .bind(display)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(ApiError::internal)?;
        if let Some(owner) = owner {
            let owner_uuid: Option<String> = owner.get("uuid");
            if let Some(owner_uuid) = owner_uuid.filter(|owner| !owner.eq_ignore_ascii_case(uuid)) {
                let offline = sqlx::query("SELECT uuid FROM offline WHERE uuid = ?")
                    .bind(&owner_uuid)
                    .fetch_optional(&mut *transaction)
                    .await
                    .map_err(ApiError::internal)?;
                if offline.is_none() {
                    return Err(ApiError::message(409, "玩家名已被其他在线账户占用"));
                }
                let Some(replacement_name) = &replacement else {
                    transaction.rollback().await.map_err(ApiError::internal)?;
                    replacement = Some(random_name());
                    continue;
                };
                let result = sqlx::query("UPDATE namelink SET username = ? WHERE uuid = ?")
                    .bind(replacement_name)
                    .bind(&owner_uuid)
                    .execute(&mut *transaction)
                    .await;
                if let Err(error) = result {
                    if database_error_is(&error, "1062") {
                        transaction.rollback().await.map_err(ApiError::internal)?;
                        replacement = Some(random_name());
                        continue;
                    }
                    return Err(ApiError::internal(error));
                }
            }
        }
        match write_name(&mut transaction, uuid, display).await {
            Ok(()) => {
                if create_identity_link {
                    sqlx::query("INSERT IGNORE INTO userlink (uuid, new_uuid) VALUES (?, ?)")
                        .bind(uuid)
                        .bind(uuid)
                        .execute(&mut *transaction)
                        .await
                        .map_err(ApiError::internal)?;
                }
                transaction.commit().await.map_err(ApiError::internal)?;
                return Ok(());
            }
            Err(error) if database_error_is(&error, "1062") => {
                transaction.rollback().await.map_err(ApiError::internal)?;
            }
            Err(error) => return Err(ApiError::internal(error)),
        }
    }
    Err(ApiError::message(409, "暂时无法分配玩家名，请重试"))
}
