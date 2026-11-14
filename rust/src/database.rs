use crate::config::RuntimeConfig;
use sqlx::{
    MySqlPool,
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
};

pub async fn open(runtime: &RuntimeConfig) -> Result<MySqlPool, String> {
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
    let pool = tokio::time::timeout(
        config.connect_timeout.0,
        MySqlPoolOptions::new()
            .max_connections(config.max_open_connections)
            .acquire_timeout(config.connect_timeout.0)
            .connect_with(options.disable_statement_logging()),
    )
    .await
    .map_err(|_| "cannot connect to configured MySQL or MariaDB instance")?
    .map_err(|_| "cannot connect to configured MySQL or MariaDB instance")?;
    Ok(pool)
}
