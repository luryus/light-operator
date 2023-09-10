use std::sync::Arc;

use async_trait::async_trait;

use crate::config::{Config, SmartHomePlatform};

use self::smartthings::SmartThings;

mod smartthings;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Invalid ID `{0}`")]
    InvalidId(String),

    #[error("Request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Device was not found")]
    UnknownDeviceId,
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Color {
    pub hue: u8,
    pub saturation: u8,
}

#[derive(Debug)]
pub enum LightStatus {
    Offline,
    Online(LightOptions),
}

#[derive(Debug)]
pub struct LightOptions {
    pub switched_on: bool,
    pub brightness: Option<f32>,
    pub color_temperature: Option<u16>,
    pub color: Option<Color>,
}

#[async_trait]
pub trait SmartHomeApi: Send + Sync {
    async fn get_light_status(&self, id: &str) -> Result<LightStatus>;

    async fn set_switched_on(&self, id: &str, switched_on: bool) -> Result<()>;

    async fn set_brightness(&self, id: &str, brightness: f32) -> Result<()>;

    async fn set_color_temperature(&self, id: &str, color_temp: u16) -> Result<()>;

    async fn set_color(&self, id: &str, hue: u8, saturation: u8) -> Result<()>;
}

pub fn get_smart_home_api(config: Arc<Config>) -> Result<Arc<dyn SmartHomeApi + Send + Sync>> {
    match config.smart_home.platform {
        SmartHomePlatform::SmartThings => {
            let thing_res = SmartThings::new(config);
            let arc_smartthings = thing_res.map(Arc::new)?;
            Ok(arc_smartthings)
        }
    }
}
