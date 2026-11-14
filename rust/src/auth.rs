use std::{future::Future, pin::Pin, sync::Arc};

use serde_json::{Value, json};

use crate::{
    config::RuntimeConfig,
    protocol::{self, Login, Profile, Property, User},
    store::{self, Store},
    ucenter,
};

#[derive(Debug)]
pub enum Error {
    Api {
        status: u16,
        name: &'static str,
        message: String,
    },
    Store(store::Error),
    Upstream,
    Protocol,
    Material,
}

impl Error {
    pub fn api(status: u16, name: &'static str, message: impl Into<String>) -> Self {
        Self::Api {
            status,
            name,
            message: message.into(),
        }
    }

    pub fn invalid_token() -> Self {
        Self::api(403, "ForbiddenOperationException", "Invalid token.")
    }
}

impl From<store::Error> for Error {
    fn from(error: store::Error) -> Self {
        Self::Store(error)
    }
}

impl From<ucenter::Error> for Error {
    fn from(error: ucenter::Error) -> Self {
        match error {
            ucenter::Error::InvalidCredentials => Self::api(
                403,
                "ForbiddenOperationException",
                "Invalid credentials. Invalid username or password.",
            ),
            ucenter::Error::Upstream => Self::Upstream,
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(_: serde_json::Error) -> Self {
        Self::Protocol
    }
}

impl From<rsa::Error> for Error {
    fn from(_: rsa::Error) -> Self {
        Self::Protocol
    }
}

impl From<rsa::rand_core::Error> for Error {
    fn from(_: rsa::rand_core::Error) -> Self {
        Self::Protocol
    }
}

pub struct Material {
    pub skin: String,
    pub model: String,
    pub cape: String,
}

pub type MaterialFuture<'a> = Pin<Box<dyn Future<Output = Result<Material, Error>> + Send + 'a>>;

pub trait MaterialProvider: Send + Sync {
    fn resolve<'a>(&'a self, name: &'a str) -> MaterialFuture<'a>;
}

pub struct Service {
    pub runtime: Arc<RuntimeConfig>,
    pub store: Store,
    users: ucenter::Client,
    materials: Arc<dyn MaterialProvider>,
}

impl Service {
    pub fn new(
        runtime: Arc<RuntimeConfig>,
        store: Store,
        users: ucenter::Client,
        materials: Arc<dyn MaterialProvider>,
    ) -> Self {
        Self {
            runtime,
            store,
            users,
            materials,
        }
    }

    pub async fn authenticate(
        &self,
        account: &str,
        password: &str,
        client: &str,
        agent: &str,
        request_user: bool,
    ) -> Result<Login, Error> {
        if account.is_empty() || password.is_empty() {
            return Err(Error::api(
                400,
                "HTTP/1.1 400 Bad Request",
                "账号或密码不能为空",
            ));
        }
        if !agent.eq_ignore_ascii_case("minecraft") {
            return Err(Error::api(403, "HTTP/1.1 403 Forbidden", "非法请求"));
        }
        let user = self.users.login(account, password).await?;
        let material = self.materials.resolve(&user.name).await?;
        let client = if client.is_empty() {
            protocol::client_token()?
        } else {
            client.to_owned()
        };
        let now = ucenter::unix_timestamp();
        // Only replay the same local transaction after the observed legacy deadlock.
        // Upstream login and material requests are completed once, outside retries.
        tokio::time::timeout(self.runtime.config.database.query_timeout.0, async {
            for attempt in 0..3 {
                let user_name = user.name.clone();
                let email = user.email.clone();
                let client = client.clone();
                let skin = material.skin.clone();
                let model = material.model.clone();
                let cape = material.cape.clone();
                let base_url = self.runtime.config.textures.public_base_url.clone();
                let result = self
                    .store
                    .transaction::<_, Error>(move |mut q| {
                        Box::pin(async move {
                            let mut profile = match q.profile_by_uid(user.uid, true).await? {
                                Some(profile) => profile,
                                None => store::Profile {
                                    uid: user.uid,
                                    name: user_name.clone(),
                                    uuid: protocol::new_uuid()?,
                                    profile_id: protocol::profile_id(&user_name, &email, user.uid),
                                    texture_data: String::new(),
                                },
                            };
                            q.bind_token(&store::Token {
                                uid: user.uid,
                                name: user_name.clone(),
                                access_token: protocol::access_token(&user_name, &client, now),
                                client_token: client,
                                created_at: now,
                            })
                            .await?;
                            q.save_texture(&store::Texture {
                                uid: user.uid,
                                uuid: profile.uuid.clone(),
                                name: user_name,
                                skin,
                                model,
                                cape,
                                timestamp: now,
                            })
                            .await?;
                            let texture =
                                q.texture_by_uid(user.uid).await?.ok_or(Error::Protocol)?;
                            profile.texture_data = protocol::encode_texture(
                                texture.timestamp,
                                &texture.uuid,
                                &texture.name,
                                &texture.skin,
                                &texture.model,
                                &texture.cape,
                                &base_url,
                            )?;
                            q.save_profile(&profile).await?;
                            let token = q.token_by_uid(user.uid).await?.ok_or(Error::Protocol)?;
                            let selected = Profile {
                                id: profile.uuid,
                                name: profile.name,
                                properties: vec![],
                            };
                            Ok(Login {
                                access_token: token.access_token,
                                client_token: token.client_token,
                                available_profiles: vec![selected.clone()],
                                selected_profile: Some(selected),
                                user: request_user.then_some(User {
                                    id: profile.profile_id,
                                    properties: vec![],
                                }),
                            })
                        })
                    })
                    .await;
                let deadlock = match &result {
                    Err(Error::Store(store::Error::Database(sqlx::Error::Database(error)))) => {
                        error
                            .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
                            .is_some_and(|error| error.number() == 1213)
                    }
                    _ => false,
                };
                if !deadlock || attempt == 2 {
                    return result;
                }
            }
            unreachable!("bounded transaction retry always returns")
        })
        .await
        .map_err(|_| Error::Store(store::Error::Timeout))?
    }

    pub async fn refresh(
        &self,
        access: String,
        client: String,
        request_user: bool,
    ) -> Result<Login, Error> {
        self.store
            .transaction(move |mut q| {
                Box::pin(async move {
                    let token = q
                        .token(&access, &client, false)
                        .await?
                        .ok_or_else(Error::invalid_token)?;
                    let mut profile = q
                        .profile_by_uid(token.uid, true)
                        .await?
                        .ok_or(Error::Protocol)?;
                    q.token(&access, &client, true)
                        .await?
                        .ok_or_else(Error::invalid_token)?;
                    let token = q
                        .refresh(
                            &client,
                            &protocol::refreshed_token(&client, ucenter::unix_timestamp()),
                        )
                        .await?
                        .ok_or(Error::Protocol)?;
                    if token.uid != profile.uid {
                        profile = q
                            .profile_by_uid(token.uid, false)
                            .await?
                            .ok_or(Error::Protocol)?;
                    }
                    let selected = Profile {
                        id: profile.uuid,
                        name: profile.name,
                        properties: vec![Property {
                            name: "textures".into(),
                            value: profile.texture_data,
                            signature: String::new(),
                        }],
                    };
                    Ok(Login {
                        access_token: token.access_token,
                        client_token: token.client_token,
                        available_profiles: vec![],
                        selected_profile: Some(selected),
                        user: request_user.then_some(User {
                            id: profile.profile_id,
                            properties: vec![],
                        }),
                    })
                })
            })
            .await
    }

    pub async fn validate(&self, access: String, client: String) -> Result<(), Error> {
        let token = self
            .store
            .read(move |mut q| Box::pin(async move { q.token(&access, &client, false).await }))
            .await?;
        token.ok_or_else(Error::invalid_token)?;
        Ok(())
    }

    pub async fn invalidate(&self, access: String, client: String) -> Result<(), Error> {
        self.store
            .read(move |mut q| Box::pin(async move { q.invalidate(&access, &client).await }))
            .await?;
        Ok(())
    }

    pub async fn signout(&self, account: &str, password: &str) -> Result<(), Error> {
        let user = self.users.login(account, password).await?;
        self.store
            .read(move |mut q| Box::pin(async move { q.signout(user.uid).await }))
            .await?;
        Ok(())
    }

    pub async fn profile(
        &self,
        uuid_or_name: String,
        unsigned: &str,
    ) -> Result<Option<Profile>, Error> {
        let profile = self
            .store
            .read(move |mut q| Box::pin(async move { q.profile(&uuid_or_name).await }))
            .await?;
        let Some(profile) = profile else {
            return Ok(None);
        };
        let signature = if protocol::php_truthy_query(unsigned) {
            String::new()
        } else {
            protocol::sign(&profile.texture_data, &self.runtime.private_key)?
        };
        Ok(Some(Profile {
            id: profile.uuid,
            name: profile.name,
            properties: vec![Property {
                name: "textures".into(),
                value: profile.texture_data,
                signature,
            }],
        }))
    }

    pub async fn name(&self, name: String) -> Result<Profile, Error> {
        let lookup = name.clone();
        let profile = self
            .store
            .read(move |mut q| Box::pin(async move { q.profile(&lookup).await }))
            .await?;
        let profile = profile.ok_or_else(|| {
            Error::api(
                404,
                "UserNotFound",
                format!("The requested user {name} could not be found"),
            )
        })?;
        Ok(Profile {
            id: profile.uuid,
            name: profile.name,
            properties: vec![],
        })
    }

    pub async fn profiles(&self, names: Vec<String>) -> Result<Vec<Profile>, Error> {
        let mut profiles = vec![];
        for name in names {
            let lookup = name.clone();
            let profile = self
                .store
                .read(move |mut q| Box::pin(async move { q.profile_by_name(&lookup).await }))
                .await?;
            if let Some(profile) = profile {
                profiles.push(Profile {
                    id: profile.uuid,
                    name,
                    properties: vec![],
                });
            }
        }
        Ok(profiles)
    }

    pub async fn join(
        &self,
        server_id: String,
        access: String,
        selected: String,
        ip: String,
    ) -> Result<(), Error> {
        self.store
            .transaction(move |mut q| {
                Box::pin(async move {
                    let token = q
                        .token_by_access(&access)
                        .await?
                        .ok_or_else(Error::invalid_token)?;
                    let profile = q
                        .profile_by_uid(token.uid, false)
                        .await?
                        .ok_or(Error::Protocol)?;
                    if selected != profile.uuid {
                        return Err(Error::invalid_token());
                    }
                    q.record_join(
                        &server_id,
                        &access,
                        &profile.name,
                        &ip,
                        ucenter::unix_timestamp(),
                    )
                    .await?;
                    Ok(())
                })
            })
            .await
    }

    pub async fn has_joined(
        &self,
        name: String,
        server_id: String,
    ) -> Result<Option<Profile>, Error> {
        let lookup = name.clone();
        let joined = self
            .store
            .read(move |mut q| Box::pin(async move { q.has_joined(&lookup, &server_id).await }))
            .await?;
        if !joined {
            return Ok(None);
        }
        let profile = self.profile(name.clone(), "").await?;
        if profile.is_none() {
            return Ok(None);
        }
        let before = ucenter::unix_timestamp()
            - self.runtime.config.auth.login_record_retention.0.as_secs() as i64;
        self.store
            .read(move |mut q| Box::pin(async move { q.delete_old_joins(&name, before).await }))
            .await?;
        Ok(profile)
    }

    pub async fn metadata(&self) -> Result<Value, Error> {
        let domains = self
            .store
            .read(|mut q| Box::pin(async move { q.skin_domains().await }))
            .await?;
        let site = &self.runtime.config.site;
        Ok(json!({
            "meta": {
                "serverName": site.name, "implementationName": site.implementation_name,
                "implementationVersion": site.implementation_version,
                "links": { "homepage": site.homepage, "register": site.register_url },
                "feature.non_email_login": site.allow_non_email_login,
            },
            "skinDomains": domains, "signaturePublickey": self.runtime.public_key_pem,
        }))
    }

    pub async fn delete_user(&self, key: &str, uid: i64) -> Result<(u16, &'static str), Error> {
        if !protocol::php_truthy_query(key) || uid == 0 {
            return Ok((400, "超级操作密钥或用户UID不能为空"));
        }
        if key != self.runtime.super_key {
            return Ok((403, "超级操作密钥错误"));
        }
        let deleted = self
            .store
            .transaction::<_, Error>(move |mut queries| {
                Box::pin(async move { Ok(queries.delete_user(uid).await?) })
            })
            .await?;
        if !deleted {
            return Ok((404, "用户不存在，或未登录过 Minecraft 服务器"));
        }
        Ok((200, "删除成功"))
    }
}
