use crate::action::{BorderlessPlan, MenuPolicy, Placement, TargetFrame};
use crate::error::{CoreError, CoreResult};
use crate::favorite::{FavoriteOptions, FavoriteSize};
use crate::window::{MonitorSnapshot, OriginalWindowState, WindowSnapshot};
use std::marker::PhantomData;

#[derive(Clone, Copy, Debug)]
pub struct Observed;
#[derive(Clone, Copy, Debug)]
pub struct Prepared;
#[derive(Clone, Copy, Debug)]
pub struct Applied;
#[derive(Clone, Copy, Debug)]
pub struct Restored;

#[derive(Clone, Debug)]
pub struct BorderlessSession<State> {
    snapshot: WindowSnapshot,
    original: OriginalWindowState,
    plan: Option<BorderlessPlan>,
    _state: PhantomData<State>,
}

impl BorderlessSession<Observed> {
    #[must_use]
    pub fn observe(snapshot: WindowSnapshot) -> Self {
        let original = OriginalWindowState::from(&snapshot);
        Self {
            snapshot,
            original,
            plan: None,
            _state: PhantomData,
        }
    }

    pub fn prepare(
        self,
        options: &FavoriteOptions,
        monitors: &[MonitorSnapshot],
    ) -> CoreResult<BorderlessSession<Prepared>> {
        let rect = match options.size {
            FavoriteSize::NoChange => self.snapshot.rect,
            FavoriteSize::Specific { rect } => rect,
            FavoriteSize::FullScreen => {
                resolve_target_frame(&self.snapshot, monitors, options.target_frame)?
            }
        }
        .with_offsets(options.offsets);

        let placement = Placement {
            rect,
            topmost: options.top_most,
            maximize: options.should_maximize,
        };
        let mut plan = BorderlessPlan::reversible(self.original.clone(), placement);
        plan.menu_policy = options.menu_policy;
        plan.hide_windows_taskbar = options.hide_windows_taskbar;
        plan.hide_mouse_cursor = options.hide_mouse_cursor;
        plan.mute_in_background = options.mute_in_background;

        Ok(BorderlessSession {
            snapshot: self.snapshot,
            original: self.original,
            plan: Some(plan),
            _state: PhantomData,
        })
    }
}

impl BorderlessSession<Prepared> {
    #[must_use]
    pub const fn plan(&self) -> &BorderlessPlan {
        self.plan.as_ref().expect("prepared session has plan")
    }

    #[must_use]
    pub fn mark_applied(self) -> BorderlessSession<Applied> {
        BorderlessSession {
            snapshot: self.snapshot,
            original: self.original,
            plan: self.plan,
            _state: PhantomData,
        }
    }
}

impl BorderlessSession<Applied> {
    #[must_use]
    pub const fn original(&self) -> &OriginalWindowState {
        &self.original
    }

    #[must_use]
    pub fn mark_restored(self) -> BorderlessSession<Restored> {
        BorderlessSession {
            snapshot: self.snapshot,
            original: self.original,
            plan: self.plan,
            _state: PhantomData,
        }
    }
}

impl<State> BorderlessSession<State> {
    #[must_use]
    pub const fn snapshot(&self) -> &WindowSnapshot {
        &self.snapshot
    }
}

fn resolve_target_frame(
    window: &WindowSnapshot,
    monitors: &[MonitorSnapshot],
    target: TargetFrame,
) -> CoreResult<crate::types::Rect> {
    let selected = match target {
        TargetFrame::CurrentMonitor => monitors
            .iter()
            .copied()
            .find(|monitor| monitor.contains_window_origin(window)),
        TargetFrame::PrimaryMonitor => monitors.iter().copied().find(|monitor| monitor.primary),
        TargetFrame::Monitor(id) => monitors.iter().copied().find(|monitor| monitor.id == id),
        TargetFrame::Exact(rect) => return Ok(rect),
    };

    selected
        .map(|monitor| monitor.rect)
        .ok_or(CoreError::MonitorNotFound(window.hwnd))
}

#[allow(dead_code)]
const _: fn(MenuPolicy) = |_| {};
