use anyhow::Context;
use config::{Environment, File, FileFormat};
use light_operator::config::Config;
use light_operator::kubernetes::controller::run;
use light_operator::smarthome;
use std::sync::Arc;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let conf = config::Config::builder()
        .add_source(File::new("config", FileFormat::Yaml).required(true))
        .add_source(File::new("config.local", FileFormat::Yaml).required(false))
        .add_source(Environment::default().separator("__").prefix("LO"))
        .build()
        .expect("Configuration parsing failed");

    println!("Config: {:#?}", conf);
    let config: Config = conf.try_deserialize().unwrap();
    let config = Arc::new(config);

    let logger = tracing_subscriber::fmt::layer().pretty();
    let env_filter = EnvFilter::try_new(&config.log.filters).unwrap();
    let collector = Registry::default().with(logger).with(env_filter);
    tracing::subscriber::set_global_default(collector).unwrap();

    let smart_home_api =
        smarthome::get_smart_home_api(config.clone()).context("Smart home API init failed")?;

    tracing::info!("Starting controller");
    run(config, smart_home_api)
        .await
        .context("Controller error")
}
