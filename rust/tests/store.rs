use std::{path::Path, time::Duration};

use aihmc_api::{
    config::RuntimeConfig,
    database,
    store::{self, Error, Profile, Queries, Store, Texture, Token},
};

async fn seed(
    queries: &mut Queries<'_>,
    uid: i64,
    name: &str,
    uuid: &str,
    access: &str,
    client: &str,
) -> store::Result<()> {
    queries
        .save_profile(&Profile {
            uid,
            name: name.into(),
            uuid: uuid.into(),
            profile_id: format!("profile-{uid}"),
            texture_data: "e30=".into(),
        })
        .await?;
    queries
        .save_texture(&Texture {
            uid,
            name: name.into(),
            uuid: uuid.into(),
            skin: "fixture-skin".into(),
            model: "slim".into(),
            cape: "".into(),
            timestamp: 1_700_000_000,
        })
        .await?;
    queries
        .bind_token(&Token {
            uid,
            name: name.into(),
            access_token: access.into(),
            client_token: client.into(),
            created_at: 1_700_000_000,
        })
        .await
}

fn runtime() -> RuntimeConfig {
    let path = std::env::var("AIHMC_TEST_CONFIG")
        .expect("AIHMC_TEST_CONFIG required for real database tests");
    let runtime = RuntimeConfig::load(Path::new(&path)).unwrap();
    assert_eq!(
        runtime.config.database.name, "aihmc_migration_test",
        "refusing writes outside isolated test database"
    );
    runtime
}

async fn clean(pool: &sqlx::MySqlPool) {
    sqlx::query("DELETE FROM mcauth_logins WHERE server_id = 'rust-store-test'")
        .execute(pool)
        .await
        .unwrap();
    for statement in [
        "DELETE FROM mcauth_tokens WHERE uid BETWEEN 960001 AND 960004",
        "DELETE FROM mcauth_profiles WHERE uid BETWEEN 960001 AND 960004",
        "DELETE FROM mcauth_textures WHERE uid BETWEEN 960001 AND 960004",
    ] {
        sqlx::query(statement).execute(pool).await.unwrap();
    }
    sqlx::query("DELETE FROM texture_apis WHERE id IN (960001, 960002)")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM mcauth_skin_urls WHERE id IN (960001, 960002)")
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires existing MySQL/MariaDB and isolated AIHMC_TEST_CONFIG"]
async fn existing_database_transactions_identity_queries_and_timeouts() {
    let runtime = runtime();
    let pool = database::open(&runtime).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT DATABASE()")
            .fetch_one(&*pool)
            .await
            .unwrap(),
        "aihmc_migration_test"
    );
    for statement in include_str!("../../tests/schema.sql")
        .split(';')
        .filter(|sql| !sql.trim().is_empty())
    {
        // This text comes only from the compiled-in, reviewed isolated schema.
        let mut query = sqlx::QueryBuilder::<sqlx::MySql>::new(statement.replacen(
            "CREATE TABLE `",
            "CREATE TABLE IF NOT EXISTS `",
            1,
        ));
        query.build().execute(&*pool).await.unwrap();
    }
    clean(&pool).await;
    let store = Store::new(pool.clone(), Duration::from_secs(5));

    sqlx::query("INSERT INTO mcauth_profiles (uid) VALUES (960003)")
        .execute(&*pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO mcauth_textures (uid) VALUES (960003)")
        .execute(&*pool)
        .await
        .unwrap();
    store
        .read(|mut q| {
            Box::pin(async move {
                let profile = q.profile_by_uid(960003, false).await?.unwrap();
                assert_eq!(
                    (
                        profile.name,
                        profile.uuid,
                        profile.profile_id,
                        profile.texture_data
                    ),
                    (String::new(), String::new(), String::new(), String::new()),
                );
                let texture = q.texture_by_uid(960003).await?.unwrap();
                assert!(
                    texture.skin.is_empty() && texture.cape.is_empty() && texture.model.is_empty()
                );
                assert_eq!(texture.timestamp, 0);
                Ok(())
            })
        })
        .await
        .unwrap();

    let failed = store
        .transaction::<_, Error>(|mut q| {
            Box::pin(async move {
                seed(
                    &mut q,
                    960001,
                    "RustRollback",
                    "11111111111141118111111111960001",
                    "rust-access-one",
                    "rust-client",
                )
                .await?;
                Err::<(), _>(Error::Timeout)
            })
        })
        .await;
    assert!(matches!(failed, Err(Error::Timeout)));
    store
        .read(|mut q| {
            Box::pin(async move {
                assert!(q.profile_by_uid(960001, false).await?.is_none());
                assert!(q.token_by_uid(960001).await?.is_none());
                assert!(q.texture_by_uid(960001).await?.is_none());
                Ok(())
            })
        })
        .await
        .unwrap();

    store
        .transaction::<_, Error>(|mut q| {
            Box::pin(async move {
                seed(
                    &mut q,
                    960001,
                    "迁移Rust",
                    "11111111111141118111111111960001",
                    "rust-access-one",
                    "rust-client",
                )
                .await?;
                seed(
                    &mut q,
                    960002,
                    "RustOther",
                    "11111111111141118111111111960002",
                    "rust-access-two",
                    "rust-client",
                )
                .await?;
                q.record_join(
                    "rust-store-test",
                    "rust-access-one",
                    "迁移Rust",
                    "198.51.100.1",
                    1_700_000_000,
                )
                .await
            })
        })
        .await
        .unwrap();

    store
        .transaction::<_, Error>(|mut q| {
            Box::pin(async move {
                let mut profile = q.profile_by_uid(960001, true).await?.unwrap();
                profile.name = "must-not-change-identity".into();
                profile.uuid = "must-not-change-uuid".into();
                profile.texture_data = "bmV3".into();
                q.save_profile(&profile).await?;
                q.bind_token(&Token {
                    uid: 960001,
                    name: "must-not-change-name".into(),
                    access_token: "must-not-replace-token".into(),
                    client_token: "rust-client".into(),
                    created_at: 1_700_000_001,
                })
                .await?;
                Ok(())
            })
        })
        .await
        .unwrap();

    store
        .read(|mut q| {
            Box::pin(async move {
                let profile = q.profile("迁移Rust").await?.unwrap();
                assert_eq!(profile.name, "迁移Rust");
                assert_eq!(profile.uuid, "11111111111141118111111111960001");
                assert_eq!(profile.texture_data, "bmV3");
                assert_eq!(q.profile_by_uuid(&profile.uuid).await?.unwrap(), profile);
                assert_eq!(q.profile_by_name("迁移Rust").await?.unwrap(), profile);
                assert!(q.profile_by_name("' OR '1'='1").await?.is_none());
                let token = q.token_by_uid(960001).await?.unwrap();
                assert_eq!(token.access_token, "rust-access-one");
                assert_eq!(token.name, "迁移Rust");
                assert_eq!(q.token_by_access("rust-access-one").await?.unwrap(), token);
                assert_eq!(
                    q.token("rust-access-one", "rust-client", false)
                        .await?
                        .unwrap(),
                    token
                );
                assert!(q.has_joined("迁移Rust", "rust-store-test").await?);
                Ok(())
            })
        })
        .await
        .unwrap();

    let conflicting = store
        .transaction::<_, Error>(|mut q| {
            Box::pin(async move {
                q.refresh("rust-client", "ambiguous-new-access").await?;
                Ok(())
            })
        })
        .await;
    assert!(matches!(conflicting, Err(Error::Database(_))));
    let conflicting_identity = store
        .transaction::<_, Error>(|mut q| {
            Box::pin(async move {
                q.save_profile(&Profile {
                    uid: 960004,
                    name: "迁移Rust".into(),
                    profile_id: "must-not-create".into(),
                    uuid: "must-not-create".into(),
                    texture_data: "must-not-overwrite".into(),
                })
                .await
            })
        })
        .await;
    assert!(matches!(conflicting_identity, Err(Error::Database(_))));
    store
        .read(|mut q| {
            Box::pin(async move {
                assert!(
                    q.token("rust-access-one", "rust-client", false)
                        .await?
                        .is_some()
                );
                assert!(
                    q.token("rust-access-two", "rust-client", false)
                        .await?
                        .is_some()
                );
                Ok(())
            })
        })
        .await
        .unwrap();

    store
        .read(|mut q| {
            Box::pin(async move {
                assert!(q.profile_by_uid(960004, false).await?.is_none());
                assert_eq!(
                    q.profile_by_uid(960001, false).await?.unwrap().texture_data,
                    "bmV3"
                );
                Ok(())
            })
        })
        .await
        .unwrap();

    let rollback = store
        .transaction::<_, Error>(|mut q| {
            Box::pin(async move {
                assert!(q.delete_user(960001).await?);
                Err::<(), _>(Error::Timeout)
            })
        })
        .await;
    assert!(matches!(rollback, Err(Error::Timeout)));
    store
        .read(|mut q| {
            Box::pin(async move {
                assert!(q.has_joined("迁移Rust", "rust-store-test").await?);
                assert!(q.token_by_uid(960001).await?.is_some());
                Ok(())
            })
        })
        .await
        .unwrap();

    // Force a real row lock wait; the store deadline must cancel and roll back.
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("SELECT uid FROM mcauth_tokens WHERE uid = 960001 FOR UPDATE")
        .fetch_one(&mut *lock)
        .await
        .unwrap();
    let short_store = Store::new(pool.clone(), Duration::from_millis(40));
    let timeout = short_store
        .transaction::<_, Error>(|mut q| {
            Box::pin(async move {
                q.token("rust-access-one", "rust-client", true).await?;
                Ok(())
            })
        })
        .await;
    assert!(matches!(timeout, Err(Error::Timeout)));
    lock.rollback().await.unwrap();

    sqlx::query("INSERT INTO texture_apis (id, api_url, added_by, added_date) VALUES (960001, 'http://z-rust.invalid', 'fixture', 0), (960002, 'http://a-rust.invalid', 'fixture', 0)").execute(store.pool()).await.unwrap();
    sqlx::query("INSERT INTO mcauth_skin_urls (id, skin_url, added_by, added_date) VALUES (960001, 'z-rust.invalid', 'fixture', '0'), (960002, 'a-rust.invalid', 'fixture', '0')").execute(store.pool()).await.unwrap();
    store
        .read(|mut q| {
            Box::pin(async move {
                let apis = q.skin_apis().await?;
                assert!(
                    apis.iter().position(|url| url == "http://z-rust.invalid")
                        < apis.iter().position(|url| url == "http://a-rust.invalid")
                );
                let domains = q.skin_domains().await?;
                assert!(
                    domains.iter().position(|url| url == "z-rust.invalid")
                        < domains.iter().position(|url| url == "a-rust.invalid")
                );
                Ok(())
            })
        })
        .await
        .unwrap();

    store
        .transaction::<_, Error>(|mut q| {
            Box::pin(async move {
                assert!(q.delete_user(960001).await?);
                assert!(!q.delete_user(960001).await?);
                q.invalidate("missing", "missing").await?;
                q.signout(960002).await?;
                assert!(q.profile_by_uid(960001, false).await?.is_none());
                assert!(q.texture_by_uid(960001).await?.is_none());
                assert!(q.token_by_uid(960001).await?.is_none());
                assert!(!q.has_joined("迁移Rust", "rust-store-test").await?);
                assert!(q.token_by_uid(960002).await?.is_none());
                Ok(())
            })
        })
        .await
        .unwrap();
    store
        .transaction::<_, Error>(|mut q| {
            Box::pin(async move {
                seed(
                    &mut q,
                    960001,
                    "RustRefresh",
                    "11111111111141118111111111960001",
                    "rust-refresh-old",
                    "rust-refresh-client",
                )
                .await?;
                let token = q
                    .refresh("rust-refresh-client", "rust-refreshed-access")
                    .await?
                    .unwrap();
                assert_eq!(token.created_at, 1_700_000_000);
                assert!(q.token_by_access("rust-refresh-old").await?.is_none());
                q.record_join(
                    "rust-store-test",
                    "rust-refreshed-access",
                    "RustRefresh",
                    "198.51.100.1",
                    1_700_000_000,
                )
                .await?;
                q.delete_old_joins("RustRefresh", 1_700_000_001).await?;
                assert!(!q.has_joined("RustRefresh", "rust-store-test").await?);
                q.invalidate("rust-refreshed-access", "rust-refresh-client")
                    .await?;
                assert!(q.token_by_uid(960001).await?.is_none());
                Ok(())
            })
        })
        .await
        .unwrap();
    clean(&pool).await;
    pool.close().await;
}

#[tokio::test]
#[ignore = "requires existing MySQL/MariaDB and isolated AIHMC_TEST_CONFIG"]
async fn connection_pool_honors_zero_idle_limit() {
    let mut runtime = runtime();
    runtime.config.database.max_idle_connections = 0;
    let pool = database::open(&runtime).await.unwrap();
    let connection = pool.acquire().await.unwrap();
    drop(connection);
    tokio::time::timeout(Duration::from_secs(3), async {
        while pool.size() != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("max_idle=0 retained an idle connection");
    assert_eq!(pool.num_idle(), 0);
    pool.close().await;
}
