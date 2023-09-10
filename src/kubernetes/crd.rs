use std::fmt::Display;

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct HueSaturationColor {
    /// Hue 0-100 (percentage)
    #[schemars(range(min = 0, max = 100))]
    pub hue: u8,
    /// Saturation 0-100 (percentage)
    #[schemars(range(min = 0, max = 100))]
    pub saturation: u8,
}

impl Display for HueSaturationColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(hue {}, saturation {})", self.hue, self.saturation)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all="camelCase")]
pub enum Color {
    ColorTemperature(#[schemars(range(min = 1, max = 60_000))] u16),
    HueSaturation(HueSaturationColor)
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    kind = "Light",
    shortname = "li",
    group = "lightcontroller.lkoskela.com",
    version = "v1alpha1",
    namespaced
)]
#[kube(status = "LightStatus")]
#[serde(rename_all = "camelCase")]
pub struct LightSpec {
    /// Device id
    pub device_id: String,
    /// Is the light on or off
    pub active: bool,

    pub color: Option<Color>,
    
    /// Brightness 0-100 (percentage)
    #[schemars(range(min = 0, max = 100))]
    pub brightness: Option<u8>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct LightStatus {
    pub active: bool,
    pub color: Option<Color>,
    pub color_temperature: Option<u16>,
    pub brightness: Option<f32>,
}
