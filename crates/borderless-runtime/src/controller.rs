use crate::messages::ControllerMsg;
use borderless_core::{
    AppConfig, AppliedStateStore, BorderlessSession, Favorite, FavoriteId, Hwnd, Observed,
    OriginalWindowState, SettingsStore, WindowCatalog, WindowManipulator, WindowSnapshot,
};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use std::collections::HashMap;
use std::sync::Arc;

pub struct ControllerActor<B> {
    backend: Arc<B>,
}

#[derive(Debug)]
pub struct ControllerState {
    config: AppConfig,
    applied: HashMap<Hwnd, OriginalWindowState>,
}

impl<B> ControllerActor<B> {
    #[must_use]
    pub fn new(backend: Arc<B>) -> Self {
        Self { backend }
    }
}

#[ractor::async_trait]
impl<B> Actor for ControllerActor<B>
where
    B: WindowCatalog
        + WindowManipulator
        + SettingsStore
        + AppliedStateStore
        + Send
        + Sync
        + 'static,
{
    type Msg = ControllerMsg;
    type State = ControllerState;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (): (),
    ) -> Result<Self::State, ActorProcessingErr> {
        let config = self.backend.load_config().unwrap_or_default();
        let applied = self
            .backend
            .load_applied_states()
            .unwrap_or_default()
            .into_iter()
            .filter(|original| self.backend.by_hwnd(original.hwnd).ok().flatten().is_some())
            .map(|original| (original.hwnd, original))
            .collect();
        Ok(ControllerState { config, applied })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ControllerMsg::ListWindows(reply) => {
                let _ = reply.send(self.backend.windows().unwrap_or_default());
            }
            ControllerMsg::ListMonitors(reply) => {
                let _ = reply.send(self.backend.monitors().unwrap_or_default());
            }
            ControllerMsg::ApplyByHwnd(hwnd, reply) => {
                let result =
                    self.apply_window(state, self.backend.by_hwnd(hwnd).ok().flatten(), None);
                let _ = reply.send(result.map_err(|err| err.to_string()));
            }
            ControllerMsg::ApplyByHwndWithOptions(hwnd, options, reply) => {
                let result = self.apply_window(
                    state,
                    self.backend.by_hwnd(hwnd).ok().flatten(),
                    Some(&options),
                );
                let _ = reply.send(result.map_err(|err| err.to_string()));
            }
            ControllerMsg::ApplyByPid(pid, reply) => {
                let result =
                    self.apply_window(state, self.backend.by_pid(pid).ok().flatten(), None);
                let _ = reply.send(result.map_err(|err| err.to_string()));
            }
            ControllerMsg::ApplyByProcessName(name, reply) => {
                let result = self.apply_window(
                    state,
                    self.backend.by_process_name(&name).ok().flatten(),
                    None,
                );
                let _ = reply.send(result.map_err(|err| err.to_string()));
            }
            ControllerMsg::ApplyByTitle(title, reply) => {
                let result =
                    self.apply_window(state, self.backend.by_title(&title).ok().flatten(), None);
                let _ = reply.send(result.map_err(|err| err.to_string()));
            }
            ControllerMsg::Restore(hwnd, reply) => {
                let result = state
                    .applied
                    .remove(&hwnd)
                    .ok_or_else(|| format!("no captured state for {hwnd}"))
                    .and_then(|original| {
                        self.backend
                            .restore_original(&original)
                            .map_err(|err| err.to_string())
                    })
                    .and_then(|()| self.save_applied(state).map_err(|err| err.to_string()));
                let _ = reply.send(result);
            }
            ControllerMsg::AddFavorite(favorite, reply) => {
                state.config.favorites.push(favorite);
                let result = self
                    .backend
                    .save_config(&state.config)
                    .map_err(|err| err.to_string());
                let _ = reply.send(result);
            }
            ControllerMsg::RemoveFavorite(id, reply) => {
                state
                    .config
                    .favorites
                    .retain(|favorite| favorite.id.as_str() != id);
                let result = self
                    .backend
                    .save_config(&state.config)
                    .map_err(|err| err.to_string());
                let _ = reply.send(result);
            }
            ControllerMsg::SaveConfig(reply) => {
                let result = self
                    .backend
                    .save_config(&state.config)
                    .map_err(|err| err.to_string());
                let _ = reply.send(result);
            }
            ControllerMsg::SetTaskbarVisible(visible, reply) => {
                let result = self
                    .backend
                    .set_taskbar_visible(visible)
                    .map_err(|err| err.to_string());
                let _ = reply.send(result);
            }
            ControllerMsg::SetCursorVisible(visible, reply) => {
                let result = self
                    .backend
                    .set_cursor_visible(visible)
                    .map_err(|err| err.to_string());
                let _ = reply.send(result);
            }
        }
        Ok(())
    }
}

impl<B> ControllerActor<B>
where
    B: WindowCatalog + WindowManipulator + SettingsStore + AppliedStateStore,
{
    fn apply_window(
        &self,
        state: &mut ControllerState,
        window: Option<WindowSnapshot>,
        explicit_options: Option<&borderless_core::FavoriteOptions>,
    ) -> Result<Hwnd, borderless_core::CoreError> {
        let window = window.ok_or(borderless_core::CoreError::Transition("window not found"))?;
        if explicit_options.is_none() && state.applied.contains_key(&window.hwnd) {
            return Ok(window.hwnd);
        }
        let favorite = state
            .config
            .favorites
            .iter()
            .find(|favorite| favorite.matches(&window).unwrap_or(false));
        let options = explicit_options
            .cloned()
            .or_else(|| favorite.map(|favorite| favorite.options.clone()))
            .unwrap_or_default();
        let captured = state.applied.get(&window.hwnd).cloned();
        let planning_window = captured.as_ref().map_or_else(
            || window.clone(),
            |original| WindowSnapshot {
                style: original.style,
                ex_style: original.ex_style,
                rect: original.rect,
                ..window.clone()
            },
        );
        let monitors = self.backend.monitors()?;
        let prepared =
            BorderlessSession::<Observed>::observe(planning_window).prepare(&options, &monitors)?;
        let original = self.backend.apply_plan(prepared.plan())?;
        state
            .applied
            .insert(window.hwnd, captured.unwrap_or(original));
        self.save_applied(state)?;
        Ok(window.hwnd)
    }

    fn save_applied(&self, state: &ControllerState) -> Result<(), borderless_core::CoreError> {
        let mut states = state.applied.values().cloned().collect::<Vec<_>>();
        states.sort_by_key(|original| original.hwnd.0);
        self.backend.save_applied_states(&states)
    }
}

#[allow(dead_code)]
fn _keep_favorite(_: Favorite, _: FavoriteId) {}
