use serde_json::Value;

use crate::{
    config::Source,
    error::{ApiError, ApiResult},
    state::AppState,
};

pub fn domain(source: &Source) -> ApiResult<(String, String)> {
    let url = reqwest::Url::parse(&source.root_url).map_err(ApiError::internal)?;
    let host = url
        .host_str()
        .ok_or_else(|| ApiError::internal("source URL has no host"))?;
    let domain = psl::domain_str(host).unwrap_or(host);
    Ok((url.scheme().to_string(), domain.to_string()))
}

pub fn endpoint(source: &Source, operation: &str) -> ApiResult<String> {
    let (scheme, domain) = domain(source)?;
    let path = match (source.api_type.as_str(), operation) {
        ("mojang", "join") => format!("sessionserver.{domain}/session/minecraft/hasJoined"),
        ("mojang", "profile") => format!("sessionserver.{domain}/session/minecraft/profile"),
        ("mojang", "name") => format!("api.{domain}/users/profiles/minecraft"),
        ("aihmc", "join") => {
            format!("api.{domain}/mcauth/sessionserver/session/minecraft/hasJoined")
        }
        ("aihmc", "profile") => {
            format!("api.{domain}/mcauth/sessionserver/session/minecraft/profile")
        }
        ("aihmc", "name") => format!("api.{domain}/mcauth/api/users/profiles/minecraft"),
        ("blessingskin", "join") => {
            format!("{domain}/api/yggdrasil/sessionserver/session/minecraft/hasJoined")
        }
        ("blessingskin", "profile") => {
            format!("{domain}/api/yggdrasil/sessionserver/session/minecraft/profile")
        }
        ("blessingskin", "name") => format!("{domain}/api/yggdrasil/api/users/profiles/minecraft"),
        ("elyby", "join") => format!("account.{domain}/api/minecraft/session/hasJoined"),
        ("elyby", "profile") => format!("account.{domain}/api/minecraft/session/profile"),
        ("elyby", "head-profile") => format!("authserver.{domain}/session/profile"),
        ("elyby", "name") => format!("authserver.{domain}/api/users/profiles/minecraft"),
        (_, "head-profile") => return endpoint(source, "profile"),
        _ => return Err(ApiError::internal("unsupported source type")),
    };
    Ok(format!("{scheme}://{path}"))
}

pub async fn get_json(state: &AppState, url: &str) -> ApiResult<Option<Value>> {
    let response = state
        .client
        .get(url)
        .send()
        .await
        .map_err(ApiError::internal)?;
    if response.status() != reqwest::StatusCode::OK {
        return Ok(None);
    }
    let value: Value = response.json().await.map_err(ApiError::internal)?;
    Ok(if truthy(&value) { Some(value) } else { None })
}

pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
        Value::String(value) => !value.is_empty(),
        Value::Number(value) => value.as_f64() != Some(0.0),
    }
}

pub async fn join(
    state: &AppState,
    source: &Source,
    username: &str,
    server_id: &str,
    ip: Option<&str>,
) -> ApiResult<Option<Value>> {
    let endpoint = endpoint(source, "join")?;
    // Characterized Python behavior: IP is appended using a second question mark.
    let mut url = format!("{endpoint}?username={username}&serverId={server_id}");
    if let Some(ip) = ip {
        url.push_str(&format!("?ip={ip}"));
    }
    get_json(state, &url).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_suffix_and_four_sources() {
        for (kind, expected) in [
            (
                "mojang",
                "https://sessionserver.example.co.uk/session/minecraft/hasJoined",
            ),
            (
                "aihmc",
                "https://api.example.co.uk/mcauth/sessionserver/session/minecraft/hasJoined",
            ),
            (
                "blessingskin",
                "https://example.co.uk/api/yggdrasil/sessionserver/session/minecraft/hasJoined",
            ),
            (
                "elyby",
                "https://account.example.co.uk/api/minecraft/session/hasJoined",
            ),
        ] {
            let source = Source {
                name: kind.into(),
                api_type: kind.into(),
                root_url: "https://nested.example.co.uk".into(),
                enabled: Some(true),
            };
            assert_eq!(endpoint(&source, "join").ok().unwrap(), expected);
        }
    }
}
