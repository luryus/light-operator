use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use k8s_openapi::{
    apimachinery::pkg::apis::meta::v1::{Condition, Time},
    chrono::{DateTime, Utc},
};
use kube::{
    api::{Patch, PatchParams},
    runtime::{controller::Action, Controller},
    Api, Client, ResourceExt,
};
use serde_json::json;

use crate::{
    config::Config,
    kubernetes::crd::{self, Color},
    smarthome::{self, LightStatus, SmartHomeApi},
};

use super::crd::Light;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Smart home API error: {0}")]
    SmartHomeApi(#[from] smarthome::Error),

    #[error("Kubernetes API error: {0}")]
    Kubernetes(#[from] kube::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub struct Context {
    config: Arc<Config>,
    smart_home_api: Arc<dyn SmartHomeApi>,
    kube_client: Client,
}

pub async fn run(
    config: Arc<Config>,
    smart_home_api: Arc<dyn SmartHomeApi + Send + Sync + 'static>,
) -> Result<(), kube::Error> {
    let client = Client::try_default().await?;
    let lights = Api::<Light>::all(client.clone());

    let context = Arc::new(Context {
        config,
        smart_home_api,
        kube_client: client,
    });

    Controller::new(lights.clone(), Default::default())
        .run(reconcile, error_policy, context)
        .for_each(|_| futures::future::ready(()))
        .await;

    Ok(())
}

pub async fn reconcile(light: Arc<Light>, ctx: Arc<Context>) -> Result<Action, Error> {
    let ns = light.namespace().unwrap();
    let name = light.name_any();
    tracing::info!("Reconciling {}/{}", &ns, &name);

    // Get status
    let id = &light.spec.device_id;
    let status_res = ctx.smart_home_api.get_light_status(id).await;

    let lights: Api<Light> = Api::namespaced(ctx.kube_client.clone(), &ns);

    let mut conds = light
        .status
        .clone()
        .map(|s| s.conditions)
        .unwrap_or_default();
    ensure_condition(&mut conds, "InvalidDevice", light.metadata.generation);
    ensure_condition(&mut conds, "Ready", light.metadata.generation);

    let status = match status_res {
        Ok(s) => s,
        Err(he) => {
            let invalid = match he {
                smarthome::Error::Configuration(_) => None,
                smarthome::Error::InvalidId(_) => Some("IdIsInvalid".to_string()),
                smarthome::Error::RequestFailed(_) => None,
                smarthome::Error::UnknownDeviceId => Some("DeviceNotFound".to_string()),
            };

            let invalid_cond = match invalid {
                None => invalid_device_condition(Some(false), "DeviceIdOk", None, light.metadata.generation),
                Some(reason) => invalid_device_condition(Some(true), reason, Some("Device ID is invalid or unknown"), light.metadata.generation),
            };
            update_conditions(&mut conds, invalid_cond);
            let ready_cond = ready_condition(Some(false), "InvalidDevice", None, light.metadata.generation);
            update_conditions(&mut conds, ready_cond);

            patch_conditions(conds, lights, &name).await?;
            return Err(Error::SmartHomeApi(he));
        }
    };

    let invalid_cond = invalid_device_condition(Some(false), "DeviceIdOk", None, light.metadata.generation);
    update_conditions(&mut conds, invalid_cond);

    if let LightStatus::Online(light_options) = status {
        if light_options.switched_on != light.spec.state.is_switched_on() {
            tracing::info!("Setting light switched on status to {:?}", light.spec.state);
            ctx.smart_home_api
                .set_switched_on(id, light.spec.state.into())
                .await?;
        }

        if let Some(target_brightness) = light.spec.brightness {
            if Some(target_brightness) != light_options.brightness {
                tracing::info!("Setting light brightness to {target_brightness}");
                ctx.smart_home_api
                    .set_brightness(id, target_brightness)
                    .await?;
            }
        }

        match light.spec.color.as_ref() {
            Some(&Color::ColorTemperature(target_temp)) => {
                if Some(target_temp) != light_options.color_temperature {
                    tracing::info!("Setting color temperature to {target_temp} K");
                    ctx.smart_home_api
                        .set_color_temperature(id, target_temp)
                        .await?;
                }
            }
            Some(Color::HueSaturation(target_hue_sat)) => {
                if Some(target_hue_sat.hue) != light_options.color.as_ref().map(|x| x.hue)
                    || Some(target_hue_sat.saturation)
                        != light_options.color.as_ref().map(|x| x.saturation)
                {
                    tracing::info!("Setting color to {target_hue_sat}");
                    ctx.smart_home_api
                        .set_color(id, target_hue_sat.hue, target_hue_sat.saturation)
                        .await?;
                }
            }
            None => (),
        };

        let ready_cond = ready_condition(Some(true), "DeviceOnline", None, light.metadata.generation);
        update_conditions(&mut conds, ready_cond);
    } else {
        let ready_cond = ready_condition(Some(false), "DeviceOffline", Some("Light device is offline"), light.metadata.generation);
        update_conditions(&mut conds, ready_cond);
    }

    patch_conditions(conds, lights, &name).await?;

    Ok(Action::requeue(Duration::from_secs(
        ctx.config.controller.sync_interval_seconds,
    )))
}

pub fn error_policy(_light: Arc<Light>, err: &Error, _ctx: Arc<Context>) -> Action {
    let err_ref: &(dyn std::error::Error + Send + Sync) = err;
    tracing::error!(error = err_ref, "Reconciler error");
    Action::requeue(Duration::from_secs(5))
}

fn update_conditions(
    status_conditions: &mut Vec<Condition>,
    mut new_condition: Condition,
) {
    if let Some(existing_cond) = status_conditions
        .iter_mut()
        .find(|c| c.type_ == new_condition.type_)
    {
        if existing_cond.status != new_condition.status
            || existing_cond.reason != new_condition.reason
            || existing_cond.observed_generation != new_condition.observed_generation
        {
            existing_cond.status = new_condition.status;
            existing_cond.last_transition_time = Time(Utc::now());
            existing_cond.message = new_condition.message;
            existing_cond.reason = new_condition.reason;
            existing_cond.observed_generation = new_condition.observed_generation;
        }
    } else {
        new_condition.last_transition_time = Time(Utc::now());
        status_conditions.push(new_condition);
    }
}

async fn patch_conditions(
    conditions: Vec<Condition>,
    lights: Api<Light>,
    light_name: &str,
) -> kube::Result<()> {
    let status_patch = Patch::Apply(json!({
        "apiVersion": crd::API_VERSION_FULL,
        "kind": "Light",
        "status": crd::LightStatus {
            conditions
        }
    }));

    let pp = PatchParams::apply("cntrlr").force();
    lights.patch_status(light_name, &pp, &status_patch).await?;
    Ok(())
}

fn invalid_device_condition(
    status: Option<bool>,
    reason: impl Into<String>,
    message: Option<&str>,
    generation: Option<i64>,
) -> Condition {
    create_condition("InvalidDevice", status, reason, message.map(|m| m.into()), generation)
}

fn ready_condition(
    status: Option<bool>,
    reason: impl Into<String>,
    message: Option<&str>,
    generation: Option<i64>,
) -> Condition {
    create_condition("Ready", status, reason, message.map(|m| m.into()), generation)
}

fn create_condition(
    _type: impl Into<String>,
    status: Option<bool>,
    reason: impl Into<String>,
    message: Option<String>,
    generation: Option<i64>,
) -> Condition {
    let status = match status {
        None => "Unknown",
        Some(true) => "True",
        Some(false) => "False",
    }
    .to_string();

    Condition {
        type_: _type.into(),
        status,
        reason: reason.into(),
        message: message.unwrap_or_default(),
        last_transition_time: Time(DateTime::default()),
        observed_generation: generation,
    }
}

fn ensure_condition(
    conditions: &mut Vec<Condition>,
    _type: impl Into<String>,
    generation: Option<i64>
) {
    let _type = _type.into();
    if !conditions.iter().any(|c| c.type_ == _type) {
        update_conditions(conditions, create_condition(_type, None, "Unknown", None, generation))
    }
}