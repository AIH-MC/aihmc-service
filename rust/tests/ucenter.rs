use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use aihmc_api::ucenter::{self, Client, Error};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
};
use serde_json::{Value, json};

#[derive(Clone)]
struct Fixture {
    requests: Arc<Mutex<Vec<String>>>,
    redirected: Arc<AtomicUsize>,
    mode: &'static str,
}

async fn upstream(
    State(fixture): State<Fixture>,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if fixture.mode == "redirect" {
        if uri.path() == "/redirected" {
            fixture.redirected.fetch_add(1, Ordering::SeqCst);
            return Json(json!({"token":"unexpected"})).into_response();
        }
        return (StatusCode::FOUND, [("location", "/redirected")]).into_response();
    }
    if fixture.mode == "invalid" {
        return (StatusCode::OK, "not JSON").into_response();
    }
    if fixture.mode == "error" {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if fixture.mode == "slow" {
        tokio::time::sleep(Duration::from_millis(80)).await;
    }
    assert_eq!(uri.path(), "/api/");
    let path = uri.query().unwrap();
    fixture.requests.lock().unwrap().push(path.into());
    let timestamp = headers["t"].to_str().unwrap().parse::<i64>().unwrap();
    assert_eq!(
        headers["sign"],
        ucenter::signature("nonce", timestamp, "secret")
    );
    assert_eq!(headers["appid"], "app");
    assert_eq!(headers["content-type"], "application/json; charset=utf-8");
    match path {
        "/token" => Json(json!({"token":"upstream-token"})).into_response(),
        "/user/login" => {
            assert_eq!(body["isuid"], 2);
            assert_eq!(headers["token"], "upstream-token");
            Json(json!({"data":{"status": if body["password"] == "valid-password" {1} else {-1}, "username":"迁移测试"}})).into_response()
        }
        "/user/get_user" => {
            assert_eq!(body["username"], "迁移测试");
            assert_eq!(body["isuid"], 0);
            Json(
                json!({"data":{"uid":"910001", "username":"迁移测试", "email":"test@example.com"}}),
            )
            .into_response()
        }
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn fixture(
    mode: &'static str,
    timeout: Duration,
) -> (Client, Fixture, tokio::task::JoinHandle<()>) {
    let fixture = Fixture {
        requests: Arc::new(Mutex::new(vec![])),
        redirected: Arc::new(AtomicUsize::new(0)),
        mode,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/api/?", listener.local_addr().unwrap());
    let router = Router::new()
        .fallback(any(upstream))
        .with_state(fixture.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (
        Client::with_settings(&endpoint, "app", "nonce", "secret", timeout).unwrap(),
        fixture,
        task,
    )
}

#[tokio::test]
async fn preserves_signed_query_endpoint_and_request_sequence() {
    let (client, state, server) = fixture("normal", Duration::from_secs(2)).await;
    let result = client
        .login("test@example.com", "valid-password")
        .await
        .unwrap();
    assert_eq!(result.uid, 910001);
    assert_eq!(result.name, "迁移测试");
    assert_eq!(
        *state.requests.lock().unwrap(),
        ["/token", "/user/login", "/token", "/user/get_user"]
    );
    assert_eq!(
        client.login("test@example.com", "invalid-password").await,
        Err(Error::InvalidCredentials)
    );
    server.abort();
}

#[tokio::test]
async fn rejects_redirects_invalid_payloads_statuses_and_total_timeout() {
    for mode in ["redirect", "invalid", "error", "slow"] {
        let (client, state, server) = fixture(mode, Duration::from_millis(120)).await;
        let started = Instant::now();
        assert_eq!(
            client.login("test@example.com", "valid-password").await,
            Err(Error::Upstream)
        );
        assert_eq!(state.redirected.load(Ordering::SeqCst), 0);
        assert!(started.elapsed() < Duration::from_secs(1));
        server.abort();
    }
}
