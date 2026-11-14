//! Controlled migration harness; never included in release packages.
use std::{path::Path, sync::Arc};

use aihmc_api::{
    auth::{Material, MaterialFuture, MaterialProvider, Service},
    config::RuntimeConfig,
    database, httpapi, server,
    store::Store,
    texture, ucenter,
};

struct FixtureMaterials;

impl MaterialProvider for FixtureMaterials {
    fn resolve<'a>(&'a self, _: &'a str) -> MaterialFuture<'a> {
        Box::pin(async {
            Ok(Material {
                skin: "fixture-skin".into(),
                model: "slim".into(),
                cape: "fixture-cape".into(),
            })
        })
    }
}

#[tokio::main]
async fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("synthetic runtime config required");
    let runtime = Arc::new(RuntimeConfig::load(Path::new(&path)).expect("valid synthetic config"));
    assert_eq!(
        runtime.config.database.name, "aihmc_migration_test",
        "fixture refuses business database"
    );
    let pool = database::open(&runtime)
        .await
        .expect("existing isolated database");
    let store = Store::new(pool, runtime.config.database.query_timeout.0);
    let users = ucenter::Client::new(&runtime).expect("UCenter client");
    let listen = runtime.config.server.listen.clone();
    let avatars = Arc::new(
        texture::Service::new(&runtime.config, Arc::new(store.clone())).expect("avatar dependency"),
    );
    let service = Arc::new(Service::new(
        runtime.clone(),
        store,
        users,
        Arc::new(FixtureMaterials),
    ));
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .expect("fixture listener");
    println!("fixture listening {}", listener.local_addr().unwrap());
    server::serve(
        listener,
        httpapi::router(service, avatars),
        &runtime.config.server,
        std::future::pending(),
    )
    .await
    .expect("fixture HTTP server");
}
