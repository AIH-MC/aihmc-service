use axum::{
    Json, Router,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};

use crate::state::AppState;
use crate::{
    auth,
    error::{ApiError, ApiResult},
    figura, offline, profile,
};
use axum::extract::{ConnectInfo, OriginalUri, Path, Query, rejection::JsonRejection};
use axum::routing::post;
use serde_json::Value;
use std::{collections::HashMap, net::SocketAddr};

pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route(
            "/check_token",
            get(|| async { Json(json!({"message": "验证通过"})) }),
        )
        .route("/offline/check/{username}", get(offline_check))
        .route("/profile/rstname/{uuid}", get(reset_name))
        .route("/offline/reg", post(admin))
        .route("/offline/login", post(admin))
        .route("/offline/chpasswd", post(admin))
        .route("/figura/register", post(admin))
        .route("/figura/refresh", post(admin))
        .route("/figura/server", post(admin))
        .route("/profile/linkprofile", post(admin))
        .route("/profile/rename", post(admin))
        .route_layer(middleware::from_fn_with_state(state.clone(), authorize));
    Router::new()
        .route(
            "/",
            get(|| async { Json(json!({"message": "Hello World"})) }),
        )
        .route("/auth", get(metadata))
        .route("/auth/sessionserver/session/minecraft/hasJoined", get(join))
        .route(
            "/auth/sessionserver/session/minecraft/profile/{uuid}",
            get(ygg_profile),
        )
        .route(
            "/auth/api/users/profiles/minecraft/{username}",
            get(check_name),
        )
        .route("/figura/hasJoined", get(figura_join))
        .merge(protected)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}

async fn metadata(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(Json(auth::metadata(&state).await?))
}

fn optional_profile(profile: Option<Value>) -> Response {
    profile.map_or_else(
        || StatusCode::NO_CONTENT.into_response(),
        |value| Json(value).into_response(),
    )
}

fn require_query(query: &HashMap<String, String>, fields: &[&str]) -> ApiResult<()> {
    let errors = fields
        .iter()
        .filter(|field| !query.contains_key(**field))
        .map(|field| {
            json!({
                "type": "missing", "loc": ["query", field],
                "msg": "Field required", "input": null
            })
        })
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::validation(errors))
    }
}

async fn join(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Response> {
    require_query(&query, &["username", "serverId"])?;
    Ok(optional_profile(
        auth::has_joined(
            &state,
            &query["username"],
            &query["serverId"],
            query.get("ip").map(String::as_str),
        )
        .await?,
    ))
}

async fn figura_join(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Response> {
    require_query(&query, &["username", "serverId"])?;
    Ok(optional_profile(
        figura::has_joined(
            &state,
            &query["username"],
            &query["serverId"],
            query.get("ip").map(String::as_str),
        )
        .await?,
    ))
}

async fn ygg_profile(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
) -> ApiResult<Response> {
    Ok(optional_profile(auth::profile(&state, &uuid).await?))
}

async fn check_name(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> ApiResult<Response> {
    Ok(optional_profile(auth::check_name(&state, &username).await?))
}

fn message(value: impl Into<String>) -> Json<Value> {
    Json(json!({"message": value.into()}))
}

async fn offline_check(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(message(offline::check(&state, &username).await?))
}

async fn reset_name(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
) -> ApiResult<Json<Value>> {
    profile::reset_name(&state, &uuid).await?;
    Ok(message("操作成功"))
}

fn validate_body(body: &Value, fields: &[&str], optional_ip: bool) -> ApiResult<()> {
    if !body.is_object() {
        return Err(ApiError::validation(vec![
            json!({"type": "model_attributes_type", "loc": ["body"], "msg": "Input should be a valid dictionary or object to extract fields from", "input": body}),
        ]));
    }
    let mut errors = Vec::new();
    for field in fields {
        match body.get(*field) {
            None => errors.push(json!({"type": "missing", "loc": ["body", field], "msg": "Field required", "input": body})),
            Some(value) if !value.is_string() => errors.push(json!({"type": "string_type", "loc": ["body", field], "msg": "Input should be a valid string", "input": value})),
            _ => {}
        }
    }
    if optional_ip
        && let Some(ip) = body.get("ip")
        && !ip.is_null()
        && !ip.is_string()
    {
        errors.push(json!({"type": "string_type", "loc": ["body", "ip"], "msg": "Input should be a valid string", "input": ip}));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::validation(errors))
    }
}

async fn admin(
    State(state): State<AppState>,
    OriginalUri(uri): OriginalUri,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    body: Result<Json<Value>, JsonRejection>,
) -> ApiResult<Json<Value>> {
    let body = body
        .map_err(|error| {
            ApiError::validation(vec![json!({
                "type": "json_invalid", "loc": ["body", 0],
                "msg": "JSON decode error", "input": {},
                "ctx": {"error": error.body_text()}
            })])
        })?
        .0;
    let path = uri.path();
    let fields: &[&str] = match path {
        "/offline/reg" | "/figura/register" => &["username", "passwd", "repasswd"],
        "/offline/login" | "/figura/refresh" => &["username", "passwd"],
        "/offline/chpasswd" => &["username", "passwd", "newpasswd"],
        "/profile/linkprofile" => &["uuid", "new_uuid"],
        _ => &["uuid", "username"],
    };
    validate_body(&body, fields, path == "/offline/reg")?;
    let field = |name: &str| body[name].as_str().unwrap_or_default();
    let peer_ip = peer.ip().to_string();
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .unwrap_or(&peer_ip);
    let result = match path {
        "/offline/reg" => {
            offline::register(
                &state,
                field("username"),
                field("passwd"),
                field("repasswd"),
                body["ip"].as_str(),
            )
            .await?
        }
        "/offline/login" => offline::login(&state, field("username"), field("passwd")).await?,
        "/offline/chpasswd" => {
            offline::change_password(
                &state,
                field("username"),
                field("passwd"),
                field("newpasswd"),
            )
            .await?
        }
        "/figura/register" => {
            figura::register(
                &state,
                field("username"),
                field("passwd"),
                field("repasswd"),
                ip,
            )
            .await?;
            "操作成功".into()
        }
        "/figura/refresh" => {
            figura::refresh(&state, field("username"), field("passwd"), ip).await?;
            "操作成功".into()
        }
        "/figura/server" => figura::sync(&state, field("uuid"), field("username")).await,
        "/profile/linkprofile" => {
            profile::link(&state, field("uuid"), field("new_uuid")).await?;
            "操作成功".into()
        }
        "/profile/rename" => {
            profile::rename(&state, field("uuid"), field("username")).await?;
            "操作成功".into()
        }
        _ => unreachable!("admin is registered only for supported routes"),
    };
    Ok(message(result))
}

async fn authorize(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let credentials = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split_once(' '))
        .filter(|(scheme, token)| scheme.eq_ignore_ascii_case("bearer") && !token.is_empty());
    match credentials {
        None => (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
            Json(json!({"detail": "Not authenticated"})),
        )
            .into_response(),
        Some((_, token)) if token != state.config.access_token => (
            StatusCode::UNAUTHORIZED,
            [("www+authenticate", "Bearer")],
            Json(json!({"detail": "Invalid or missing Bearer Token"})),
        )
            .into_response(),
        _ => next.run(request).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use tower::ServiceExt;

    #[tokio::test]
    async fn root_and_token_contracts() {
        let mut config =
            toml::from_str::<crate::config::Config>(include_str!("../config.example.toml"))
                .unwrap();
        config.access_token = "test-token".into();
        let app = router(AppState::new(config, ".".into()).unwrap());
        let response = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
            br#"{"message":"Hello World"}"#
        );
        for (token, status) in [
            (None, 401),
            (Some("Bearer wrong"), 401),
            (Some("Bearer test-token"), 200),
        ] {
            let mut request = Request::builder().uri("/check_token");
            if let Some(token) = token {
                request = request.header(header::AUTHORIZATION, token);
            }
            let response = app
                .clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), status);
        }
    }
}
