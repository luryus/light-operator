use std::fmt::Display;

use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const API_GROUP: &str = "light-operator.lkoskela.com";
pub const API_VERSION: &str = "v1alpha1";
pub const API_VERSION_FULL: &str = "light-operator.lkoskela.com/v1alpha1";

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


#[derive(Deserialize, Serialize, Clone, Copy, Debug, JsonSchema)]
#[serde(rename_all="PascalCase")]
pub enum LightState {
  SwitchedOn, SwitchedOff
}

impl From<LightState> for bool {
    fn from(value: LightState) -> Self {
        value.is_switched_on()
    }
}

impl LightState {
  pub fn is_switched_on(&self) -> bool {
    match self {
      LightState::SwitchedOff => false,
      LightState::SwitchedOn => true
    }
  }
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    namespaced,
    kind = "Light",
    shortname = "li",
    group = "light-operator.lkoskela.com",
    version = "v1alpha1",
    printcolumn = r#"{"name": "Ready", "type": "string", "jsonPath": ".status.conditions[?(@.type==\"Ready\")].status"}"#,
    printcolumn = r#"{"name": "Status", "type": "string", "jsonPath": ".status.conditions[?(@.type==\"Ready\")].message", "priority": 1}"#,
    printcolumn = r#"{"name": "Age", "type": "date", "jsonPath": ".metadata.creationTimestamp"}"#,
    printcolumn = r#"{"name": "Switched on", "type": "string", "jsonPath": ".spec.active"}"#,
)]
#[kube(status = "LightStatus")]
#[serde(rename_all = "camelCase")]
pub struct LightSpec {
    /// Device id
    pub device_id: String,
    /// Is the light on or off
    pub state: LightState,

    pub color: Option<Color>,
    
    /// Brightness 0-100 (percentage)
    #[schemars(range(min = 0, max = 100))]
    pub brightness: Option<u8>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct LightStatus {
    #[schemars(schema_with="schemas::conditions")]
    pub conditions: Vec<Condition>,
}

mod schemas {
    use schemars::{gen::SchemaGenerator, schema::Schema};
    use serde_json::{from_value, json};

    pub fn conditions(_: &mut SchemaGenerator) -> Schema {
        // Custom schema for an array of Conditions. Mostly copied from core.v1.ServiceStatus,
        // but the "items" ref is replaced with the actual Condition object schema because
        // references are not supported
        from_value(json!({
            "type": "array",
            "x-kubernetes-list-map-keys": [
                "type"
            ],
            "x-kubernetes-list-type": "map",
            "x-kubernetes-patch-merge-key": "type",
            "x-kubernetes-patch-strategy": "merge",
            "items": {
                "description": "Condition contains details for one aspect of the current state of this API Resource.",
                "properties": {
                  "lastTransitionTime": {
                    "type": "string",
                    "format": "date-time",
                    "description": "lastTransitionTime is the last time the condition transitioned from one status to another. This should be when the underlying condition changed.  If that is not known, then using the time when the API field changed is acceptable."
                  },
                  "message": {
                    "description": "message is a human readable message indicating details about the transition. This may be an empty string.",
                    "type": "string"
                  },
                  "observedGeneration": {
                    "description": "observedGeneration represents the .metadata.generation that the condition was set based upon. For instance, if .metadata.generation is currently 12, but the .status.conditions[x].observedGeneration is 9, the condition is out of date with respect to the current state of the instance.",
                    "format": "int64",
                    "type": "integer"
                  },
                  "reason": {
                    "description": "reason contains a programmatic identifier indicating the reason for the condition's last transition. Producers of specific condition types may define expected values and meanings for this field, and whether the values are considered a guaranteed API. The value should be a CamelCase string. This field may not be empty.",
                    "type": "string"
                  },
                  "status": {
                    "description": "status of the condition, one of True, False, Unknown.",
                    "type": "string"
                  },
                  "type": {
                    "description": "type of condition in CamelCase or in foo.example.com/CamelCase.",
                    "type": "string"
                  }
                },
                "required": [
                  "type",
                  "status",
                  "lastTransitionTime",
                  "reason",
                  "message"
                ],
                "type": "object"
              },
        }))
        .unwrap()
    }
}