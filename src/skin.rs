use std::time::Duration;

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

use crate::{
    db,
    error::{ApiError, ApiResult},
    state::AppState,
    upstream,
};

#[derive(Debug, PartialEq)]
pub struct Textures {
    pub skin: Option<String>,
    pub cape: Option<String>,
    pub model: String,
}

pub fn decode_textures(profile: &Value) -> Option<Value> {
    let property = profile["properties"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == "textures")?;
    let bytes = STANDARD.decode(property["value"].as_str()?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn parse_mojang(profile: &Value) -> Option<Textures> {
    let decoded = decode_textures(profile)?;
    let textures = &decoded["textures"];
    let hash = |value: &Value| {
        value.as_str().map(|url| {
            url.rsplit('/')
                .next()
                .unwrap_or_default()
                .replace(".png", "")
        })
    };
    Some(Textures {
        skin: hash(&textures["SKIN"]["url"]),
        cape: hash(&textures["CAPE"]["url"]),
        model: textures["SKIN"]["metadata"]["model"]
            .as_str()
            .unwrap_or("default")
            .into(),
    })
}

pub fn parse_csl(value: &Value) -> Textures {
    let first = value["skins"]
        .as_object()
        .and_then(|skins| skins.iter().next());
    Textures {
        skin: first.and_then(|(_, hash)| hash.as_str().map(String::from)),
        cape: value["cape"].as_str().map(String::from),
        model: first.map_or("default", |(model, _)| model.as_str()).into(),
    }
}

pub async fn cache_texture(state: &AppState, url: &str) -> bool {
    let directory = state.resource("textures");
    let hash = url.rsplit('/').next().unwrap_or_default();
    let path = directory.join(hash);
    if path.exists() {
        return true;
    }
    let result = async {
        tokio::fs::create_dir_all(&directory).await?;
        let response = state
            .client
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await?;
        if response.status() != reqwest::StatusCode::OK {
            return Ok(false);
        }
        let bytes = response.bytes().await?;
        tokio::fs::write(path, bytes).await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(true)
    }
    .await;
    match result {
        Ok(success) => success,
        Err(error) => {
            tracing::error!(%error, "texture download failed");
            false
        }
    }
}

async fn from_source(
    state: &AppState,
    source: &crate::config::Source,
    username: &str,
) -> ApiResult<Option<Value>> {
    let (scheme, domain) = upstream::domain(source)?;
    let textures = match source.api_type.as_str() {
        "mojang" => {
            let name_url = format!("{scheme}://api.{domain}/users/profiles/minecraft/{username}");
            if let Some(name) = upstream::get_json(state, &name_url).await? {
                let uuid = name["id"]
                    .as_str()
                    .ok_or_else(|| ApiError::internal("profile has no id"))?;
                let profile_url =
                    format!("{scheme}://sessionserver.{domain}/session/minecraft/profile/{uuid}");
                upstream::get_json(state, &profile_url)
                    .await?
                    .and_then(|profile| parse_mojang(&profile))
            } else {
                None
            }
        }
        "aihmc" | "blessingskin" => {
            let host = if source.api_type == "aihmc" {
                format!("skin.{domain}")
            } else {
                domain.clone()
            };
            let url = format!("{scheme}://{host}/csl/{username}.json");
            upstream::get_json(state, &url)
                .await?
                .map(|value| parse_csl(&value))
        }
        "elyby" => {
            let url =
                format!("{scheme}://authserver.{domain}/api/users/profiles/minecraft/{username}");
            upstream::get_json(state, &url)
                .await?
                .and_then(|profile| parse_mojang(&profile))
        }
        _ => None,
    };
    let Some(textures) = textures.filter(|t| {
        t.skin.as_ref().is_some_and(|s| !s.is_empty())
            || t.cape.as_ref().is_some_and(|s| !s.is_empty())
    }) else {
        return Ok(None);
    };
    let mut downloads = Vec::new();
    let mut add_downloads = |prefix: &str| {
        for hash in [&textures.skin, &textures.cape]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
        {
            downloads.push(format!("{prefix}/{hash}"));
        }
    };
    match source.api_type.as_str() {
        // The Python `official`/`mojang` mismatch skips downloads for mojang.
        "elyby" => {
            add_downloads(&format!("{scheme}://textures.minecraft.net/texture"));
            if let Some(hash) = &textures.skin {
                downloads.push(format!("{scheme}://{domain}/storage/skins/{hash}"));
            }
            if let Some(hash) = &textures.cape {
                downloads.push(format!("{scheme}://{domain}/storage/skins/{hash}"));
            }
        }
        "aihmc" => add_downloads(&format!("{scheme}://skin.{domain}/textures")),
        "blessingskin" => add_downloads(&format!("{scheme}://{domain}/textures")),
        _ => {}
    }
    for url in downloads {
        cache_texture(state, &url).await;
    }
    let sql = "
        INSERT INTO skins (username, cape_hash, skin_hash, model_type) VALUES (?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE username=?, cape_hash=?, skin_hash=?, model_type=?
    ";
    db::execute(
        state,
        sql,
        &[
            json!(username),
            json!(textures.cape),
            json!(textures.skin),
            json!(textures.model),
            json!(username),
            json!(textures.cape),
            json!(textures.skin),
            json!(textures.model),
        ],
    )
    .await;
    let rows = db::query(
        state,
        "SELECT * FROM skins WHERE username = ?",
        &[json!(username)],
    )
    .await;
    let row = rows
        .first()
        .ok_or_else(|| ApiError::internal("empty skin record"))?;
    let mut skins = serde_json::Map::new();
    skins.insert(
        db::text(row, "model_type").unwrap_or_else(|| "null".into()),
        json!(db::text(row, "skin_hash")),
    );
    let mut response = json!({"username": db::text(row, "username"), "skins": skins});
    if let Some(cape) = db::text(row, "cape_hash").filter(|s| !s.is_empty()) {
        response["cape"] = json!(cape);
    }
    Ok(Some(response))
}

pub async fn create_csl(state: &AppState, username: &str) -> ApiResult<Option<Value>> {
    for source in state.config.servers.iter().filter(|s| s.is_enabled()) {
        if let Some(value) = from_source(state, source, username).await? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mojang_and_csl_models() {
        let encoded = STANDARD.encode(br#"{"textures":{"SKIN":{"url":"https://example.com/skin.png","metadata":{"model":"slim"}},"CAPE":{"url":"https://example.com/cape"}}}"#);
        let profile = json!({"properties": [{"name": "textures", "value": encoded}]});
        assert_eq!(
            parse_mojang(&profile),
            Some(Textures {
                skin: Some("skin".into()),
                cape: Some("cape".into()),
                model: "slim".into()
            })
        );
        assert_eq!(
            parse_csl(&json!({"skins": {"default": "skin"}, "cape": "cape"})).model,
            "default"
        );
        assert!(parse_mojang(&json!({})).is_none());
    }
}
