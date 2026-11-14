use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use sqlx::{MySql, MySqlConnection, MySqlPool, QueryBuilder};

#[derive(Debug)]
pub enum Error {
    Database(sqlx::Error),
    Timeout,
}

impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
pub type QueryFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

#[derive(Clone)]
pub struct Store {
    pool: Arc<MySqlPool>,
    timeout: Duration,
}

pub struct Queries<'a> {
    connection: &'a mut MySqlConnection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub uid: i64,
    pub name: String,
    pub profile_id: String,
    pub uuid: String,
    pub texture_data: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub uid: i64,
    pub name: String,
    pub access_token: String,
    pub client_token: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture {
    pub uid: i64,
    pub uuid: String,
    pub name: String,
    pub skin: String,
    pub model: String,
    pub cape: String,
    pub timestamp: i64,
}

const PROFILE_COLUMNS: &str = "COALESCE(uid,0), COALESCE(player_name,''), COALESCE(profile_id,''), COALESCE(player_uuid,''), COALESCE(texture_data,'')";
const TOKEN_COLUMNS: &str = "uid, player_name, access_token, client_token, create_time";
const TEXTURE_COLUMNS: &str = "uid, COALESCE(uuid,''), COALESCE(player_name,''), COALESCE(skin_hash,''), COALESCE(skin_type,''), COALESCE(cape_hash,''), COALESCE(timestamps,0)";

impl Store {
    pub fn new(pool: Arc<MySqlPool>, timeout: Duration) -> Self {
        Self { pool, timeout }
    }

    pub fn pool(&self) -> &MySqlPool {
        &self.pool
    }

    pub async fn read<T: Send>(
        &self,
        action: impl for<'a> FnOnce(Queries<'a>) -> QueryFuture<'a, T>,
    ) -> Result<T> {
        tokio::time::timeout(self.timeout, async {
            let mut connection = self.pool.acquire().await?;
            action(Queries {
                connection: &mut connection,
            })
            .await
        })
        .await
        .map_err(|_| Error::Timeout)?
    }

    pub async fn transaction<T: Send>(
        &self,
        action: impl for<'a> FnOnce(Queries<'a>) -> QueryFuture<'a, T>,
    ) -> Result<T> {
        tokio::time::timeout(self.timeout, async {
            let mut transaction = self.pool.begin().await?;
            let result = action(Queries {
                connection: &mut transaction,
            })
            .await;
            match result {
                Ok(value) => {
                    transaction.commit().await?;
                    Ok(value)
                }
                Err(error) => {
                    transaction.rollback().await?;
                    Err(error)
                }
            }
        })
        .await
        .map_err(|_| Error::Timeout)?
    }
}

impl Queries<'_> {
    pub async fn profile_by_uid(&mut self, uid: i64, lock: bool) -> Result<Option<Profile>> {
        let mut query = QueryBuilder::<MySql>::new("SELECT ");
        query
            .push(PROFILE_COLUMNS)
            .push(" FROM mcauth_profiles WHERE uid = ")
            .push_bind(uid);
        if lock {
            query.push(" FOR UPDATE");
        }
        let row = query
            .build_query_as::<(i64, String, String, String, String)>()
            .fetch_optional(&mut *self.connection)
            .await?;
        Ok(row.map(profile_from_row))
    }

    pub async fn profile(&mut self, uuid_or_name: &str) -> Result<Option<Profile>> {
        let mut query = QueryBuilder::<MySql>::new("SELECT ");
        query
            .push(PROFILE_COLUMNS)
            .push(" FROM mcauth_profiles WHERE player_uuid = ")
            .push_bind(uuid_or_name)
            .push(" OR player_name = ")
            .push_bind(uuid_or_name);
        let row = query
            .build_query_as::<(i64, String, String, String, String)>()
            .fetch_optional(&mut *self.connection)
            .await?;
        Ok(row.map(profile_from_row))
    }

    pub async fn profile_by_name(&mut self, name: &str) -> Result<Option<Profile>> {
        let mut query = QueryBuilder::<MySql>::new("SELECT ");
        query
            .push(PROFILE_COLUMNS)
            .push(" FROM mcauth_profiles WHERE player_name = ")
            .push_bind(name);
        let row = query
            .build_query_as::<(i64, String, String, String, String)>()
            .fetch_optional(&mut *self.connection)
            .await?;
        Ok(row.map(profile_from_row))
    }

    pub async fn profile_by_uuid(&mut self, uuid: &str) -> Result<Option<Profile>> {
        let mut query = QueryBuilder::<MySql>::new("SELECT ");
        query
            .push(PROFILE_COLUMNS)
            .push(" FROM mcauth_profiles WHERE player_uuid = ")
            .push_bind(uuid);
        let row = query
            .build_query_as::<(i64, String, String, String, String)>()
            .fetch_optional(&mut *self.connection)
            .await?;
        Ok(row.map(profile_from_row))
    }

    pub async fn token_by_uid(&mut self, uid: i64) -> Result<Option<Token>> {
        let mut query = QueryBuilder::<MySql>::new("SELECT ");
        query
            .push(TOKEN_COLUMNS)
            .push(" FROM mcauth_tokens WHERE uid = ")
            .push_bind(uid);
        let row = query
            .build_query_as::<(i64, String, String, String, i64)>()
            .fetch_optional(&mut *self.connection)
            .await?;
        Ok(row.map(token_from_row))
    }

    pub async fn token(&mut self, access: &str, client: &str, lock: bool) -> Result<Option<Token>> {
        let mut query = QueryBuilder::<MySql>::new("SELECT ");
        query
            .push(TOKEN_COLUMNS)
            .push(" FROM mcauth_tokens WHERE access_token = ")
            .push_bind(access)
            .push(" AND client_token = ")
            .push_bind(client);
        if lock {
            query.push(" FOR UPDATE");
        }
        let row = query
            .build_query_as::<(i64, String, String, String, i64)>()
            .fetch_optional(&mut *self.connection)
            .await?;
        Ok(row.map(token_from_row))
    }

    pub async fn token_by_access(&mut self, access: &str) -> Result<Option<Token>> {
        let mut query = QueryBuilder::<MySql>::new("SELECT ");
        query
            .push(TOKEN_COLUMNS)
            .push(" FROM mcauth_tokens WHERE access_token = ")
            .push_bind(access);
        let row = query
            .build_query_as::<(i64, String, String, String, i64)>()
            .fetch_optional(&mut *self.connection)
            .await?;
        Ok(row.map(token_from_row))
    }

    pub async fn bind_token(&mut self, token: &Token) -> Result<()> {
        if self.token_by_uid(token.uid).await?.is_some() {
            sqlx::query("UPDATE mcauth_tokens SET client_token = ?, create_time = ? WHERE uid = ?")
                .bind(&token.client_token)
                .bind(token.created_at)
                .bind(token.uid)
                .execute(&mut *self.connection)
                .await?;
        } else {
            sqlx::query(concat!(
                "INSERT INTO mcauth_tokens ",
                "(uid, player_name, access_token, client_token, create_time) ",
                "VALUES (?, ?, ?, ?, ?)",
            ))
            .bind(token.uid)
            .bind(&token.name)
            .bind(&token.access_token)
            .bind(&token.client_token)
            .bind(token.created_at)
            .execute(&mut *self.connection)
            .await?;
        }
        Ok(())
    }

    pub async fn save_profile(&mut self, profile: &Profile) -> Result<()> {
        if self.profile_by_uid(profile.uid, false).await?.is_some() {
            sqlx::query("UPDATE mcauth_profiles SET texture_data = ? WHERE uid = ?")
                .bind(&profile.texture_data)
                .bind(profile.uid)
                .execute(&mut *self.connection)
                .await?;
        } else {
            sqlx::query(concat!(
                "INSERT INTO mcauth_profiles ",
                "(uid, player_name, profile_id, player_uuid, texture_data) ",
                "VALUES (?, ?, ?, ?, ?)",
            ))
            .bind(profile.uid)
            .bind(&profile.name)
            .bind(&profile.profile_id)
            .bind(&profile.uuid)
            .bind(&profile.texture_data)
            .execute(&mut *self.connection)
            .await?;
        }
        Ok(())
    }

    pub async fn texture_by_uid(&mut self, uid: i64) -> Result<Option<Texture>> {
        let mut query = QueryBuilder::<MySql>::new("SELECT ");
        query
            .push(TEXTURE_COLUMNS)
            .push(" FROM mcauth_textures WHERE uid = ")
            .push_bind(uid);
        let row = query
            .build_query_as::<(i64, String, String, String, String, String, i64)>()
            .fetch_optional(&mut *self.connection)
            .await?;
        Ok(
            row.map(|(uid, uuid, name, skin, model, cape, timestamp)| Texture {
                uid,
                uuid,
                name,
                skin,
                model,
                cape,
                timestamp,
            }),
        )
    }

    pub async fn save_texture(&mut self, texture: &Texture) -> Result<()> {
        if self.texture_by_uid(texture.uid).await?.is_some() {
            sqlx::query(concat!(
                "UPDATE mcauth_textures SET player_name = ?, skin_hash = ?, ",
                "skin_type = ?, cape_hash = ?, timestamps = ? WHERE uid = ?",
            ))
            .bind(&texture.name)
            .bind(&texture.skin)
            .bind(&texture.model)
            .bind(&texture.cape)
            .bind(texture.timestamp)
            .bind(texture.uid)
            .execute(&mut *self.connection)
            .await?;
        } else {
            sqlx::query(concat!(
                "INSERT INTO mcauth_textures ",
                "(uid, uuid, player_name, skin_hash, skin_type, cape_hash, timestamps) ",
                "VALUES (?, ?, ?, ?, ?, ?, ?)",
            ))
            .bind(texture.uid)
            .bind(&texture.uuid)
            .bind(&texture.name)
            .bind(&texture.skin)
            .bind(&texture.model)
            .bind(&texture.cape)
            .bind(texture.timestamp)
            .execute(&mut *self.connection)
            .await?;
        }
        Ok(())
    }

    pub async fn refresh(&mut self, client: &str, access: &str) -> Result<Option<Token>> {
        // Preserve legacy clientToken-wide scope and the existing unique constraint.
        sqlx::query("UPDATE mcauth_tokens SET access_token = ? WHERE client_token = ?")
            .bind(access)
            .bind(client)
            .execute(&mut *self.connection)
            .await?;
        self.token(access, client, false).await
    }

    pub async fn invalidate(&mut self, access: &str, client: &str) -> Result<()> {
        sqlx::query("DELETE FROM mcauth_tokens WHERE access_token = ? AND client_token = ?")
            .bind(access)
            .bind(client)
            .execute(&mut *self.connection)
            .await?;
        Ok(())
    }

    pub async fn signout(&mut self, uid: i64) -> Result<()> {
        sqlx::query("DELETE FROM mcauth_tokens WHERE uid = ?")
            .bind(uid)
            .execute(&mut *self.connection)
            .await?;
        Ok(())
    }

    pub async fn record_join(
        &mut self,
        server: &str,
        access: &str,
        name: &str,
        ip: &str,
        timestamp: i64,
    ) -> Result<()> {
        sqlx::query(concat!(
            "INSERT INTO mcauth_logins ",
            "(server_id, access_token, playername, ip, timestamp) VALUES (?, ?, ?, ?, ?)",
        ))
        .bind(server)
        .bind(access)
        .bind(name)
        .bind(ip)
        .bind(timestamp)
        .execute(&mut *self.connection)
        .await?;
        Ok(())
    }

    pub async fn has_joined(&mut self, name: &str, server: &str) -> Result<bool> {
        Ok(sqlx::query_scalar::<_, i32>(
            "SELECT 1 FROM mcauth_logins WHERE playername = ? AND server_id = ?",
        )
        .bind(name)
        .bind(server)
        .fetch_optional(&mut *self.connection)
        .await?
        .is_some())
    }

    pub async fn delete_old_joins(&mut self, name: &str, before: i64) -> Result<()> {
        sqlx::query("DELETE FROM mcauth_logins WHERE timestamp < ? AND playername = ?")
            .bind(before)
            .bind(name)
            .execute(&mut *self.connection)
            .await?;
        Ok(())
    }

    pub async fn skin_domains(&mut self) -> Result<Vec<String>> {
        Ok(
            sqlx::query_scalar("SELECT skin_url FROM mcauth_skin_urls ORDER BY id")
                .fetch_all(&mut *self.connection)
                .await?,
        )
    }

    pub async fn skin_apis(&mut self) -> Result<Vec<String>> {
        Ok(
            sqlx::query_scalar("SELECT api_url FROM texture_apis ORDER BY id")
                .fetch_all(&mut *self.connection)
                .await?,
        )
    }

    pub async fn delete_user(&mut self, uid: i64) -> Result<bool> {
        if self.token_by_uid(uid).await?.is_none() {
            return Ok(false);
        }
        let name = self
            .texture_by_uid(uid)
            .await?
            .map(|texture| texture.name)
            .unwrap_or_default();
        sqlx::query("DELETE FROM mcauth_logins WHERE playername = ?")
            .bind(name)
            .execute(&mut *self.connection)
            .await?;
        for statement in [
            "DELETE FROM mcauth_profiles WHERE uid = ?",
            "DELETE FROM mcauth_tokens WHERE uid = ?",
            "DELETE FROM mcauth_textures WHERE uid = ?",
        ] {
            sqlx::query(statement)
                .bind(uid)
                .execute(&mut *self.connection)
                .await?;
        }
        Ok(true)
    }
}

fn profile_from_row(
    (uid, name, profile_id, uuid, texture_data): (i64, String, String, String, String),
) -> Profile {
    Profile {
        uid,
        name,
        profile_id,
        uuid,
        texture_data,
    }
}

fn token_from_row(
    (uid, name, access_token, client_token, created_at): (i64, String, String, String, i64),
) -> Token {
    Token {
        uid,
        name,
        access_token,
        client_token,
        created_at,
    }
}
