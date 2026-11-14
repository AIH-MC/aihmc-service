use std::{path::PathBuf, sync::Arc};

use aihmc_api::{
    auth, config::RuntimeConfig, database, httpapi, server, store::Store, texture, ucenter,
};
use tracing::Level;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("startup failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let mut path = PathBuf::from("config.toml");
    let mut check = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--help" | "-h" | "-help" => {
                println!(
                    "Usage: aihmc-api [--config <path>] [--check-config]\n  --config <path>   runtime TOML file (default: config.toml)\n  --check-config    validate configuration, keys, textures and database, then exit"
                );
                return Ok(());
            }
            "--config" | "-config" => {
                path = arguments.next().ok_or("--config requires a path")?.into()
            }
            "--check-config" | "-check-config" => check = true,
            _ if argument.starts_with("--config=") || argument.starts_with("-config=") => {
                path = argument.split_once('=').unwrap().1.into();
            }
            _ => return Err("unknown command-line argument; use --help".into()),
        }
    }
    let runtime = Arc::new(RuntimeConfig::load(&path)?);
    let pool = database::open(&runtime).await?;
    if check {
        pool.close().await;
        println!("configuration, key pair, texture directory and database connection validated");
        return Ok(());
    }
    let level = runtime
        .config
        .logging
        .level
        .parse::<Level>()
        .map_err(|_| "invalid logging.level")?;
    let logger = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(std::io::stdout);
    if runtime.config.logging.format == "json" {
        logger.json().init();
    } else {
        logger.with_ansi(false).init();
    }
    let store = Store::new(pool.clone(), runtime.config.database.query_timeout.0);
    let materials = Arc::new(
        texture::Service::new(&runtime.config, Arc::new(store.clone()))
            .map_err(|_| "cannot initialize material client")?,
    );
    let users = ucenter::Client::new(&runtime).map_err(|_| "cannot initialize UCenter client")?;
    let service = Arc::new(auth::Service::new(
        runtime.clone(),
        store,
        users,
        materials.clone(),
    ));
    let app = httpapi::router(service, materials);
    let listen = if runtime.config.server.listen.starts_with(':') {
        format!("0.0.0.0{}", runtime.config.server.listen)
    } else {
        runtime.config.server.listen.clone()
    };
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .map_err(|_| "cannot listen on server.listen")?;
    tracing::info!(address = %listener.local_addr().map_err(|_| "cannot query listener address")?, "server started");
    server::serve(listener, app, &runtime.config.server, shutdown_signal()).await?;
    pool.close().await;
    tracing::info!("server stopped");
    Ok(())
}

async fn shutdown_signal() -> Result<(), String> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate =
            signal(SignalKind::terminate()).map_err(|_| "cannot register SIGTERM handler")?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result.map_err(|_| "cannot receive interrupt")?,
            _ = terminate.recv() => {},
        }
    }
    #[cfg(windows)]
    {
        let mut interrupt = tokio::signal::windows::ctrl_c()
            .map_err(|_| "cannot register console interrupt handler")?;
        let mut console_break = tokio::signal::windows::ctrl_break()
            .map_err(|_| "cannot register console break handler")?;
        tokio::select! {
            _ = interrupt.recv() => {},
            _ = console_break.recv() => {},
        }
    }
    Ok(())
}
