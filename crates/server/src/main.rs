use std::sync::Arc;

use anyhow::{Context, Result};
use tendhost_kernel::SystemClock;
use tendhost_server::config::Config;
use tendhost_store::Store;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Config::from_env()?;
    std::fs::create_dir_all(&config.data_dir)
        .with_context(|| format!("creating the data directory {}", config.data_dir.display()))?;
    let store = Store::open(&config.data_dir)?;
    let settings = tendhost_identity::Settings {
        secure_cookies: config.served_over_https(),
    };
    let app = tendhost_server::assemble(&store, Arc::new(SystemClock), settings)?;

    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .with_context(|| format!("listening on {}", config.listen))?;
    tracing::info!(url = %config.url(), data = %config.data_dir.display(), "tendhost is up");
    if let Some(token) = &app.setup_token {
        // printed, not logged: it must be readable on a bare terminal and
        // should not end up in a log collector
        println!(
            "\nNo account exists yet. Open {} and enter this setup token:\n\n    {token}\n",
            config.url()
        );
    }

    axum::serve(listener, app.router)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}
