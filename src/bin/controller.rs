use anyhow::{anyhow, Context};
use config::{Environment, File, FileFormat};
use light_operator::kubernetes::controller::run;
use light_operator::smarthome;
use light_operator::{config::Config, health_check};
use std::sync::Arc;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};
use std::future::pending;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let conf = config::Config::builder()
        .add_source(File::new("config", FileFormat::Yaml).required(true))
        .add_source(File::new("config.local", FileFormat::Yaml).required(false))
        .add_source(Environment::default().separator("__").prefix("LO"))
        .build()
        .expect("Configuration parsing failed");

    let config: Config = conf.try_deserialize().unwrap();
    let config = Arc::new(config);

    let logger = tracing_subscriber::fmt::layer().pretty();
    let env_filter = EnvFilter::try_new(&config.log.filters).unwrap();
    let collector = Registry::default().with(logger).with(env_filter);
    tracing::subscriber::set_global_default(collector).unwrap();

    let smart_home_api =
        smarthome::get_smart_home_api(config.clone()).context("Smart home API init failed")?;

    let health_join_handle = if config.health_check.enable_server {
        let c = config.clone();
        tokio::spawn(health_check::run(c))
    } else {
        tokio::spawn(pending())
    };

    tracing::info!("Starting controller");
    let controller_join_handle = tokio::spawn(run(config, smart_home_api));

    tokio::select! {
        res = controller_join_handle => match res {
            Ok(inner) => inner.context("Controller error"),
            Err(e) => Err(anyhow!(e))
        },
        res = health_join_handle => match res {
            Ok(inner) => inner.context("Health check server error"),
            Err(e) => Err(anyhow!(e))
        },
        _ = tokio::signal::ctrl_c() => Ok(())
    }?;

    tracing::info!("Exiting...");
    Ok(())
}
