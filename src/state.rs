use std::{error::Error, path::PathBuf, sync::Arc, time::Duration};

use sqlx::{
    MySqlPool,
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
};

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub pool: MySqlPool,
    pub client: reqwest::Client,
    pub directory: PathBuf,
}

impl AppState {
    pub fn new(config: Config, directory: PathBuf) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let db = &config.database;
        let options = MySqlConnectOptions::new()
            .host(&db.host)
            .port(db.port)
            .username(&db.user)
            .password(&db.password)
            .database(&db.db_name)
            .charset("utf8mb4");
        let pool = MySqlPoolOptions::new().connect_lazy_with(options);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs_f64(config.server.timeout))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            config: Arc::new(config),
            pool,
            client,
            directory,
        })
    }

    pub fn resource(&self, path: &str) -> PathBuf {
        let path = std::path::Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.directory.join(path)
        }
    }
}
