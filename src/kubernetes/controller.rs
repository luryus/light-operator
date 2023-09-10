use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use kube::{
    runtime::{controller::Action, Controller},
    Api, Client,
};

use crate::{
    config::Config,
    kubernetes::crd::Color,
    smarthome::{self, LightStatus, SmartHomeApi},
};

use super::crd::Light;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Smart home API error: {0}")]
    SmartHomeApi(#[from] smarthome::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub struct Context {
    config: Arc<Config>,
    smart_home_api: Arc<dyn SmartHomeApi>,
}

pub async fn run(
    config: Arc<Config>,
    smart_home_api: Arc<dyn SmartHomeApi + Send + Sync + 'static>,
) -> Result<(), kube::Error> {
    let client = Client::try_default().await?;
    let lights = Api::<Light>::all(client);

    let context = Arc::new(Context {
        config,
        smart_home_api,
    });

    Controller::new(lights.clone(), Default::default())
        .run(reconcile, error_policy, context)
        .for_each(|_| futures::future::ready(()))
        .await;

    Ok(())
}

pub async fn reconcile(light: Arc<Light>, ctx: Arc<Context>) -> Result<Action, Error> {
    tracing::info!(
        "Reconciling {}/{}",
        light.metadata.namespace.as_ref().unwrap(),
        light.metadata.name.as_ref().unwrap()
    );

    // Get status
    let id = &light.spec.device_id;
    let status = ctx.smart_home_api.get_light_status(id).await?;

    if let LightStatus::Online(light_options) = status {
        if light_options.switched_on != light.spec.active {
            tracing::info!("Setting light switched on status to {}", light.spec.active);
            ctx.smart_home_api
                .set_switched_on(id, light.spec.active)
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
    }

    Ok(Action::requeue(Duration::from_secs(
        ctx.config.controller.sync_interval_seconds,
    )))
}

pub fn error_policy(_light: Arc<Light>, err: &Error, _ctx: Arc<Context>) -> Action {
    let err_ref: &(dyn std::error::Error + Send + Sync) = err;
    tracing::error!(error = err_ref, "Reconciler error");
    Action::requeue(Duration::from_secs(5))
}
