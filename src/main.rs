use std::{error::Error, path::PathBuf};

use aihmc_auth::{config::Config, http, state::AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let mut arguments = std::env::args().skip(1);
    let config_path = match arguments.next().as_deref() {
        None => PathBuf::from("config.toml"),
        Some("--config") => PathBuf::from(arguments.next().ok_or("--config requires a path")?),
        _ => return Err("usage: aihmc-auth [--config PATH]".into()),
    };
    if arguments.next().is_some() {
        return Err("unexpected command-line argument".into());
    }
    let config_path = config_path.canonicalize()?;
    let config = Config::load(&config_path)?;
    let listener =
        tokio::net::TcpListener::bind((config.server.host.as_str(), config.server.port)).await?;
    tracing::info!(address = %listener.local_addr()?, "listening");
    let state = AppState::new(config, config_path.parent().unwrap().to_path_buf())?;
    axum::serve(
        listener,
        http::router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}
