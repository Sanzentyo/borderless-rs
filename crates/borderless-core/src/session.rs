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
            FavoriteSize::AspectFit { width, height } => {
                let target = resolve_target_frame(&self.snapshot, monitors, options.target_frame)?;
                aspect_fit_rect(target, width, height)?
            }
        }
        .with_offsets(options.offsets);

        let maximize =
            options.should_maximize && !matches!(options.size, FavoriteSize::AspectFit { .. });
        let placement = Placement {
            rect,
            topmost: options.top_most,
            maximize,
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

fn aspect_fit_rect(
    target: crate::types::Rect,
    aspect_width: u32,
    aspect_height: u32,
) -> CoreResult<crate::types::Rect> {
    if aspect_width == 0 || aspect_height == 0 {
        return Err(CoreError::Transition("aspect ratio must be non-zero"));
    }

    let aspect_width = i64::from(aspect_width);
    let aspect_height = i64::from(aspect_height);
    let (width, height) = aspect_fit_dimensions(
        i64::from(target.width().0),
        i64::from(target.height().0),
        aspect_width,
        aspect_height,
    );

    centered_rect(target, width, height)
}

fn aspect_fit_dimensions(
    target_width: i64,
    target_height: i64,
    aspect_width: i64,
    aspect_height: i64,
) -> (i64, i64) {
    if target_width * aspect_height <= target_height * aspect_width {
        (target_width, target_width * aspect_height / aspect_width)
    } else {
        (target_height * aspect_width / aspect_height, target_height)
    }
}

fn centered_rect(
    target: crate::types::Rect,
    width: i64,
    height: i64,
) -> CoreResult<crate::types::Rect> {
    if width <= 0 || height <= 0 {
        return Err(CoreError::Transition("aspect rect is empty"));
    }
    let target_width = i64::from(target.width().0);
    let target_height = i64::from(target.height().0);
    let left = i64::from(target.left.0) + (target_width - width) / 2;
    let top = i64::from(target.top.0) + (target_height - height) / 2;
    crate::types::Rect::new(
        i32::try_from(left).map_err(|_| CoreError::Transition("aspect rect overflow"))?,
        i32::try_from(top).map_err(|_| CoreError::Transition("aspect rect overflow"))?,
        i32::try_from(left + width).map_err(|_| CoreError::Transition("aspect rect overflow"))?,
        i32::try_from(top + height).map_err(|_| CoreError::Transition("aspect rect overflow"))?,
    )
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
            .map(|monitor| (monitor, monitor.window_intersection_area(window)))
            .max_by_key(|(_, area)| *area)
            .filter(|(_, area)| *area > 0)
            .map(|(monitor, _)| monitor),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Rect;

    #[test]
    fn aspect_fit_centers_four_by_three_inside_sixteen_by_nine() {
        let monitor = Rect::new(0, 0, 1920, 1080).expect("valid monitor rect");
        let fitted = aspect_fit_rect(monitor, 4, 3).expect("valid aspect fit");

        assert_eq!(fitted, Rect::new(240, 0, 1680, 1080).unwrap());
    }

    #[test]
    fn aspect_fit_centers_wide_inside_tall_area() {
        let monitor = Rect::new(0, 0, 1200, 1600).expect("valid monitor rect");
        let fitted = aspect_fit_rect(monitor, 16, 9).expect("valid aspect fit");

        assert_eq!(fitted, Rect::new(0, 462, 1200, 1137).unwrap());
    }

    #[test]
    fn current_monitor_uses_largest_overlap_not_only_window_origin() {
        let monitor = MonitorSnapshot {
            id: crate::types::MonitorId(1),
            rect: Rect::new(-3840, 0, -1280, 1440).unwrap(),
            work_area: Rect::new(-3840, 0, -1280, 1400).unwrap(),
            primary: false,
        };
        let window = WindowSnapshot {
            hwnd: crate::types::Hwnd(42),
            pid: crate::types::Pid::new(1).unwrap(),
            process_name: crate::types::ProcessName::new("terminal.exe").unwrap(),
            title: crate::types::WindowTitle::new("terminal"),
            class_name: "Window".to_owned(),
            rect: Rect::new(-3847, -6, -1272, 1401).unwrap(),
            style: crate::window::StyleBits::VISIBLE,
            ex_style: crate::window::ExStyleBits::default(),
            is_visible: true,
        };

        let resolved =
            resolve_target_frame(&window, &[monitor], TargetFrame::CurrentMonitor).unwrap();

        assert_eq!(resolved, monitor.rect);
    }
}
