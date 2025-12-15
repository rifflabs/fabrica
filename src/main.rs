//! Palace Fabrica - Coordination infrastructure for Riff Labs
//!
//! A unified Discord bot handling:
//! - Translation (Hindi <-> English)
//! - Status/availability tracking
//! - Plane project visibility
//! - GitHub activity notifications

use anyhow::Result;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod bot;
mod config;
mod db;
mod modules;
mod services;
mod webhooks;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if it exists
    if std::path::Path::new(".env").exists() {
        dotenvy::dotenv()?;
        info!("Loaded environment variables from .env file");
    }

    // Initialize logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Palace Fabrica starting...");

    // Load configuration
    let config = config::Config::load()?;
    info!("Configuration loaded");
    info!("OpenRouter URL: {}", config.translation.openrouter_url);
    info!("OpenRouter API key: {}", if config.translation.openrouter_api_key.is_empty() { "EMPTY" } else { "SET" });
    info!("Translation model: {}", config.translation.model);
    info!("Translation backend: {}", config.translation.backend);

    // Initialize database
    let db = db::Database::new(&config.database.path).await?;
    db.migrate().await?;
    info!("Database initialized");

    // Start webhook server in background
    let webhook_handle = webhooks::start_server(config.clone(), db.clone());
    info!("Webhook server starting on port {}", config.webhooks.port);

    // Start Discord bot (blocks)
    info!("Starting Discord bot...");
    bot::run(config, db).await?;

    // Clean shutdown
    webhook_handle.abort();
    info!("Palace Fabrica shutting down");

    Ok(())
}
