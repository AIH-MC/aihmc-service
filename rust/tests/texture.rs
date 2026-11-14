use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use aihmc_api::{
    auth::Error,
    config::{Config, PositiveDuration},
    store,
    texture::{Repository, RepositoryFuture, Service},
};
use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Response, StatusCode, Uri},
    response::IntoResponse,
    routing::get,
};
use serde_json::json;
use sha2::{Digest, Sha256};

const SKIN: &[u8] = include_bytes!("../../tests/testdata/skin-64.png");
const AVATAR: &[u8] = include_bytes!("../../tests/testdata/avatar-64-20.png");

struct FixtureRepository {
    apis: Vec<String>,
    profile: bool,
    fail: bool,
}

impl Repository for FixtureRepository {
    fn skin_apis(&self) -> RepositoryFuture<'_, Vec<String>> {
        Box::pin(async move {
            if self.fail {
                Err(store::Error::Timeout)
            } else {
                Ok(self.apis.clone())
            }
        })
    }

    fn profile_by_uuid<'a>(&'a self, _: &'a str) -> RepositoryFuture<'a, Option<store::Profile>> {
        Box::pin(async move {
            Ok(self.profile.then_some(store::Profile {
                uid: 1,
                name: "Fixture".into(),
                profile_id: "profile".into(),
                uuid: "local-uuid".into(),
                texture_data: String::new(),
            }))
        })
    }
}

#[derive(Clone)]
struct Upstream {
    mode: &'static str,
    calls: Arc<Mutex<Vec<String>>>,
}

async fn upstream(State(state): State<Upstream>, uri: Uri) -> Response<Body> {
    let path = uri.path();
    state.calls.lock().unwrap().push(uri.to_string());
    if state.mode == "redirect" {
        return (StatusCode::FOUND, [("location", "/redirected")]).into_response();
    }
    if state.mode == "missing" {
        return StatusCode::NOT_FOUND.into_response();
    }
    if path.contains("/csl/") {
        if state.mode == "fallback" || path.starts_with("/first/") {
            return StatusCode::NOT_FOUND.into_response();
        }
        if state.mode == "malformed" {
            return "{".into_response();
        }
        let response = match state.mode {
            "invalid-key" => json!({"skins":{"default":"../outside"}}),
            "corrupt" => json!({"skins":{"default":"broken"}}),
            "slow-cape" => json!({"skins":{"slim":"remote-skin"}, "cape":"slow-cape"}),
            "ordered" => json!({"skins":{"slim":"remote-skin"}}),
            _ => json!({"skins":{"default":"remote-skin"}, "cape":"remote-cape"}),
        };
        return axum::Json(response).into_response();
    }
    if path.ends_with("/textures/broken") {
        return "not PNG".into_response();
    }
    if path.ends_with("/textures/slow-cape") {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    if path.contains("/textures/") {
        return SKIN.to_vec().into_response();
    }
    if path.starts_with("/users/profiles/minecraft/")
        && matches!(state.mode, "ordered" | "fallback")
    {
        return axum::Json(json!({"id":"fallback-uuid"})).into_response();
    }
    if path == "/skins/fallback-uuid" || (path == "/capes/fallback-uuid" && state.mode == "ordered")
    {
        return SKIN.to_vec().into_response();
    }
    if path == "/helm/missing-uuid/20" {
        return AVATAR.to_vec().into_response();
    }
    if path == "/helm/corrupt/20" {
        return "not PNG".into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

struct Fixture {
    service: Arc<Service>,
    directory: tempfile::TempDir,
    calls: Arc<Mutex<Vec<String>>>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn fixture(mode: &'static str, profile: bool, fail: bool) -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let calls = Arc::new(Mutex::new(vec![]));
    let state = Upstream {
        mode,
        calls: calls.clone(),
    };
    let router = Router::new().fallback(get(upstream)).with_state(state);
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let mut config = Config::parse(include_str!("../../config.example.toml")).unwrap();
    config.textures.directory = directory.path().into();
    if mode == "filesystem" {
        config.textures.directory = directory.path().join("blocked");
        std::fs::write(&config.textures.directory, b"not a directory").unwrap();
    }
    config.textures.mojang_base_url = base.clone();
    config.textures.crafatar_base_url = base.clone();
    config.textures.crafthead_base_url = base.clone();
    config.textures.request_timeout =
        PositiveDuration(Duration::from_millis(if mode == "slow-cape" {
            100
        } else {
            2000
        }));
    let apis = if mode == "ordered" {
        vec![format!("{base}/first"), format!("{base}/second")]
    } else {
        vec![base]
    };
    let service = Arc::new(
        Service::new(
            &config,
            Arc::new(FixtureRepository {
                apis,
                profile,
                fail,
            }),
        )
        .unwrap(),
    );
    Fixture {
        service,
        directory,
        calls,
        server,
    }
}

#[tokio::test]
async fn preserves_source_order_query_encoding_and_missing_cape_fallback() {
    let fixture = fixture("ordered", false, false).await;
    let material = fixture
        .service
        .resolve_material("迁移用户 A")
        .await
        .unwrap();
    let cape = format!("{:x}", Sha256::digest(SKIN));
    assert_eq!(
        (
            material.skin.as_str(),
            material.model.as_str(),
            material.cape.as_str()
        ),
        ("remote-skin", "slim", cape.as_str())
    );
    let calls = fixture.calls.lock().unwrap();
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls[0],
        "/first/csl/%E8%BF%81%E7%A7%BB%E7%94%A8%E6%88%B7+A.json"
    );
    assert!(calls[1].starts_with("/second/csl/"));
    for key in [&material.skin, &material.cape] {
        assert_eq!(
            std::fs::read(fixture.directory.path().join(key)).unwrap(),
            SKIN
        );
    }
}

#[tokio::test]
async fn preserves_fallback_hash_model_and_optional_missing_cape() {
    let fixture = fixture("fallback", false, false).await;
    let material = fixture.service.resolve_material("Fixture").await.unwrap();
    assert_eq!(material.skin, format!("{:x}", Sha256::digest(SKIN)));
    assert_eq!(material.model, "default");
    assert!(material.cape.is_empty());
}

#[tokio::test]
async fn rejects_bad_remote_materials_and_does_not_follow_redirects() {
    for mode in ["missing", "malformed", "corrupt", "invalid-key", "redirect"] {
        let fixture = fixture(mode, false, false).await;
        let material = fixture.service.resolve_material("Fixture").await.unwrap();
        assert!(material.skin.is_empty(), "accepted {mode} skin");
        assert!(material.cape.is_empty(), "accepted {mode} cape");
        assert_eq!(
            std::fs::read_dir(fixture.directory.path()).unwrap().count(),
            0
        );
        assert!(
            !fixture
                .calls
                .lock()
                .unwrap()
                .iter()
                .any(|path| path == "/redirected")
        );
    }
}

#[tokio::test]
async fn remote_timeout_preserves_partial_skin_and_database_failure_is_not_optional() {
    let fixture = fixture("slow-cape", false, false).await;
    let material = fixture.service.resolve_material("Fixture").await.unwrap();
    assert_eq!(material.skin, "remote-skin");
    assert!(material.cape.is_empty());
    let failed = self::fixture("normal", false, true).await;
    assert!(matches!(
        failed.service.resolve_material("Fixture").await,
        Err(Error::Store(_))
    ));
}

#[tokio::test]
async fn concurrent_publication_is_complete_and_leaves_no_temporary_files() {
    let fixture = fixture("normal", false, false).await;
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let service = fixture.service.clone();
        tasks.spawn(async move { service.resolve_material("Fixture").await });
    }
    while let Some(result) = tasks.join_next().await {
        result.unwrap().unwrap();
    }
    assert_eq!(
        std::fs::read_dir(fixture.directory.path()).unwrap().count(),
        2
    );
    for key in ["remote-skin", "remote-cape"] {
        assert_eq!(
            std::fs::read(fixture.directory.path().join(key)).unwrap(),
            SKIN
        );
    }
}

#[tokio::test]
async fn renders_local_avatar_and_forwards_remote_png_intact() {
    let local = fixture("normal", true, false).await;
    let actual = local.service.render_avatar("local-uuid", 20).await.unwrap();
    assert_eq!(
        image::load_from_memory(&actual).unwrap().into_rgba8(),
        image::load_from_memory(AVATAR).unwrap().into_rgba8()
    );
    let remote = fixture("normal", false, false).await;
    assert_eq!(
        remote
            .service
            .render_avatar("missing-uuid", 20)
            .await
            .unwrap(),
        AVATAR
    );
    assert!(remote.service.render_avatar("corrupt", 20).await.is_err());
    assert!(remote.service.render_avatar("missing", 20).await.is_err());
    assert!(
        remote
            .service
            .render_avatar("missing-uuid", 0)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn filesystem_failure_is_reported_instead_of_optional_remote_fallback() {
    let fixture = fixture("filesystem", false, false).await;
    assert!(matches!(
        fixture.service.resolve_material("Fixture").await,
        Err(Error::Material)
    ));
}
