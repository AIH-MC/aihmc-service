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

pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route(
            "/check_token",
            get(|| async { Json(json!({"message": "验证通过"})) }),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), authorize));
    Router::new()
        .route(
            "/",
            get(|| async { Json(json!({"message": "Hello World"})) }),
        )
        .merge(protected)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
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
