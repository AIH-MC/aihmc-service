use crate::config::RuntimeConfig;
use sqlx::{
    MySqlPool,
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
};
use std::sync::{Arc, Weak};

pub async fn open(runtime: &RuntimeConfig) -> Result<Arc<MySqlPool>, String> {
    let config = &runtime.config.database;
    let options = MySqlConnectOptions::new()
        .host(&config.host)
        .port(config.port)
        .username(&config.user)
        .password(&runtime.database_password)
        .database(&config.name)
        .charset(&config.charset)
        .collation(&config.collation);
    // Keep SQL statements and connection details out of application logs.
    use sqlx::ConnectOptions;
    // A weak reference lets the release hook inspect idle count without keeping
    // the pool alive through its own callback. Go's max_idle=0 closes on release.
    let max_idle = config.max_idle_connections as usize;
    let pool = Arc::new_cyclic(|weak: &Weak<MySqlPool>| {
        let weak = weak.clone();
        MySqlPoolOptions::new()
            .max_connections(config.max_open_connections)
            .acquire_timeout(config.connect_timeout.0)
            .max_lifetime(None)
            .idle_timeout(None)
            .after_release(move |_, _| {
                let keep = weak
                    .upgrade()
                    .is_some_and(|pool| pool.num_idle() < max_idle);
                Box::pin(async move { Ok(keep) })
            })
            .connect_lazy_with(options.disable_statement_logging())
    });
    tokio::time::timeout(config.connect_timeout.0, async {
        use sqlx::Connection;
        let mut connection = pool.acquire().await?;
        connection.ping().await
    })
    .await
    .map_err(|_| "cannot connect to configured MySQL or MariaDB instance")?
    .map_err(|_| "cannot connect to configured MySQL or MariaDB instance")?;
    Ok(pool)
}
