use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use kube::{
    runtime::{controller::Action, Controller},
    Api, Client,
};

use crate::{
    config::Config,
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
        "Reconciling {:?}/{:?}",
        &light.metadata.namespace,
        &light.metadata.name
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
    }

    Ok(Action::requeue(Duration::from_secs(
        ctx.config.controller.sync_interval_seconds,
    )))
}

pub fn error_policy(_light: Arc<Light>, err: &Error, _ctx: Arc<Context>) -> Action {
    tracing::error!("Error: {}", err);
    Action::requeue(Duration::from_secs(5))
}
