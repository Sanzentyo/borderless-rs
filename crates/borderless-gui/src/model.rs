use borderless_core::{
    Favorite, FavoriteId, FavoriteMatcher, FavoriteOptions, FavoriteSize, Hwnd, MonitorSnapshot,
    TargetFrame, WindowSnapshot,
};
use borderless_upscale_core::{
    CaptureBackend, InputBackend, RendererBackend, ScalingAlgorithm, ScalingAlgorithmId,
    ScalingPipeline, scaling_algorithm,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Windows,
    Favorites,
    Settings,
    Logs,
}

impl Page {
    pub const WINDOWS_TAG: &'static str = "windows";
    pub const FAVORITES_TAG: &'static str = "favorites";
    pub const SETTINGS_TAG: &'static str = "settings";
    pub const LOGS_TAG: &'static str = "logs";

    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Windows => Self::WINDOWS_TAG,
            Self::Favorites => Self::FAVORITES_TAG,
            Self::Settings => Self::SETTINGS_TAG,
            Self::Logs => Self::LOGS_TAG,
        }
    }

    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            Self::WINDOWS_TAG => Some(Self::Windows),
            Self::FAVORITES_TAG => Some(Self::Favorites),
            Self::SETTINGS_TAG => Some(Self::Settings),
            Self::LOGS_TAG => Some(Self::Logs),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchQuery(String);

impl SearchQuery {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into().trim().to_owned())
    }

    #[must_use]
    pub fn matches(&self, window: &WindowSnapshot) -> bool {
        let needle = self.0.to_ascii_lowercase();
        needle.is_empty()
            || window.title.as_str().to_ascii_lowercase().contains(&needle)
            || window
                .process_name
                .as_str()
                .to_ascii_lowercase()
                .contains(&needle)
            || window.class_name.to_ascii_lowercase().contains(&needle)
            || window.pid.to_string().contains(&needle)
            || window
                .hwnd
                .to_string()
                .to_ascii_lowercase()
                .contains(&needle)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatusKind {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusLine {
    kind: StatusKind,
    title: String,
    message: String,
    open: bool,
}

impl Default for StatusLine {
    fn default() -> Self {
        Self::info("Ready", "Refresh the window list or select an action.")
    }
}

impl StatusLine {
    #[must_use]
    pub fn info(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusKind::Info, title, message)
    }

    #[must_use]
    pub fn success(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusKind::Success, title, message)
    }

    #[must_use]
    pub fn warning(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusKind::Warning, title, message)
    }

    #[must_use]
    pub fn error(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusKind::Error, title, message)
    }

    #[must_use]
    pub fn closed() -> Self {
        Self {
            open: false,
            ..Self::default()
        }
    }

    #[must_use]
    pub const fn kind(&self) -> StatusKind {
        self.kind
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    fn new(kind: StatusKind, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            message: message.into(),
            open: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiModel {
    page: Page,
    query: SearchQuery,
    windows: Vec<WindowSnapshot>,
    monitors: Vec<MonitorSnapshot>,
    selected: Option<Hwnd>,
    aspect_preset: AspectPreset,
    custom_aspect_width: u32,
    custom_aspect_height: u32,
    scaling_algorithm: ScalingAlgorithmId,
    target_display_index: i32,
    reset_environment_on_exit: bool,
    status: StatusLine,
    busy: bool,
    watcher_running: bool,
    logs: Vec<String>,
}

impl Default for GuiModel {
    fn default() -> Self {
        Self {
            page: Page::Windows,
            query: SearchQuery::default(),
            windows: Vec::new(),
            monitors: Vec::new(),
            selected: None,
            aspect_preset: AspectPreset::default(),
            custom_aspect_width: 4,
            custom_aspect_height: 3,
            scaling_algorithm: ScalingAlgorithmId::default(),
            target_display_index: 0,
            reset_environment_on_exit: true,
            status: StatusLine::default(),
            busy: false,
            watcher_running: true,
            logs: vec!["Borderless Oxide Reactor GUI initialized.".to_owned()],
        }
    }
}

impl GuiModel {
    #[must_use]
    pub const fn page(&self) -> Page {
        self.page
    }

    #[must_use]
    pub const fn selected(&self) -> Option<Hwnd> {
        self.selected
    }

    #[must_use]
    pub fn selected_window(&self) -> Option<&WindowSnapshot> {
        self.selected
            .and_then(|hwnd| self.windows.iter().find(|window| window.hwnd == hwnd))
    }

    #[must_use]
    pub fn status(&self) -> &StatusLine {
        &self.status
    }

    #[must_use]
    pub const fn watcher_running(&self) -> bool {
        self.watcher_running
    }

    #[must_use]
    pub fn logs(&self) -> &[String] {
        &self.logs
    }

    #[must_use]
    pub fn monitors(&self) -> &[MonitorSnapshot] {
        &self.monitors
    }

    #[must_use]
    pub const fn aspect_preset(&self) -> AspectPreset {
        self.aspect_preset
    }

    #[must_use]
    pub const fn custom_aspect_width(&self) -> u32 {
        self.custom_aspect_width
    }

    #[must_use]
    pub const fn custom_aspect_height(&self) -> u32 {
        self.custom_aspect_height
    }

    #[must_use]
    pub const fn target_display_index(&self) -> i32 {
        self.target_display_index
    }

    #[must_use]
    pub const fn scaling_algorithm_id(&self) -> ScalingAlgorithmId {
        self.scaling_algorithm
    }

    #[must_use]
    pub fn scaling_algorithm(&self) -> &'static ScalingAlgorithm {
        scaling_algorithm(self.scaling_algorithm)
    }

    #[must_use]
    pub const fn reset_environment_on_exit(&self) -> bool {
        self.reset_environment_on_exit
    }

    #[must_use]
    pub fn filtered_windows(&self) -> Vec<WindowSnapshot> {
        self.windows
            .iter()
            .filter(|window| self.query.matches(window))
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn with_page(mut self, page: Page) -> Self {
        self.page = page;
        self
    }

    #[must_use]
    pub fn with_query(mut self, query: SearchQuery) -> Self {
        self.query = query;
        self
    }

    #[must_use]
    pub fn with_windows(mut self, windows: Vec<WindowSnapshot>) -> Self {
        if self
            .selected
            .is_some_and(|hwnd| !windows.iter().any(|window| window.hwnd == hwnd))
        {
            self.selected = None;
        }
        if self.selected.is_none() {
            self.selected = windows.first().map(|window| window.hwnd);
        }
        self.windows = windows;
        self
    }

    #[must_use]
    pub fn with_monitors(mut self, monitors: Vec<MonitorSnapshot>) -> Self {
        if self.target_display_index >= i32::try_from(monitors.len() + 2).unwrap_or(i32::MAX) {
            self.target_display_index = 0;
        }
        self.monitors = monitors;
        self
    }

    #[must_use]
    pub fn with_selected(mut self, selected: Option<Hwnd>) -> Self {
        self.selected = selected;
        self
    }

    #[must_use]
    pub fn with_status(mut self, status: StatusLine) -> Self {
        self.push_log(&status);
        self.status = status;
        self
    }

    #[must_use]
    pub const fn with_busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    #[must_use]
    pub const fn with_watcher_running(mut self, running: bool) -> Self {
        self.watcher_running = running;
        self
    }

    #[must_use]
    pub fn with_aspect_preset(mut self, preset: AspectPreset) -> Self {
        self.aspect_preset = preset;
        self
    }

    #[must_use]
    pub fn with_custom_aspect_width(mut self, value: u32) -> Self {
        self.custom_aspect_width = value.max(1);
        self
    }

    #[must_use]
    pub fn with_custom_aspect_height(mut self, value: u32) -> Self {
        self.custom_aspect_height = value.max(1);
        self
    }

    #[must_use]
    pub const fn with_scaling_algorithm(mut self, algorithm: ScalingAlgorithmId) -> Self {
        self.scaling_algorithm = algorithm;
        self
    }

    #[must_use]
    pub const fn with_target_display_index(mut self, index: i32) -> Self {
        self.target_display_index = index;
        self
    }

    #[must_use]
    pub const fn with_reset_environment_on_exit(mut self, reset: bool) -> Self {
        self.reset_environment_on_exit = reset;
        self
    }

    #[must_use]
    pub fn aspect_fit_options(&self) -> Option<FavoriteOptions> {
        let (width, height) = match self.aspect_preset {
            AspectPreset::FourThree => (4, 3),
            AspectPreset::SixteenNine => (16, 9),
            AspectPreset::SixteenTen => (16, 10),
            AspectPreset::CurrentWindow => self
                .selected_window()
                .map(|window| reduce_ratio(window.rect.width().0, window.rect.height().0))?,
            AspectPreset::Custom => (self.custom_aspect_width, self.custom_aspect_height),
        };

        Some(FavoriteOptions {
            size: FavoriteSize::AspectFit { width, height },
            target_frame: self.target_frame(),
            should_maximize: false,
            ..FavoriteOptions::default()
        })
    }

    #[must_use]
    pub fn upscale_pipeline(&self) -> Option<ScalingPipeline> {
        let window = self.selected_window()?;
        let output_rect = self.selected_output_rect(window);
        let algorithm = self.scaling_algorithm();

        Some(ScalingPipeline::proxy_presentation(
            CaptureBackend::GraphicsCapture,
            algorithm.backend,
            InputBackend::WindowMessageRemap,
            RendererBackend::WgpuDx12,
            window.rect,
            output_rect,
        ))
    }

    fn selected_output_rect(&self, window: &WindowSnapshot) -> borderless_core::PhysicalRect {
        match self.target_frame() {
            TargetFrame::CurrentMonitor => self
                .monitors
                .iter()
                .copied()
                .map(|monitor| (monitor, monitor.window_intersection_area(window)))
                .max_by_key(|(_, area)| *area)
                .filter(|(_, area)| *area > 0)
                .map_or(window.rect, |(monitor, _)| monitor.rect),
            TargetFrame::PrimaryMonitor => self
                .monitors
                .iter()
                .copied()
                .find(|monitor| monitor.primary)
                .map_or(window.rect, |monitor| monitor.rect),
            TargetFrame::Monitor(id) => self
                .monitors
                .iter()
                .copied()
                .find(|monitor| monitor.id == id)
                .map_or(window.rect, |monitor| monitor.rect),
            TargetFrame::Exact(rect) => rect,
        }
    }

    fn push_log(&mut self, status: &StatusLine) {
        if status.open {
            self.logs.push(format!(
                "{:?}: {} — {}",
                status.kind, status.title, status.message
            ));
        }
        if self.logs.len() > 200 {
            let remove_count = self.logs.len() - 200;
            self.logs.drain(0..remove_count);
        }
    }

    fn target_frame(&self) -> TargetFrame {
        match self.target_display_index {
            1 => TargetFrame::PrimaryMonitor,
            index if index >= 2 => usize::try_from(index - 2)
                .ok()
                .and_then(|monitor_index| self.monitors.get(monitor_index))
                .map_or(TargetFrame::CurrentMonitor, |monitor| {
                    TargetFrame::Monitor(monitor.id)
                }),
            _ => TargetFrame::CurrentMonitor,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AspectPreset {
    #[default]
    FourThree,
    SixteenNine,
    SixteenTen,
    CurrentWindow,
    Custom,
}

impl AspectPreset {
    pub const ITEMS: [&'static str; 5] = ["4:3", "16:9", "16:10", "Current window", "Custom"];

    #[must_use]
    pub const fn selected_index(self) -> i32 {
        match self {
            Self::FourThree => 0,
            Self::SixteenNine => 1,
            Self::SixteenTen => 2,
            Self::CurrentWindow => 3,
            Self::Custom => 4,
        }
    }

    #[must_use]
    pub const fn from_index(index: i32) -> Self {
        match index {
            1 => Self::SixteenNine,
            2 => Self::SixteenTen,
            3 => Self::CurrentWindow,
            4 => Self::Custom,
            _ => Self::FourThree,
        }
    }
}

fn reduce_ratio(width: i32, height: i32) -> (u32, u32) {
    let width = u32::try_from(width.max(1)).unwrap_or(1);
    let height = u32::try_from(height.max(1)).unwrap_or(1);
    let divisor = gcd(width, height);
    (width / divisor, height / divisor)
}

const fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let next = a % b;
        a = b;
        b = next;
    }
    if a == 0 { 1 } else { a }
}

#[must_use]
pub fn favorite_from_window(window: &WindowSnapshot) -> Favorite {
    let id = FavoriteId::new(format!(
        "{}-{}",
        window
            .process_name
            .as_str()
            .replace([' ', '\\', '/', ':'], "-"),
        window.pid.get()
    ))
    .expect("generated favorite id must not be empty");

    Favorite {
        id,
        enabled: true,
        matcher: FavoriteMatcher::ProcessName(window.process_name.clone()),
        options: FavoriteOptions::default(),
    }
}
