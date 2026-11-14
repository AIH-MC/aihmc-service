use std::path::PathBuf;

use aihmc_api::{config::RuntimeConfig, database};
use axum::{Json, Router, routing::any};
use serde_json::json;
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
    let runtime = RuntimeConfig::load(&path)?;
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
    // R1 exposes only the existing root notice. Business routes follow in R3/R4.
    let app = Router::new().route("/", any(|| async {
        Json(json!({"message": "This is AI Hobbyist MC API, please use the correct endpoint to access the API."}))
    }));
    let listen = if runtime.config.server.listen.starts_with(':') {
        format!("0.0.0.0{}", runtime.config.server.listen)
    } else {
        runtime.config.server.listen.clone()
    };
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .map_err(|_| "cannot listen on server.listen")?;
    tracing::info!(address = %listener.local_addr().map_err(|_| "cannot query listener address")?, "server started");
    let (stop_sender, stop_receiver) = tokio::sync::oneshot::channel();
    let server = axum::serve(listener, app).with_graceful_shutdown(async {
        let _ = stop_receiver.await;
    });
    let server = std::future::IntoFuture::into_future(server);
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => result.map_err(|_| "HTTP server failed")?,
        result = shutdown_signal() => {
            result?;
            let _ = stop_sender.send(());
            tokio::time::timeout(runtime.config.server.shutdown_timeout.0, &mut server).await
                .map_err(|_| "graceful shutdown timed out")?
                .map_err(|_| "HTTP server failed")?;
        }
    }
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
