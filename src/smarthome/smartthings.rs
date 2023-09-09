use std::{sync::Arc, time::Duration};

use anyhow::{bail, Error};
use async_trait::async_trait;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, Response, StatusCode, Url,
};
use serde::Deserialize;
use serde_flat_path::flat_path;
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::config::Config;

use super::{LightOptions, LightStatus, SmartHomeApi};

const APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

const API_BASE_URL: &str = "https://api.smartthings.com/v1/";

pub struct SmartThings {
    config: Arc<Config>,
    client: Client,
    base_url: Url,
}

impl SmartThings {
    pub fn new(config: Arc<Config>) -> super::Result<Self> {
        let Some(api_key) = &config.smart_home.smartthings.api_key else {
            return Err(super::Error::Configuration(
                "SmartThings API key not configured".to_string(),
            ));
        };

        let mut auth_header = HeaderMap::new();
        auth_header.append(
            "Authorization",
            format!("Bearer {}", api_key).parse().unwrap(),
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
            config,
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

    async fn ping(&self, id: &str) -> super::Result<()> {
        Self::validate_device_id(id)?;

        tracing::info!("Pinging device {id}");

        let url = self
            .base_url
            .join(&format!("devices/{id}/commands"))
            .map_err(|_| super::Error::InvalidId(id.to_string()))?;

        let body = json!({
            "commands": [{
                "component": "main",
                "capability": "healthCheck",
                "command": "ping",
                "arguments": []
            }]
        });

        let res = self.client.post(url).json(&body).send().await?;

        Self::error_from_status(res)?;
        Ok(())
    }
}

#[async_trait]
impl SmartHomeApi for SmartThings {
    async fn get_light_status(&self, id: &str) -> super::Result<LightStatus> {
        Self::validate_device_id(id)?;
        self.ping(id).await?;
        tokio::time::sleep(Duration::from_millis(200)).await;

        tracing::info!("Getting status for device {id}");

        let url = self
            .base_url
            .join(&format!("devices/{id}/status"))
            .map_err(|_| super::Error::InvalidId(id.to_string()))?;

        let res = self.client.get(url).send().await?;

        let body: DeviceStatus = Self::error_from_status(res)?.json().await?;

        tracing::info!("Got status {body:#?}");

        let online = body
            .main_component
            .health_check
            .and_then(|x| x.device_status)
            .map(|x| matches!(x.value, Some(DeviceStatusValue::Online)))
            .unwrap_or(false);

        if !online {
            return Ok(LightStatus::Offline);
        }

        let switched_on = body
            .main_component
            .switch
            .and_then(|x| x.value)
            .map(|x| x == SwitchState::On)
            .unwrap_or(false);

        let brightness = body
            .main_component
            .switch_level
            .map(|x| (x.value as f32 / 100f32).clamp(0.0, 1.0));

        let color_temperature = body
            .main_component
            .color_temperature
            .and_then(|x| x.value)
            .and_then(|x| x.try_into().ok());

        let color = body.main_component.color_control.and_then(|x| {
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
        Self::validate_device_id(id)?;
        let url = self
            .base_url
            .join(&format!("devices/{id}/commands"))
            .map_err(|_| super::Error::InvalidId(id.to_string()))?;

        let body = json!({
            "commands": [{
                "component": "main",
                "capability": "switch",
                "command": if switched_on { "on" } else { "off" },
                "arguments": []
            }]
        });

        let res = self.client.post(url).json(&body).send().await?;

        Self::error_from_status(res)?;
        Ok(())
    }

    async fn set_brightness(&self, id: &str, brightness: f32) -> super::Result<()> {
        Self::validate_device_id(id)?;
        let url = self
            .base_url
            .join(&format!("devices/{id}/commands"))
            .map_err(|_| super::Error::InvalidId(id.to_string()))?;

        let switch_level = (brightness * 100.0).round().clamp(0f32, 100f32) as u8;

        let body = json!({
            "commands": [{
                "component": "main",
                "capability": "switchLevel",
                "command": "setLevel",
                "arguments": [switch_level, 20]
            }]
        });

        let res = self.client.post(url).json(&body).send().await?;

        Self::error_from_status(res)?;
        Ok(())
    }
}

#[derive(Deserialize, Debug)]
struct ColorControlStatus {
    hue: ColorComponentStatus,
    saturation: ColorComponentStatus,
}
#[derive(Deserialize, Debug)]
struct ColorComponentStatus {
    #[serde(with = "time::serde::rfc3339::option")]
    timestamp: Option<time::OffsetDateTime>,
    value: Option<i32>,
}

#[derive(Deserialize, Debug)]
struct ColorTemperatureStatus {
    #[serde(with = "time::serde::rfc3339::option")]
    timestamp: Option<time::OffsetDateTime>,
    value: Option<i32>,
    unit: Option<String>,
}
#[derive(Deserialize, Debug)]
struct HealthCheckStatus {
    #[serde(rename = "DeviceWatch-DeviceStatus")]
    device_status: Option<DeviceWatchDeviceStatus>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
enum DeviceStatusValue {
    Offline,
    Online,
}

#[derive(Deserialize, Debug)]
struct DeviceWatchDeviceStatus {
    value: Option<DeviceStatusValue>,
    #[serde(with = "time::serde::rfc3339::option")]
    timestamp: Option<time::OffsetDateTime>,
}
#[derive(Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
enum SwitchState {
    On,
    Off,
}

#[derive(Deserialize, Debug)]
struct SwitchStatus {
    value: Option<SwitchState>,
    #[serde(with = "time::serde::rfc3339::option")]
    timestamp: Option<time::OffsetDateTime>,
}

#[derive(Deserialize, Debug)]
struct SwitchLevelStatus {
    value: i32,
    #[serde(with = "time::serde::rfc3339::option")]
    timestamp: Option<time::OffsetDateTime>,
    unit: String,
}

#[flat_path]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ComponentStatus {
    color_control: Option<ColorControlStatus>,
    #[flat_path("colorTemperature.colorTemperature")]
    color_temperature: Option<ColorTemperatureStatus>,
    health_check: Option<HealthCheckStatus>,
    #[flat_path("switch.switch")]
    switch: Option<SwitchStatus>,
    #[flat_path("switchLevel.level")]
    switch_level: Option<SwitchLevelStatus>,
}

#[flat_path]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DeviceStatus {
    #[flat_path("components.main")]
    main_component: ComponentStatus,
}
