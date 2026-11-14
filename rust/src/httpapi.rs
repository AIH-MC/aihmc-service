use std::{
    net::{IpAddr, SocketAddr},
    sync::{Arc, LazyLock},
    time::Instant,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, Method, Response, StatusCode, Uri},
};
use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    auth::{Error, Service},
    protocol::Profile,
};

static UUID_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("profile/([a-f0-9]{32})").expect("valid profile expression"));

pub fn router(service: Arc<Service>) -> Router {
    Router::new().fallback(dispatch).with_state(service)
}

async fn dispatch(
    State(service): State<Arc<Service>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response<Body> {
    let started = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let result = handle(&service, peer, request).await;
    let response = match result {
        Ok(response) => response,
        Err(error) => {
            let (status, name, message) = match error {
                Error::Api {
                    status,
                    name,
                    message,
                } => (status, name, message),
                other => {
                    // Database diagnostics may contain tokens or bound values.
                    let category = match other {
                        Error::Store(_) => "database",
                        Error::Upstream => "upstream",
                        _ => "protocol",
                    };
                    tracing::error!(%path, error_type = category, "request failed");
                    (500, "InternalServerError", "Internal server error.".into())
                }
            };
            json_response(status, &json!({"error": name, "errorMessage": message}))
                .expect("fixed error response serializes")
        }
    };
    tracing::info!(%method, %path, status = response.status().as_u16(), elapsed_ms = started.elapsed().as_millis() as u64, "request completed");
    response
}

async fn handle(
    service: &Service,
    peer: SocketAddr,
    request: Request,
) -> Result<Response<Body>, Error> {
    let uri = request.uri().clone();
    let raw = uri.path_and_query().map(|uri| uri.as_str()).unwrap_or("/");
    let method = request.method();
    match raw {
        "/" => json_response(
            200,
            &json!({"message": "This is AI Hobbyist MC API, please use the correct endpoint to access the API."}),
        ),
        "/mcauth" => {
            require_method(method, Method::GET)?;
            json_response(200, &service.metadata().await?)
        }
        "/mcauth/authserver/authenticate" => {
            require_method(method, Method::POST)?;
            let body = read_object(service, request).await?;
            let agent = field(&body, "agent");
            if !agent.is_null() && !agent.is_object() {
                return Err(bad_json());
            }
            let login = service
                .authenticate(
                    &string(&body, "username")?,
                    &string(&body, "password")?,
                    &string(&body, "clientToken")?,
                    &string(agent, "name")?,
                    truth(field(&body, "requestUser")),
                )
                .await?;
            json_response(200, &login)
        }
        "/mcauth/authserver/refresh"
        | "/mcauth/authserver/validate"
        | "/mcauth/authserver/invalidate" => {
            require_method(method, Method::POST)?;
            let body = read_object(service, request).await?;
            let access = string(&body, "accessToken")?;
            let client = string(&body, "clientToken")?;
            match raw {
                "/mcauth/authserver/refresh" => json_response(
                    200,
                    &service
                        .refresh(access, client, truth(field(&body, "requestUser")))
                        .await?,
                ),
                "/mcauth/authserver/validate" => {
                    service.validate(access, client).await?;
                    Ok(empty_response())
                }
                _ => {
                    service.invalidate(access, client).await?;
                    Ok(empty_response())
                }
            }
        }
        "/mcauth/authserver/signout" => {
            require_method(method, Method::POST)?;
            let body = read_object(service, request).await?;
            service
                .signout(&string(&body, "username")?, &string(&body, "password")?)
                .await?;
            Ok(empty_response())
        }
        _ if raw.starts_with("/mcauth/sessionserver/session/minecraft/profile") => {
            require_method(method, Method::GET)?;
            let uuid = UUID_PATH
                .captures(raw)
                .and_then(|capture| capture.get(1))
                .map(|value| value.as_str())
                .unwrap_or("");
            profile_response(
                service
                    .profile(uuid.into(), &last_query(&uri, "unsigned"))
                    .await?,
            )
        }
        "/mcauth/api/profiles/minecraft" => {
            require_method(method, Method::POST)?;
            let body = read_json(service, request).await?;
            let names = match body {
                Value::Null => vec![],
                Value::Array(values) => values
                    .into_iter()
                    .map(|value| match value {
                        Value::String(name) => Ok(name),
                        Value::Null => Ok(String::new()),
                        _ => Err(bad_json()),
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                _ => return Err(bad_json()),
            };
            json_response(200, &service.profiles(names).await?)
        }
        "/mcauth/sessionserver/session/minecraft/join" => {
            require_method(method, Method::POST)?;
            let ip = client_ip(peer.ip(), request.headers());
            let body = read_object(service, request).await?;
            service
                .join(
                    string(&body, "serverId")?,
                    string(&body, "accessToken")?,
                    string(&body, "selectedProfile")?,
                    ip,
                )
                .await?;
            Ok(empty_response())
        }
        _ if raw.starts_with("/mcauth/sessionserver/session/minecraft/hasJoined") => {
            require_method(method, Method::GET)?;
            let name = query_decode(&last_query(&uri, "username"))?;
            profile_response(
                service
                    .has_joined(name, last_query(&uri, "serverId"))
                    .await?,
            )
        }
        _ if raw.starts_with("/mcauth/users/profiles/minecraft/")
            || raw.starts_with("/mcauth/api/users/profiles/minecraft/") =>
        {
            require_method(method, Method::GET)?;
            let mut parts = vec![];
            for part in uri.path().split('/') {
                match part {
                    "" | "." => {}
                    ".." => {
                        parts.pop();
                    }
                    _ => parts.push(part),
                }
            }
            let name = query_decode(parts.last().copied().unwrap_or("/"))?;
            json_response(200, &service.name(name).await?)
        }
        _ => json_response(200, &service.metadata().await?),
    }
}

fn require_method(actual: &Method, expected: Method) -> Result<(), Error> {
    if actual != expected {
        return Err(Error::api(
            405,
            "HTTP/1.1 405 Method Not Allowed",
            "不支持的请求方法",
        ));
    }
    Ok(())
}

fn bad_json() -> Error {
    Error::api(
        400,
        "IllegalArgumentException",
        "Invalid JSON request body.",
    )
}

async fn read_json(service: &Service, request: Request) -> Result<Value, Error> {
    let budget = request
        .extensions()
        .get::<crate::server::ReadBudget>()
        .cloned();
    let deadline = budget
        .as_ref()
        .map(|budget| budget.deadline)
        .unwrap_or_else(|| {
            tokio::time::Instant::now() + service.runtime.config.server.read_timeout.0
        });
    let data = tokio::time::timeout_at(deadline, to_bytes(request.into_body(), usize::MAX))
        .await
        .map_err(|_| bad_json())?
        .map_err(|_| bad_json())?;
    if let Some(budget) = budget {
        budget.complete();
    }
    serde_json::from_slice(&data).map_err(|_| bad_json())
}

async fn read_object(service: &Service, request: Request) -> Result<Value, Error> {
    let value = read_json(service, request).await?;
    if !value.is_null() && !value.is_object() {
        return Err(bad_json());
    }
    Ok(value)
}

fn field<'a>(object: &'a Value, name: &str) -> &'a Value {
    object
        .as_object()
        .and_then(|fields| {
            fields
                .iter()
                .rev()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value)
        })
        .unwrap_or(&Value::Null)
}

fn string(object: &Value, name: &str) -> Result<String, Error> {
    match field(object, name) {
        Value::Null => Ok(String::new()),
        Value::String(value) => Ok(value.clone()),
        _ => Err(bad_json()),
    }
}

fn truth(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::String(value) => crate::protocol::php_truthy_query(value),
        Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
        Value::Array(values) => !values.is_empty(),
        Value::Object(fields) => !fields.is_empty(),
    }
}

fn query_decode(text: &str) -> Result<String, Error> {
    let mut decoded = vec![];
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let invalid = || {
                    Error::api(
                        400,
                        "IllegalArgumentException",
                        "Invalid username encoding.",
                    )
                };
                let hex = bytes.get(index + 1..index + 3).ok_or_else(invalid)?;
                let high = (hex[0] as char).to_digit(16).ok_or_else(invalid)?;
                let low = (hex[1] as char).to_digit(16).ok_or_else(invalid)?;
                decoded.push((high * 16 + low) as u8);
                index += 3;
            }
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            value => {
                decoded.push(value);
                index += 1;
            }
        }
    }
    Ok(String::from_utf8_lossy(&decoded).into_owned())
}

fn last_query(uri: &Uri, key: &str) -> String {
    let mut last = String::new();
    for pair in uri
        .query()
        .unwrap_or("")
        .split('&')
        .filter(|pair| !pair.contains(';'))
    {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if let (Ok(name), Ok(value)) = (query_decode(name), query_decode(value))
            && name == key
        {
            last = value;
        }
    }
    last
}

pub fn client_ip(peer: IpAddr, headers: &HeaderMap) -> String {
    let peer = canonical_ip(peer);
    if peer.is_loopback()
        && let Some(address) = headers
            .get("x-real-ip")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<IpAddr>().ok())
    {
        return canonical_ip(address).to_string();
    }
    peer.to_string()
}

fn canonical_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(ip) => ip
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(ip)),
        ip => ip,
    }
}

fn json_response(status: u16, value: &impl Serialize) -> Result<Response<Body>, Error> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/json; charset=utf-8")
        .body(Body::from(bytes))
        .expect("known response status and content type"))
}

fn empty_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header("content-type", "application/json; charset=utf-8")
        .body(Body::empty())
        .expect("known empty response")
}

fn profile_response(profile: Option<Profile>) -> Result<Response<Body>, Error> {
    match profile {
        Some(profile) => json_response(200, &profile),
        None => Ok(empty_response()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusts_forwarded_ip_only_from_loopback_including_mapped_ipv4() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "198.51.100.42".parse().unwrap());
        for peer in ["127.0.0.1", "::1", "::ffff:127.0.0.1"] {
            assert_eq!(client_ip(peer.parse().unwrap(), &headers), "198.51.100.42");
        }
        assert_eq!(
            client_ip("203.0.113.9".parse().unwrap(), &headers),
            "203.0.113.9"
        );
        headers.insert("x-real-ip", "not-an-ip".parse().unwrap());
        assert_eq!(
            client_ip("127.0.0.1".parse().unwrap(), &headers),
            "127.0.0.1"
        );
    }

    #[test]
    fn preserves_last_query_value_and_double_decoding_rules() {
        let uri: Uri = "/profile/id?unsigned=false&broken=%&unsigned=0"
            .parse()
            .unwrap();
        assert_eq!(last_query(&uri, "unsigned"), "0");
        assert_eq!(
            query_decode("%E8%BF%81%E7%A7%BB+User").unwrap(),
            "迁移 User"
        );
        assert!(query_decode("invalid%2").is_err());
        let uri: Uri = "/hasJoined?username=%2525E8%2525BF%252581".parse().unwrap();
        assert_eq!(
            query_decode(&last_query(&uri, "username")).unwrap(),
            "%E8%BF%81"
        );
    }
}
