//! HTTP server for receiving webhooks from GitHub and Plane

use crate::config::Config;
use crate::db::Database;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{error, info};

/// Shared state for webhook handlers
#[derive(Clone)]
pub struct WebhookState {
    pub config: Config,
    pub db: Database,
}

/// Start the webhook server in the background
pub fn start_server(config: Config, db: Database) -> JoinHandle<()> {
    let state = WebhookState { config: config.clone(), db };

    tokio::spawn(async move {
        let app = Router::new()
            .route("/health", get(health))
            .route("/webhooks/github", post(github_webhook))
            .route("/webhooks/plane", post(plane_webhook))
            .with_state(Arc::new(state));

        let addr = format!("{}:{}", config.webhooks.host, config.webhooks.port);
        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to bind webhook server to {}: {}", addr, e);
                return;
            }
        };

        info!("Webhook server listening on {}", addr);

        if let Err(e) = axum::serve(listener, app).await {
            error!("Webhook server error: {}", e);
        }
    })
}

/// Health check endpoint
async fn health() -> &'static str {
    "OK"
}

/// GitHub webhook payload (simplified)
#[derive(Deserialize)]
struct GitHubPayload {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    repository: Option<GitHubRepo>,
}

#[derive(Deserialize)]
struct GitHubRepo {
    full_name: String,
}

/// Handle GitHub webhooks
async fn github_webhook(
    State(_state): State<Arc<WebhookState>>,
    Json(payload): Json<GitHubPayload>,
) -> StatusCode {
    // TODO: Verify webhook signature

    let repo_name = payload
        .repository
        .as_ref()
        .map(|r| r.full_name.as_str())
        .unwrap_or("unknown");

    let action = payload.action.as_deref().unwrap_or("unknown");

    info!("GitHub webhook: {} on {}", action, repo_name);

    // TODO: Look up watching channels and post notifications

    StatusCode::OK
}

/// Plane webhook payload (simplified)
#[derive(Deserialize)]
struct PlanePayload {
    #[serde(default)]
    event: Option<String>,
    #[serde(default)]
    project: Option<String>,
}

/// Handle Plane webhooks
async fn plane_webhook(
    State(_state): State<Arc<WebhookState>>,
    Json(payload): Json<PlanePayload>,
) -> StatusCode {
    let project = payload.project.as_deref().unwrap_or("unknown");
    let event = payload.event.as_deref().unwrap_or("unknown");

    info!("Plane webhook: {} on {}", event, project);

    // TODO: Look up watching channels and post notifications

    StatusCode::OK
}
