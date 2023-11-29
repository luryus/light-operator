use std::{net::SocketAddr, sync::Arc};

use anyhow::{Context, Error};
use axum::{routing::get, Router};

use crate::config::Config;

pub async fn run(config: Arc<Config>) -> Result<(), Error> {
    let app = Router::new().route("/healthz", get(())); // Always succeed

    let addr = SocketAddr::from(([0, 0, 0, 0], config.health_check.port));
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .await
        .context("Health check server failed")
}
