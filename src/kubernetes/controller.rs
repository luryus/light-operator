use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use kube::{
    runtime::{controller::Action, Controller},
    Api, Client,
};

use super::crd::Light;

#[derive(thiserror::Error, Debug)]
pub enum Error {}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub struct Context {}

pub async fn run() -> Result<(), kube::Error> {
    let client = Client::try_default().await?;
    let lights = Api::<Light>::all(client);

    Controller::new(lights.clone(), Default::default())
        .run(reconcile, error_policy, Arc::new(Context {}))
        .for_each(|_| futures::future::ready(()))
        .await;

    Ok(())
}

pub async fn reconcile(_light: Arc<Light>, _ctx: Arc<Context>) -> Result<Action, Error> {
    Ok(Action::requeue(Duration::from_secs(60)))
}

pub fn error_policy(_light: Arc<Light>, _err: &Error, _ctx: Arc<Context>) -> Action {
    Action::requeue(Duration::from_secs(5))
}
