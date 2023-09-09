use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct Color {
    #[schemars(range(min = 0, max = 100))]
    hue: u8,
    #[schemars(range(min = 0, max = 100))]
    saturation: u8,
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    kind = "Light",
    group = "lightcontroller.lkoskela.com",
    version = "v1alpha1",
    namespaced
)]
#[kube(status = "LightStatus")]
#[serde(rename_all = "camelCase")]
pub struct LightSpec {
    /// Device id
    pub device_id: String,
    // Is the light on or off
    pub active: bool,

    // RGB color
    pub color: Option<Color>,
    // Color temperature in Kelvin,
    #[schemars(range(min = 1, max = 60_000))]
    pub color_temperature: Option<u16>,
    // Brightness 0-1
    #[schemars(range(min = 0, max = 1))]
    pub brightness: Option<f32>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct LightStatus {
    pub active: bool,
    pub color: Option<Color>,
    pub color_temperature: Option<u16>,
    pub brightness: Option<f32>,
}
