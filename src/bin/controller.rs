use std::sync::Arc;
use light_operator::kubernetes::{crd::Light, controller::run};
use config::{Config, Environment};
use config::{File, FileFormat};

#[tokio::main]
async fn main() -> Result<(), kube::Error> {

    Config::builder()
    .add_source(File::new("config.yaml", FileFormat::Yaml))
    .add_source(Environment::default());

    run().await
}
