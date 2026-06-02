use crate::messages::{ControllerMsg, WatcherMsg};
use borderless_core::{AppConfig, SettingsStore, WindowCatalog};
use ractor::{Actor, ActorProcessingErr, ActorRef, call};
use std::sync::Arc;
use tokio::time::{Instant, MissedTickBehavior, interval_at};

pub struct WatcherActor<B> {
    backend: Arc<B>,
    controller: ActorRef<ControllerMsg>,
}

#[derive(Debug)]
pub struct WatcherState {
    config: AppConfig,
}

impl<B> WatcherActor<B> {
    #[must_use]
    pub fn new(backend: Arc<B>, controller: ActorRef<ControllerMsg>) -> Self {
        Self {
            backend,
            controller,
        }
    }
}

#[ractor::async_trait]
impl<B> Actor for WatcherActor<B>
where
    B: WindowCatalog + SettingsStore + Send + Sync + 'static,
{
    type Msg = WatcherMsg;
    type State = WatcherState;
    type Arguments = ();

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        (): (),
    ) -> Result<Self::State, ActorProcessingErr> {
        let config = self.backend.load_config().unwrap_or_default();
        let poll_interval = config.effective_poll_interval();
        let mut ticker = interval_at(Instant::now() + poll_interval, poll_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        tokio::spawn(async move {
            loop {
                ticker.tick().await;
                if myself.send_message(WatcherMsg::Tick).is_err() {
                    break;
                }
            }
        });
        Ok(WatcherState { config })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            WatcherMsg::Tick => self.tick(state).await,
            WatcherMsg::Stop => myself.stop(None),
        }
        Ok(())
    }
}

impl<B> WatcherActor<B>
where
    B: WindowCatalog + SettingsStore,
{
    async fn tick(&self, state: &WatcherState) {
        let Ok(windows) = self.backend.windows() else {
            return;
        };
        for favorite in &state.config.favorites {
            let Some(window) = windows
                .iter()
                .find(|window| favorite.matches(window).unwrap_or(false))
            else {
                continue;
            };
            if !favorite.options.delay.is_zero() {
                tokio::time::sleep(favorite.options.delay).await;
            }
            let _ = call!(self.controller, |reply| ControllerMsg::ApplyByHwnd(
                window.hwnd,
                reply
            ));
        }
    }
}
