mod app;
mod auth;
mod authz;
mod cache;
mod config;
mod db;
mod dto;
mod error;
mod extract;
mod json_fmt;
mod logging;
mod mail;
mod routes;
mod schema;
mod storage;

#[cfg(test)]
mod tests;

use std::net::SocketAddr;

use anyhow::Context;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "serve".to_string());
    if mode == "healthcheck" {
        return healthcheck().await;
    }
    if mode != "serve" {
        anyhow::bail!("unknown command {mode:?}; expected `serve` or `healthcheck`");
    }

    config::load_dotenv();
    let config = config::Config::from_env().context("invalid configuration")?;

    let (writer, forwarder) = match &config.log_tail_source_key {
        Some(source_key) => {
            let forwarder = logging::channel();
            (
                logging::TeeWriter::new(Some(forwarder.sender.clone())),
                Some((forwarder.receiver, source_key.clone())),
            )
        }
        None => (logging::TeeWriter::new(None), None),
    };
    tracing_subscriber::fmt()
        .json()
        .with_writer(writer)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    if let Some((receiver, source_key)) = forwarder {
        let client = reqwest::Client::new();
        tokio::spawn(logging::forward_logs(
            receiver,
            config.log_tail_host.clone(),
            source_key,
            config.service_name.clone(),
            client,
        ));
    }

    let state = app::AppState::connect(config).await?;
    state.check_schema().await?;

    let addr: SocketAddr = format!("{}:{}", state.config.bind, state.config.port)
        .parse()
        .context("invalid listen address")?;
    let listener = TcpListener::bind(addr).await.context("bind failed")?;
    tracing::info!(%addr, "listening");
    axum::serve(
        listener,
        app::router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn healthcheck() -> anyhow::Result<()> {
    config::load_dotenv();
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let url = format!("http://127.0.0.1:{port}/ping");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()?;
    let response = client.get(url).send().await?;
    if response.status().is_success() {
        Ok(())
    } else {
        anyhow::bail!("healthcheck returned {}", response.status())
    }
}
