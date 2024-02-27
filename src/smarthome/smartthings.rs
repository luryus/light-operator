use std::{sync::Arc, time::Duration};

use api_models::*;
use async_trait::async_trait;
use reqwest::{header::HeaderMap, Client, Response, Url};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::config::Config;

use super::{LightOptions, LightStatus, SmartHomeApi};

const APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

const API_BASE_URL: &str = "https://api.smartthings.com/v1/";

pub struct SmartThings {
    _config: Arc<Config>,
    client: Client,
    base_url: Url,
}

macro_rules! command_json {
    ($cap:literal, $cmd:expr, $args:tt) => {
        serde_json::json!({
            "commands": [{
                "component": "main",
                "capability": $cap,
                "command": $cmd,
                "arguments": $args
            }]
        })
    };
    ($cap:literal, $cmd:expr) => {
        serde_json::json!({
            "commands": [{
                "component": "main",
                "capability": $cap,
                "command": $cmd,
                "arguments": []
            }]
        })
    };
}

#[allow(dead_code)]
fn is_recent(ts: Option<OffsetDateTime>) -> bool {
    match ts {
        None => false,
        Some(t) => (OffsetDateTime::now_utc() - t) <= time::Duration::minutes(10),
    }
}

impl SmartThings {
    pub fn new(config: Arc<Config>) -> super::Result<Self> {
        let Some(api_token) = &config.smart_home.smartthings.api_token else {
            return Err(super::Error::Configuration(
                "SmartThings API token not configured".to_string(),
            ));
        };

        let mut auth_header = HeaderMap::new();
        auth_header.append(
            "Authorization",
            format!("Bearer {}", api_token).parse().unwrap(),
        );

        let client = reqwest::ClientBuilder::new()
            .user_agent(APP_USER_AGENT)
            .connection_verbose(true)
            .use_rustls_tls()
            .default_headers(auth_header)
            .gzip(true)
            .build()
            .unwrap();

        let base_url = API_BASE_URL.try_into().unwrap();

        Ok(Self {
            _config: config,
            client,
            base_url,
        })
    }

    fn validate_device_id(id: &str) -> super::Result<()> {
        Uuid::try_parse(id).map_err(|_| super::Error::InvalidId(id.to_string()))?;
        Ok(())
    }

    fn error_from_status(res: Response) -> super::Result<Response> {
        if res.status() == 403 {
            // Not found / forbidden
            return Err(super::Error::UnknownDeviceId);
        }

        Ok(res.error_for_status()?)
    }

    async fn send_command(&self, id: &str, cmd: serde_json::Value) -> super::Result<()> {
        Self::validate_device_id(id)?;

        tracing::debug!(device_id = id, command = cmd.to_string(), "Sending command");

        let url = self
            .base_url
            .join(&format!("devices/{id}/commands"))
            .map_err(|_| super::Error::InvalidId(id.to_string()))?;

        let res = self.client.post(url).json(&cmd).send().await?;

        Self::error_from_status(res)?;
        Ok(())
    }

    async fn ping(&self, id: &str) -> super::Result<()> {
        let cmd = command_json!("healthCheck", "ping");
        self.send_command(id, cmd).await
    }
}

#[async_trait]
impl SmartHomeApi for SmartThings {
    async fn get_light_status(&self, id: &str) -> super::Result<LightStatus> {
        Self::validate_device_id(id)?;
        self.ping(id).await?;
        tokio::time::sleep(Duration::from_millis(200)).await;

        tracing::debug!("Getting status for device {id}");

        let url = self
            .base_url
            .join(&format!("devices/{id}/status"))
            .map_err(|_| super::Error::InvalidId(id.to_string()))?;

        let res = self.client.get(url).send().await?;

        let body: DeviceStatus = Self::error_from_status(res)?.json().await?;

        tracing::debug!("Got status {body:#?}");

        let online = body
            .components.main
            .health_check
            .and_then(|x| x.device_status)
            .map(|x| matches!(x.value, Some(DeviceStatusValue::Online)))
            .unwrap_or(false);

        if !online {
            return Ok(LightStatus::Offline);
        }

        let switched_on = body
            .components.main
            .switch
            //.filter(|x| is_recent(x.timestamp))
            .and_then(|x| x.value)
            .map(|x| x == SwitchState::On)
            .unwrap_or(false);

        let brightness = body
            .components.main
            .switch_level
            //.filter(|x| is_recent(x.timestamp))
            .and_then(|x| x.value)
            .and_then(|x| x.clamp(0, 100).try_into().ok());

        let color_temperature = body
            .components.main
            .color_temperature
            //.filter(|x| is_recent(x.timestamp))
            .and_then(|x| x.value)
            .and_then(|x| x.try_into().ok());

        let color = body
            .components.main
            .color_control
            //.filter(|c| is_recent(c.hue.timestamp) && is_recent(c.saturation.timestamp))
            .and_then(|x| {
                let hue = x.hue.value?.try_into().ok()?;
                let saturation = x.saturation.value?.try_into().ok()?;

                Some(super::Color { hue, saturation })
            });

        Ok(LightStatus::Online(LightOptions {
            switched_on,
            brightness,
            color_temperature,
            color,
        }))
    }

    async fn set_switched_on(&self, id: &str, switched_on: bool) -> super::Result<()> {
        let cmd = command_json!("switch", if switched_on { "on" } else { "off" });
        self.send_command(id, cmd).await
    }

    async fn set_brightness(&self, id: &str, brightness: u8) -> super::Result<()> {
        let switch_level = brightness.clamp(0, 100);
        let cmd = command_json!("switchLevel", "setLevel", [switch_level, 20]);
        self.send_command(id, cmd).await
    }

    async fn set_color_temperature(&self, id: &str, temp: u16) -> super::Result<()> {
        let cmd = command_json!("colorTemperature", "setColorTemperature", [temp]);
        self.send_command(id, cmd).await
    }

    async fn set_color(&self, id: &str, hue: u8, saturation: u8) -> super::Result<()> {
        let cmd = command_json!(
            "colorControl", "setColor", [{ "hue": hue, "saturation": saturation }]);

        self.send_command(id, cmd).await
    }
}
pub(super) mod api_models {
    use serde::Deserialize;
    use serde_flat_path::flat_path;

    #[derive(Deserialize, Debug)]
    pub struct ColorControlStatus {
        pub hue: ColorComponentStatus,
        pub saturation: ColorComponentStatus,
    }
    #[derive(Deserialize, Debug, Default)]
    #[serde(default)]
    pub struct ColorComponentStatus {
        #[serde(with = "time::serde::rfc3339::option")]
        pub timestamp: Option<time::OffsetDateTime>,
        pub value: Option<i32>,
    }

    #[derive(Deserialize, Debug, Default)]
    #[serde(default)]
    pub struct ColorTemperatureStatus {
        #[serde(with = "time::serde::rfc3339::option")]
        pub timestamp: Option<time::OffsetDateTime>,
        pub value: Option<i32>,
        pub unit: Option<String>,
    }
    #[derive(Deserialize, Debug, Default)]
    #[serde(default)]
    pub struct HealthCheckStatus {
        #[serde(rename = "DeviceWatch-DeviceStatus")]
        pub device_status: Option<DeviceWatchDeviceStatus>,
    }
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub enum DeviceStatusValue {
        Offline,
        Online,
    }

    #[derive(Deserialize, Debug, Default)]
    #[serde(default)]
    pub struct DeviceWatchDeviceStatus {
        pub value: Option<DeviceStatusValue>,
        #[serde(with = "time::serde::rfc3339::option")]
        pub timestamp: Option<time::OffsetDateTime>,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    #[serde(rename_all = "camelCase")]
    pub enum SwitchState {
        On,
        Off,
    }

    #[derive(Deserialize, Debug, Default)]
    #[serde(default)]
    pub struct SwitchStatus {
        pub value: Option<SwitchState>,
        #[serde(with = "time::serde::rfc3339::option")]
        pub timestamp: Option<time::OffsetDateTime>,
    }

    #[derive(Deserialize, Debug, Default)]
    #[serde(default)]
    pub struct SwitchLevelStatus {
        pub value: Option<i32>,
        #[serde(with = "time::serde::rfc3339::option")]
        pub timestamp: Option<time::OffsetDateTime>,
        pub unit: String,
    }

    #[flat_path]
    #[derive(Deserialize, Debug, Default)]
    #[serde(rename_all = "camelCase", default)]
    pub struct ComponentStatus {
        pub color_control: Option<ColorControlStatus>,
        #[flat_path("colorTemperature.colorTemperature")]
        pub color_temperature: Option<ColorTemperatureStatus>,
        pub health_check: Option<HealthCheckStatus>,
        #[flat_path("switch.switch")]
        pub switch: Option<SwitchStatus>,
        #[flat_path("switchLevel.level")]
        pub switch_level: Option<SwitchLevelStatus>,
    }

    #[derive(Deserialize, Debug, Default)]
    #[serde(rename_all = "camelCase")]
    pub(super) struct DeviceComponentStatuses {
        #[serde(default)]
        pub main: ComponentStatus,
    }


    #[derive(Deserialize, Debug, Default)]
    #[serde(rename_all = "camelCase")]
    pub(super) struct DeviceStatus {
        #[serde(default)]
        pub components: DeviceComponentStatuses,
    }
}
