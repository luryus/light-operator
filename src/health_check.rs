use std::{net::SocketAddr, sync::Arc};

use anyhow::{Context, Error};
use axum::{routing::get, Router, Server};

use crate::config::Config;

pub async fn run(config: Arc<Config>) -> Result<(), Error> {
    let app = Router::new().route("/healthz", get(())); // Always succeed

    let addr = SocketAddr::from(([127, 0, 0, 1], config.health_check.port));
    Server::try_bind(&addr)?
        .serve(app.into_make_service())
        .await
        .context("Health check server failed")
}
