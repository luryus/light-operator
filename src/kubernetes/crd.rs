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
pub struct LightSpec {
    /// Device id
    device_id: String,
    // Is the light on or off
    active: bool,

    // RGB color
    color: Option<Color>,
    // Color temperature in Kelvin,
    #[schemars(range(min = 1, max = 60_000))]
    color_temperature: Option<u16>,
    // Brightness 0-1
    #[schemars(range(min = 0, max = 1))]
    brightness: Option<f32>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct LightStatus {
    active: bool,
    color: Option<Color>,
    color_temperature: Option<u16>,
    brightness: Option<f32>,
}
