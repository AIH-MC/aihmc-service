pub mod image;

use std::{
    collections::HashMap,
    future::Future,
    io::Write,
    path::{Component, Path},
    pin::Pin,
    sync::{Arc, Mutex},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{
    auth::{Error, Material, MaterialFuture, MaterialProvider},
    config::{Config, Textures},
    store::{self, Store},
};

pub type RepositoryFuture<'a, T> = Pin<Box<dyn Future<Output = store::Result<T>> + Send + 'a>>;

pub trait Repository: Send + Sync {
    fn skin_apis(&self) -> RepositoryFuture<'_, Vec<String>>;
    fn profile_by_uuid<'a>(&'a self, uuid: &'a str)
    -> RepositoryFuture<'a, Option<store::Profile>>;
}

impl Repository for Store {
    fn skin_apis(&self) -> RepositoryFuture<'_, Vec<String>> {
        Box::pin(self.read(|mut queries| Box::pin(async move { queries.skin_apis().await })))
    }

    fn profile_by_uuid<'a>(
        &'a self,
        uuid: &'a str,
    ) -> RepositoryFuture<'a, Option<store::Profile>> {
        let uuid = uuid.to_owned();
        Box::pin(
            self.read(move |mut queries| {
                Box::pin(async move { queries.profile_by_uuid(&uuid).await })
            }),
        )
    }
}

pub type AvatarFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;

pub trait AvatarProvider: Send + Sync {
    fn avatar<'a>(&'a self, uuid: &'a str, size: u32) -> AvatarFuture<'a>;
}

pub struct Service {
    config: Textures,
    repository: Arc<dyn Repository>,
    http: reqwest::Client,
    files: Mutex<()>,
}

#[derive(Deserialize)]
struct Csl {
    #[serde(default)]
    skins: Option<HashMap<String, Option<String>>>,
    #[serde(default)]
    cape: Option<String>,
}

impl Service {
    pub fn new(config: &Config, repository: Arc<dyn Repository>) -> Result<Self, Error> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .tls_backend_rustls()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(config.textures.request_timeout.0)
            .build()
            .map_err(|_| Error::Material)?;
        Ok(Self {
            config: config.textures.clone(),
            repository,
            http,
            files: Mutex::new(()),
        })
    }

    async fn fetch(&self, address: String) -> Option<Vec<u8>> {
        let response = self.http.get(address).send().await.ok()?;
        if response.status() != reqwest::StatusCode::OK {
            return None;
        }
        Some(response.bytes().await.ok()?.to_vec())
    }

    pub async fn resolve_material(&self, name: &str) -> Result<Material, Error> {
        let deadline = tokio::time::Instant::now() + self.config.request_timeout.0;
        let apis = tokio::time::timeout_at(deadline, self.repository.skin_apis())
            .await
            .map_err(|_| Error::Store(store::Error::Timeout))??;
        let mut material = Material {
            skin: String::new(),
            model: String::new(),
            cape: String::new(),
        };
        // Remote material sources are optional. Preserve partial results if their
        // shared budget expires; database and filesystem failures remain errors.
        if let Ok(result) =
            tokio::time::timeout_at(deadline, self.resolve_sources(name, apis, &mut material)).await
        {
            result?;
        }
        Ok(material)
    }

    async fn resolve_sources(
        &self,
        name: &str,
        apis: Vec<String>,
        material: &mut Material,
    ) -> Result<(), Error> {
        for api in apis {
            let Some(data) = self
                .fetch(format!("{api}/csl/{}.json", escape_query(name)))
                .await
            else {
                continue;
            };
            let Ok(csl) = serde_json::from_slice::<Csl>(&data) else {
                continue;
            };
            let skins = csl.skins.unwrap_or_default();
            material.model = if skins.get("slim").is_some_and(Option::is_some) {
                "slim"
            } else {
                "default"
            }
            .into();
            if let Some(Some(skin)) = skins.get(&material.model)
                && !skin.is_empty()
                && self.download_named(&api, skin).await?
            {
                material.skin = skin.clone();
            }
            if let Some(cape) = csl.cape
                && !cape.is_empty()
                && self.download_named(&api, &cape).await?
            {
                material.cape = cape;
            }
            if !material.skin.is_empty() || !material.cape.is_empty() {
                break;
            }
        }
        if material.skin.is_empty() || material.cape.is_empty() {
            self.fallback(name, material).await?;
        }
        Ok(())
    }

    async fn download_named(&self, api: &str, key: &str) -> Result<bool, Error> {
        if !valid_key(key) {
            return Ok(false);
        }
        let Some(data) = self
            .fetch(format!("{api}/textures/{}", escape_path(key)))
            .await
        else {
            return Ok(false);
        };
        if image::decode_png(&data).is_err() {
            return Ok(false);
        }
        self.save(key, &data)?;
        Ok(true)
    }

    fn save(&self, key: &str, data: &[u8]) -> Result<(), Error> {
        if !valid_key(key) {
            return Err(Error::Material);
        }
        let mut file = tempfile::Builder::new()
            .prefix(".texture-")
            .tempfile_in(&self.config.directory)
            .map_err(|_| Error::Material)?;
        file.write_all(data).map_err(|_| Error::Material)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o644))
                .map_err(|_| Error::Material)?;
        }
        let temporary = file.into_temp_path();
        let _guard = self.files.lock().expect("texture publication lock");
        temporary
            .persist(self.config.directory.join(key))
            .map_err(|_| Error::Material)?;
        Ok(())
    }

    async fn fallback(&self, name: &str, material: &mut Material) -> Result<(), Error> {
        let Some(data) = self
            .fetch(format!(
                "{}/users/profiles/minecraft/{}",
                self.config.mojang_base_url.trim_end_matches('/'),
                escape_path(name)
            ))
            .await
        else {
            return Ok(());
        };
        #[derive(Deserialize)]
        struct Profile {
            #[serde(default)]
            id: Option<String>,
        }
        let Ok(profile) = serde_json::from_slice::<Profile>(&data) else {
            return Ok(());
        };
        let Some(id) = profile.id.filter(|id| !id.is_empty()) else {
            return Ok(());
        };
        for (path, cape) in [("/skins/", false), ("/capes/", true)] {
            if (cape && !material.cape.is_empty()) || (!cape && !material.skin.is_empty()) {
                continue;
            }
            let Some(data) = self
                .fetch(format!(
                    "{}{path}{}",
                    self.config.crafatar_base_url.trim_end_matches('/'),
                    escape_path(&id)
                ))
                .await
            else {
                continue;
            };
            let Ok(skin) = image::decode_png(&data) else {
                continue;
            };
            let key = format!("{:x}", Sha256::digest(&data));
            self.save(&key, &data)?;
            if cape {
                material.cape = key;
            } else {
                material.skin = key;
                material.model = image::model_type(&skin).into();
            }
        }
        Ok(())
    }

    pub async fn render_avatar(&self, uuid: &str, size: u32) -> Result<Vec<u8>, Error> {
        if size == 0 {
            return Err(Error::api(
                400,
                "IllegalArgumentException",
                "Avatar size must be a positive integer.",
            ));
        }
        let profile = self.repository.profile_by_uuid(uuid).await?;
        let Some(profile) = profile else {
            let data = self
                .fetch(format!(
                    "{}/helm/{}/{size}",
                    self.config.crafthead_base_url.trim_end_matches('/'),
                    escape_path(uuid)
                ))
                .await
                .ok_or(Error::Material)?;
            image::decode_png(&data)?;
            return Ok(data);
        };
        let material = self.resolve_material(&profile.name).await?;
        if !valid_key(&material.skin) {
            return Err(Error::Material);
        }
        let data = {
            let _guard = self.files.lock().expect("texture publication lock");
            std::fs::read(self.config.directory.join(&material.skin))
                .map_err(|_| Error::Material)?
        };
        image::avatar(&image::decode_png(&data)?, size)
    }
}

impl MaterialProvider for Service {
    fn resolve<'a>(&'a self, name: &'a str) -> MaterialFuture<'a> {
        Box::pin(self.resolve_material(name))
    }
}

impl AvatarProvider for Service {
    fn avatar<'a>(&'a self, uuid: &'a str, size: u32) -> AvatarFuture<'a> {
        Box::pin(self.render_avatar(uuid, size))
    }
}

fn valid_key(key: &str) -> bool {
    if key.is_empty() || matches!(key, "." | "..") || key.contains(['/', '\\', '\0']) {
        return false;
    }
    if !matches!(
        Path::new(key).components().next(),
        Some(Component::Normal(_))
    ) {
        return false;
    }
    #[cfg(windows)]
    {
        if key.contains(':') {
            return false;
        }
        let base = key
            .split('.')
            .next()
            .unwrap_or("")
            .trim_end_matches(' ')
            .to_uppercase();
        if matches!(
            base.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) {
            return false;
        }
        if let Some(number) = base
            .strip_prefix("COM")
            .or_else(|| base.strip_prefix("LPT"))
            && matches!(
                number,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        {
            return false;
        }
    }
    true
}

fn escape_query(value: &str) -> String {
    escape(value, true)
}
fn escape_path(value: &str) -> String {
    escape(value, false)
}

fn escape(value: &str, query: bool) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric()
            || b"-_.~".contains(&byte)
            || (!query && b"$&+:=@".contains(&byte))
        {
            encoded.push(char::from(byte));
        } else if query && byte == b' ' {
            encoded.push('+');
        } else {
            use std::fmt::Write;
            write!(&mut encoded, "%{byte:02X}").expect("write to String");
        }
    }
    encoded
}
