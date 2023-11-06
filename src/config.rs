use serde::Deserialize;

#[derive(Default, Deserialize)]
pub enum SmartHomePlatform {
    #[default]
    SmartThings,
}

#[derive(Default, Deserialize)]
pub struct SmartHomeConfig {
    pub platform: SmartHomePlatform,
    pub smartthings: SmartThingsConfig,
}

#[derive(Default, Deserialize)]
pub struct SmartThingsConfig {
    pub api_key: Option<String>,
}

#[derive(Deserialize)]
pub struct ControllerConfig {
    pub sync_interval_seconds: u64,
}

#[derive(Deserialize)]
pub struct LogConfig {
    pub filters: String,
}

#[derive(Deserialize)]
pub struct Config {
    pub smart_home: SmartHomeConfig,
    pub controller: ControllerConfig,
    pub log: LogConfig,
    pub health_check: HealthCheckConfig,
}

#[derive(Deserialize)]
pub struct HealthCheckConfig {
    pub enable_server: bool,
    pub port: u16
}
