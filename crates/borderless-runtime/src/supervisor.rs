use crate::{
    WatcherMsg, controller::ControllerActor, messages::ControllerMsg, watcher::WatcherActor,
};
use borderless_core::{AppliedStateStore, SettingsStore, WindowCatalog, WindowManipulator};
use ractor::{Actor, ActorRef};
use std::sync::Arc;

pub struct RuntimeHandle {
    pub controller: ActorRef<ControllerMsg>,
    pub watcher: ActorRef<WatcherMsg>,
}

pub async fn spawn_runtime<B>(backend: B) -> Result<RuntimeHandle, ractor::SpawnErr>
where
    B: WindowCatalog
        + WindowManipulator
        + SettingsStore
        + AppliedStateStore
        + Send
        + Sync
        + 'static,
{
    let backend = Arc::new(backend);
    let (controller, _controller_handle) = Actor::spawn(
        Some("borderless.controller".to_owned()),
        ControllerActor::new(Arc::clone(&backend)),
        (),
    )
    .await?;
    let (watcher, _watcher_handle) = Actor::spawn(
        Some("borderless.watcher".to_owned()),
        WatcherActor::new(backend, controller.clone()),
        (),
    )
    .await?;
    Ok(RuntimeHandle {
        controller,
        watcher,
    })
}
