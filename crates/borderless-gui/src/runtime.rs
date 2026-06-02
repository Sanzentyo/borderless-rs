use crate::model::{GuiModel, StatusLine, favorite_from_window};
use anyhow::Result;
use borderless_core::profile::ProfileSpan;
use borderless_core::{FavoriteOptions, Hwnd, WindowSnapshot};
use borderless_native::NativeBackend;
use borderless_runtime::{ControllerMsg, spawn_runtime};
use ractor::ActorRef;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::runtime::Runtime;
use windows_reactor::AsyncSetState;

#[derive(Clone)]
pub struct GuiRuntime {
    inner: Arc<Runtime>,
    controller: ActorRef<ControllerMsg>,
    reset_environment_on_exit: Arc<AtomicBool>,
}

impl GuiRuntime {
    pub fn boot() -> Result<Self> {
        let _span = ProfileSpan::start("gui.runtime.boot");
        let inner = Arc::new(Runtime::new()?);
        ProfileSpan::mark("gui.runtime.boot: tokio runtime created");
        let handle = inner.block_on(spawn_runtime(NativeBackend::new()))?;
        ProfileSpan::mark("gui.runtime.boot: actors spawned");
        Ok(Self {
            inner,
            controller: handle.controller,
            reset_environment_on_exit: Arc::new(AtomicBool::new(true)),
        })
    }

    pub fn set_reset_environment_on_exit(&self, reset: bool) {
        self.reset_environment_on_exit
            .store(reset, Ordering::Relaxed);
    }

    pub fn initial_model(&self) -> GuiModel {
        let _span = ProfileSpan::start("gui.runtime.initial_model");
        let controller = self.controller.clone();
        let (windows, monitors) = self.inner.block_on(async move {
            let windows = ractor::call!(controller.clone(), ControllerMsg::ListWindows);
            ProfileSpan::mark("gui.runtime.initial_model: ListWindows returned");
            let monitors = ractor::call!(controller, ControllerMsg::ListMonitors);
            ProfileSpan::mark("gui.runtime.initial_model: ListMonitors returned");
            (windows, monitors)
        });

        let model = match monitors {
            Ok(monitors) => GuiModel::default().with_monitors(monitors),
            Err(err) => GuiModel::default()
                .with_status(StatusLine::error("Monitor refresh failed", err.to_string())),
        };

        match windows {
            Ok(windows) => on_windows_refreshed(model, windows),
            Err(err) => {
                model.with_status(StatusLine::error("Initial refresh failed", err.to_string()))
            }
        }
    }

    pub fn refresh_windows(&self, set_model: AsyncSetState<GuiModel>, model: GuiModel) {
        let controller = self.controller.clone();
        self.inner.spawn(async move {
            let windows = ractor::call!(controller, ControllerMsg::ListWindows);
            let monitors = ractor::call!(controller, ControllerMsg::ListMonitors);
            let model = match monitors {
                Ok(monitors) => model.with_monitors(monitors),
                Err(err) => model.with_status(StatusLine::warning(
                    "Monitor refresh failed",
                    err.to_string(),
                )),
            };
            let next = match windows {
                Ok(windows) => on_windows_refreshed(model, windows),
                Err(err) => model
                    .with_busy(false)
                    .with_status(StatusLine::error("Refresh failed", err.to_string())),
            };
            set_model.call(next);
        });
    }

    pub fn apply_window(&self, hwnd: Hwnd, set_model: AsyncSetState<GuiModel>, model: GuiModel) {
        let controller = self.controller.clone();
        self.inner.spawn(async move {
            let result = ractor::call!(controller, |reply| ControllerMsg::ApplyByHwnd(hwnd, reply));
            let next = match result {
                Ok(Ok(applied)) => model.with_status(StatusLine::success(
                    "Applied borderless",
                    format!("{applied} is now borderless."),
                )),
                Ok(Err(err)) => model.with_status(StatusLine::error("Apply failed", err)),
                Err(err) => model.with_status(StatusLine::error("Apply failed", err.to_string())),
            };
            set_model.call(next.with_busy(false));
        });
    }

    pub fn apply_window_with_options(
        &self,
        hwnd: Hwnd,
        options: FavoriteOptions,
        set_model: AsyncSetState<GuiModel>,
        model: GuiModel,
    ) {
        let controller = self.controller.clone();
        self.inner.spawn(async move {
            let result = ractor::call!(controller, |reply| ControllerMsg::ApplyByHwndWithOptions(
                hwnd, options, reply
            ));
            let next = match result {
                Ok(Ok(applied)) => model.with_status(StatusLine::success(
                    "Applied aspect fit",
                    format!("{applied} is centered without stretching."),
                )),
                Ok(Err(err)) => {
                    model.with_status(StatusLine::error("Apply aspect fit failed", err))
                }
                Err(err) => model.with_status(StatusLine::error(
                    "Apply aspect fit failed",
                    err.to_string(),
                )),
            };
            set_model.call(next.with_busy(false));
        });
    }

    pub fn restore_window(&self, hwnd: Hwnd, set_model: AsyncSetState<GuiModel>, model: GuiModel) {
        let controller = self.controller.clone();
        self.inner.spawn(async move {
            let result = ractor::call!(controller, |reply| ControllerMsg::Restore(hwnd, reply));
            let next = match result {
                Ok(Ok(())) => model.with_status(StatusLine::success(
                    "Restored window",
                    format!("{hwnd} has been restored."),
                )),
                Ok(Err(err)) => model.with_status(StatusLine::error("Restore failed", err)),
                Err(err) => model.with_status(StatusLine::error("Restore failed", err.to_string())),
            };
            set_model.call(next.with_busy(false));
        });
    }

    pub fn add_favorite_from_window(
        &self,
        window: WindowSnapshot,
        set_model: AsyncSetState<GuiModel>,
        model: GuiModel,
    ) {
        let controller = self.controller.clone();
        self.inner.spawn(async move {
            let favorite = favorite_from_window(&window);
            let id = favorite.id.to_string();
            let result = ractor::call!(controller, |reply| ControllerMsg::AddFavorite(
                favorite, reply
            ));
            let next = match result {
                Ok(Ok(())) => model.with_status(StatusLine::success(
                    "Favorite added",
                    format!("{id} will be applied by the watcher."),
                )),
                Ok(Err(err)) => model.with_status(StatusLine::error("Add favorite failed", err)),
                Err(err) => {
                    model.with_status(StatusLine::error("Add favorite failed", err.to_string()))
                }
            };
            set_model.call(next.with_busy(false));
        });
    }

    pub fn set_taskbar_visible(
        &self,
        visible: bool,
        set_model: AsyncSetState<GuiModel>,
        model: GuiModel,
    ) {
        let controller = self.controller.clone();
        self.inner.spawn(async move {
            let result = ractor::call!(controller, |reply| ControllerMsg::SetTaskbarVisible(
                visible, reply
            ));
            let verb = if visible { "shown" } else { "hidden" };
            let next = match result {
                Ok(Ok(())) => model.with_status(StatusLine::success(
                    "Taskbar updated",
                    format!("Windows taskbar is now {verb}."),
                )),
                Ok(Err(err)) => model.with_status(StatusLine::error("Taskbar update failed", err)),
                Err(err) => {
                    model.with_status(StatusLine::error("Taskbar update failed", err.to_string()))
                }
            };
            set_model.call(next.with_busy(false));
        });
    }

    pub fn set_cursor_visible(
        &self,
        visible: bool,
        set_model: AsyncSetState<GuiModel>,
        model: GuiModel,
    ) {
        let controller = self.controller.clone();
        self.inner.spawn(async move {
            let result = ractor::call!(controller, |reply| ControllerMsg::SetCursorVisible(
                visible, reply
            ));
            let verb = if visible { "shown" } else { "hidden" };
            let next = match result {
                Ok(Ok(())) => model.with_status(StatusLine::success(
                    "Cursor updated",
                    format!("Mouse cursor is now {verb}."),
                )),
                Ok(Err(err)) => model.with_status(StatusLine::error("Cursor update failed", err)),
                Err(err) => {
                    model.with_status(StatusLine::error("Cursor update failed", err.to_string()))
                }
            };
            set_model.call(next.with_busy(false));
        });
    }
}

impl Drop for GuiRuntime {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) != 1
            || !self.reset_environment_on_exit.load(Ordering::Relaxed)
        {
            return;
        }

        let controller = self.controller.clone();
        self.inner.block_on(async move {
            let _ = ractor::call!(controller, |reply| ControllerMsg::SetTaskbarVisible(
                true, reply
            ));
            let _ = ractor::call!(controller, |reply| ControllerMsg::SetCursorVisible(
                true, reply
            ));
        });
    }
}

fn on_windows_refreshed(model: GuiModel, windows: Vec<WindowSnapshot>) -> GuiModel {
    let current_pid = std::process::id();
    let windows = windows
        .into_iter()
        .filter(|window| window.pid.get() != current_pid)
        .collect::<Vec<_>>();
    let count = windows.len();
    model
        .with_busy(false)
        .with_windows(windows)
        .with_status(StatusLine::success(
            "Windows refreshed",
            format!("Loaded {count} targetable windows."),
        ))
}
