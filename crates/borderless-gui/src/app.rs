use crate::locale::{Locale, Text};
use crate::model::{AspectPreset, GuiModel, Page, SearchQuery, StatusKind, StatusLine};
use crate::runtime::GuiRuntime;
use anyhow::Result;
use borderless_core::profile::ProfileSpan;
use borderless_core::{Hwnd, WindowSnapshot};
use borderless_native::{Clipboard, WindowVisuals};
use std::collections::HashMap;
use windows_reactor::{
    App, AsyncSetState, Backdrop, ComboBox, CommandBarLabelPos, Element, ElementExt, GridLength,
    HorizontalAlignment, Image, ImageStretch, InfoBar, InfoBarSeverity, InnerConstraints,
    NavViewItem, NavViewPaneDisplayMode, NavigationView, NumberBox, RenderCx, RequestedTheme,
    SymbolGlyph, ThemeRef, Thickness, TitleBar, ToggleSwitch, VerticalAlignment,
    app_bar_button_icon, app_bar_separator, body, body_strong, border, button, caption,
    command_bar, grid, hstack, rich_edit_box, scroll_viewer, set_requested_theme, subtitle, title,
    vstack,
};

const SURFACE_RADIUS: f64 = 4.0;
const PAGE_PADDING: f64 = 24.0;
const PANEL_PADDING: f64 = 16.0;
const CARD_PADDING: f64 = 16.0;
const LIST_PANE_WIDTH: f64 = 520.0;
const DETAILS_PANE_WIDTH: f64 = 300.0;
const SECTION_SPACING: f64 = 16.0;
const TEXT_SPACING: f64 = 6.0;
const ACTION_SPACING: f64 = 10.0;
const NAV_EXPANDED_PANE_WIDTH: f64 = 480.0;
const PAGE_TITLE_FONT_SIZE: f64 = 24.0;
const SECTION_TITLE_FONT_SIZE: f64 = 18.0;
const BODY_FONT_SIZE: f64 = 13.0;
const BODY_STRONG_FONT_SIZE: f64 = 13.0;
const CAPTION_FONT_SIZE: f64 = 12.0;
const ICON_BOX_SIZE: f64 = 32.0;
const PREVIEW_WIDTH: f64 = 268.0;
const PREVIEW_HEIGHT: f64 = 164.0;
const VISUAL_ICON_PREFETCH: usize = 32;

pub fn run() -> Result<()> {
    let _span = ProfileSpan::start("gui.run");
    tracing_subscriber::fmt().with_env_filter("info").init();
    ProfileSpan::mark("gui.run: tracing initialized");
    let runtime = GuiRuntime::boot()?;
    ProfileSpan::mark("gui.run: runtime booted");
    let initial_model = GuiModel::default()
        .with_busy(true)
        .with_status(StatusLine::info(
            "Loading windows",
            "The window list is loading in the background.",
        ));
    ProfileSpan::mark("gui.run: initial model prepared");
    let text = Locale::detect().tr();
    ProfileSpan::mark("gui.run: locale detected");
    App::new()
        .title("Borderless Oxide")
        .inner_size(1120.0, 720.0)
        .inner_constraints(InnerConstraints {
            min_width: Some(960.0),
            min_height: Some(640.0),
            max_width: None,
            max_height: None,
        })
        .backdrop(Backdrop::Mica)
        .render(move |cx| render_root(cx, &runtime, initial_model.clone(), text))?;
    ProfileSpan::mark("gui.run: render returned");
    Ok(())
}

fn render_root(
    cx: &mut RenderCx,
    runtime: &GuiRuntime,
    initial_model: GuiModel,
    text: Text,
) -> Element {
    set_requested_theme(RequestedTheme::Default);
    let (model, set_model) = cx.use_async_state(initial_model);
    let (visuals, set_visuals) = cx.use_async_state(VisualCache::default());
    schedule_initial_refresh(cx, runtime, &model, &set_model);
    schedule_visual_loading(cx, &model, &visuals, &set_visuals);
    let inner_width = cx.use_inner_size().width;
    let content = match model.page() {
        Page::Windows => windows_page(runtime, &model, &set_model, &visuals, text),
        Page::Favorites => favorites_page(runtime, &model, &set_model, text),
        Page::Settings => settings_page(runtime, &model, &set_model, text),
        Page::Logs => logs_page(&model, &set_model, text),
    };

    let nav_layout = NavLayout::for_page(model.page(), inner_width);
    let nav_view = NavigationView::new(nav_items(text), content)
        .with_key(nav_key(nav_layout.pane_display_mode))
        .pane_title(text.app_title)
        .selected_tag(model.page().tag())
        .pane_display_mode(nav_layout.pane_display_mode);
    let nav_view = if nav_layout.pane_open {
        nav_view.pane_open(nav_layout.pane_open)
    } else {
        nav_view
    };
    let nav_view = nav_view
        .settings_visible(false)
        .auto_suggest_placeholder(text.search_windows)
        .on_search_text_changed({
            let set_model = set_model.clone();
            let model = model.clone();
            move |query| set_model.call(model.clone().with_query(SearchQuery::new(query)))
        })
        .on_selection_changed({
            move |tag: String| {
                if let Some(page) = Page::from_tag(&tag) {
                    set_model.call(model.clone().with_page(page));
                }
            }
        })
        .pane_toggle_button_visible(true)
        .back_button_visible(false);

    grid((app_title_bar(text).grid_row(0), nav_view.grid_row(1)))
        .rows([GridLength::Auto, GridLength::Star(1.0)])
        .columns([GridLength::Star(1.0)])
        .into()
}

fn schedule_initial_refresh(
    cx: &mut RenderCx,
    runtime: &GuiRuntime,
    model: &GuiModel,
    set_model: &AsyncSetState<GuiModel>,
) {
    let runtime = runtime.clone();
    let set_model = set_model.clone();
    let model = model.clone();
    cx.use_effect((), move || {
        ProfileSpan::mark("gui.render: scheduling initial refresh");
        runtime.refresh_windows(set_model, model);
    });
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct NavLayout {
    pane_open: bool,
    pane_display_mode: NavViewPaneDisplayMode,
}

impl NavLayout {
    fn for_page(page: Page, inner_width: f64) -> Self {
        if LayoutMetrics::for_page(page).expanded_navigation_fits(inner_width) {
            Self {
                pane_open: true,
                pane_display_mode: NavViewPaneDisplayMode::Left,
            }
        } else {
            Self {
                pane_open: false,
                pane_display_mode: NavViewPaneDisplayMode::LeftCompact,
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct VisualCache {
    icons: HashMap<Hwnd, Option<String>>,
}

impl VisualCache {
    fn icon_uri(&self, hwnd: Hwnd) -> Option<&str> {
        self.icons
            .get(&hwnd)
            .and_then(|uri| uri.as_ref().map(String::as_str))
    }
}

fn schedule_visual_loading(
    cx: &mut RenderCx,
    model: &GuiModel,
    visuals: &VisualCache,
    set_visuals: &AsyncSetState<VisualCache>,
) {
    let icon_requests = model
        .filtered_windows()
        .into_iter()
        .take(VISUAL_ICON_PREFETCH)
        .filter(|window| !visuals.icons.contains_key(&window.hwnd))
        .map(|window| window.hwnd)
        .collect::<Vec<_>>();
    let deps = icon_requests.clone();

    if icon_requests.is_empty() {
        cx.use_effect(deps, || {});
        return;
    }

    let mut next = visuals.clone();
    let set_visuals = set_visuals.clone();
    cx.use_effect(deps, move || {
        std::thread::spawn(move || {
            let native = WindowVisuals;
            for hwnd in icon_requests {
                let uri = native.icon_uri(hwnd).ok().flatten();
                next.icons.insert(hwnd, uri);
            }
            set_visuals.call(next);
        });
    });
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LayoutMetrics {
    content_min_width: f64,
}

impl LayoutMetrics {
    fn for_page(page: Page) -> Self {
        let content_min_width = match page {
            Page::Windows => {
                PAGE_PADDING.mul_add(2.0, LIST_PANE_WIDTH + SECTION_SPACING + DETAILS_PANE_WIDTH)
            }
            Page::Favorites | Page::Settings | Page::Logs => {
                PAGE_PADDING.mul_add(2.0, LIST_PANE_WIDTH)
            }
        };

        Self { content_min_width }
    }

    fn expanded_navigation_fits(self, inner_width: f64) -> bool {
        inner_width >= self.content_min_width + NAV_EXPANDED_PANE_WIDTH
    }
}

const fn nav_key(mode: NavViewPaneDisplayMode) -> &'static str {
    match mode {
        NavViewPaneDisplayMode::Left => "nav-left",
        NavViewPaneDisplayMode::LeftCompact => "nav-left-compact",
        NavViewPaneDisplayMode::Auto => "nav-auto",
        NavViewPaneDisplayMode::Top => "nav-top",
        NavViewPaneDisplayMode::LeftMinimal => "nav-left-minimal",
    }
}

fn app_title_bar(text: Text) -> Element {
    TitleBar::new(text.app_title)
        .pane_toggle_button_visible(false)
        .tall(false)
        .into()
}

fn nav_items(text: Text) -> Vec<NavViewItem> {
    vec![
        NavViewItem::new(text.nav_windows)
            .tag(Page::WINDOWS_TAG)
            .icon(SymbolGlyph::World),
        NavViewItem::new(text.nav_favorites)
            .tag(Page::FAVORITES_TAG)
            .icon(SymbolGlyph::Favorite),
        NavViewItem::new(text.nav_settings)
            .tag(Page::SETTINGS_TAG)
            .icon(SymbolGlyph::Setting),
        NavViewItem::new(text.nav_logs)
            .tag(Page::LOGS_TAG)
            .icon(SymbolGlyph::Flag),
    ]
}

fn page_title(label: &'static str) -> Element {
    title(label)
        .font_size(PAGE_TITLE_FONT_SIZE)
        .margin(Thickness::xy(PAGE_PADDING, 0.0))
        .into()
}

fn section_title(label: impl Into<String>) -> Element {
    subtitle(label).font_size(SECTION_TITLE_FONT_SIZE).into()
}

fn strong_text(label: impl Into<String>) -> windows_reactor::TextBlock {
    body_strong(label).font_size(BODY_STRONG_FONT_SIZE)
}

fn body_text(label: impl Into<String>) -> windows_reactor::TextBlock {
    body(label).font_size(BODY_FONT_SIZE)
}

fn meta_text(label: impl Into<String>) -> windows_reactor::TextBlock {
    caption(label).font_size(CAPTION_FONT_SIZE)
}

fn window_icon(uri: Option<&str>, process_name: &str) -> Element {
    uri.map_or_else(
        || {
            let label = process_name
                .chars()
                .find(char::is_ascii_alphanumeric)
                .map_or_else(|| "?".to_owned(), |ch| ch.to_ascii_uppercase().to_string());
            border(
                body_text(label)
                    .font_size(BODY_STRONG_FONT_SIZE)
                    .horizontal_alignment(HorizontalAlignment::Center),
            )
            .width(ICON_BOX_SIZE)
            .height(ICON_BOX_SIZE)
            .corner_radius(SURFACE_RADIUS)
            .background(ThemeRef::ControlFillTertiary)
            .padding(Thickness::uniform(6.0))
            .into()
        },
        |uri| {
            Image::new(uri)
                .stretch(ImageStretch::Uniform)
                .width(ICON_BOX_SIZE)
                .height(ICON_BOX_SIZE)
                .into()
        },
    )
}

fn preview_panel(text: Text) -> Element {
    vstack((
        meta_text(text.preview),
        border(
            body_text(text.preview_unavailable)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center),
        )
        .width(PREVIEW_WIDTH)
        .height(PREVIEW_HEIGHT)
        .corner_radius(SURFACE_RADIUS)
        .background(ThemeRef::ControlFillTertiary)
        .padding(Thickness::uniform(8.0)),
    ))
    .spacing(TEXT_SPACING)
    .into()
}

fn windows_page(
    runtime: &GuiRuntime,
    model: &GuiModel,
    set_model: &AsyncSetState<GuiModel>,
    visuals: &VisualCache,
    text: Text,
) -> Element {
    let rows = model.filtered_windows();
    let selected = model.selected_window().cloned();
    let list = if rows.is_empty() {
        empty_state(text.empty_windows_title, text.empty_windows_message)
    } else {
        vstack(window_cards(
            runtime, model, set_model, visuals, &rows, text,
        ))
        .spacing(12.0)
        .into()
    };

    grid((
        page_title(text.nav_windows).grid_row(0),
        command_bar(vec![
            app_bar_button_icon(text.refresh, SymbolGlyph::Sync),
            app_bar_button_icon(text.apply_selected, SymbolGlyph::Play),
            app_bar_button_icon(text.apply_aspect_fit, SymbolGlyph::Find),
            app_bar_button_icon(text.restore_selected, SymbolGlyph::Undo),
            app_bar_separator(),
            app_bar_button_icon(text.add_favorite, SymbolGlyph::Add),
        ])
        .default_label_position(CommandBarLabelPos::Right)
        .on_click({
            let runtime = runtime.clone();
            let set_model = set_model.clone();
            let model = model.clone();
            move |command: String| {
                handle_windows_command(&runtime, &set_model, model.clone(), &command, text);
            }
        })
        .grid_row(1),
        status_bar(model, set_model).grid_row(2),
        aspect_controls(model, set_model, text).grid_row(3),
        grid((
            scroll_viewer(list)
                .width(LIST_PANE_WIDTH)
                .vertical_alignment(VerticalAlignment::Stretch)
                .grid_column(0),
            detail_panel(runtime, model, set_model, selected, text)
                .min_width(DETAILS_PANE_WIDTH)
                .vertical_alignment(VerticalAlignment::Stretch)
                .grid_column(1),
        ))
        .columns([GridLength::Auto, GridLength::Star(1.0)])
        .column_spacing(SECTION_SPACING)
        .grid_row(4)
        .vertical_alignment(VerticalAlignment::Stretch),
    ))
    .rows([
        GridLength::Auto,
        GridLength::Auto,
        GridLength::Auto,
        GridLength::Auto,
        GridLength::Star(1.0),
    ])
    .row_spacing(SECTION_SPACING)
    .padding(PAGE_PADDING)
    .into()
}

fn window_cards(
    runtime: &GuiRuntime,
    model: &GuiModel,
    set_model: &AsyncSetState<GuiModel>,
    visuals: &VisualCache,
    windows: &[WindowSnapshot],
    text: Text,
) -> Vec<Element> {
    windows
        .iter()
        .map(|window| window_card(runtime, model, set_model, visuals, window, text))
        .collect()
}

fn window_card(
    runtime: &GuiRuntime,
    model: &GuiModel,
    set_model: &AsyncSetState<GuiModel>,
    visuals: &VisualCache,
    window: &WindowSnapshot,
    text: Text,
) -> Element {
    let hwnd = window.hwnd;
    let selected = model.selected() == Some(hwnd);
    let title = truncate_middle(&title_or_placeholder(window), 48);
    let process_name = truncate_middle(&window.process_name.to_string(), 40);
    let class_name = truncate_middle(&window.class_name, 40);

    surface(
        vstack((
            hstack((
                window_icon(visuals.icon_uri(hwnd), window.process_name.as_str()),
                vstack((
                    strong_text(title).wrap(),
                    meta_text(if selected {
                        text.selected
                    } else if window.is_borderless_like() {
                        text.borderless
                    } else {
                        text.targetable
                    }),
                ))
                .spacing(TEXT_SPACING),
            ))
            .spacing(ACTION_SPACING),
            vstack((
                meta_text(format!("{}: {}", text.process, process_name)).wrap(),
                meta_text(format!(
                    "{}: {}    {}: {}",
                    text.pid, window.pid, text.hwnd, window.hwnd
                ))
                .wrap(),
                meta_text(format!(
                    "{}: {}    {}: {}x{}",
                    text.class,
                    class_name,
                    text.size,
                    window.rect.width().0,
                    window.rect.height().0
                ))
                .wrap(),
            ))
            .spacing(TEXT_SPACING),
            hstack((
                button(text.apply).accent().on_click({
                    let runtime = runtime.clone();
                    let set_model = set_model.clone();
                    let model = model.clone().with_busy(true).with_selected(Some(hwnd));
                    move || runtime.apply_window(hwnd, set_model.clone(), model.clone())
                }),
                button(text.apply_aspect_fit).on_click({
                    let runtime = runtime.clone();
                    let set_model = set_model.clone();
                    let model = model.clone().with_selected(Some(hwnd));
                    move || {
                        apply_aspect_fit(&runtime, hwnd, set_model.clone(), model.clone());
                    }
                }),
                button(text.restore).on_click({
                    let runtime = runtime.clone();
                    let set_model = set_model.clone();
                    let model = model.clone().with_busy(true).with_selected(Some(hwnd));
                    move || runtime.restore_window(hwnd, set_model.clone(), model.clone())
                }),
                button(text.favorite).subtle().on_click({
                    let runtime = runtime.clone();
                    let set_model = set_model.clone();
                    let model = model.clone().with_busy(true).with_selected(Some(hwnd));
                    let favorite_window = window.clone();
                    move || {
                        runtime.add_favorite_from_window(
                            favorite_window.clone(),
                            set_model.clone(),
                            model.clone(),
                        );
                    }
                }),
            ))
            .spacing(ACTION_SPACING),
        ))
        .spacing(10.0),
        selected,
        CARD_PADDING,
    )
    .width(LIST_PANE_WIDTH)
    .on_tapped({
        let set_model = set_model.clone();
        let model = model.clone();
        move || set_model.call(model.clone().with_selected(Some(hwnd)))
    })
}

fn detail_panel(
    runtime: &GuiRuntime,
    model: &GuiModel,
    set_model: &AsyncSetState<GuiModel>,
    selected: Option<WindowSnapshot>,
    text: Text,
) -> Element {
    match selected {
        Some(window) => {
            let title = truncate_middle(&title_or_placeholder(&window), 56);
            let process_name = truncate_middle(&window.process_name.to_string(), 44);
            let class_name = truncate_middle(&window.class_name, 44);

            surface(
                vstack((
                    section_title(text.details),
                    preview_panel(text),
                    strong_text(title).wrap(),
                    vstack((
                        meta_text(format!("{}: {}", text.process, process_name)).wrap(),
                        meta_text(format!("{}: {}", text.pid, window.pid)).wrap(),
                        meta_text(format!("{}: {}", text.hwnd, window.hwnd)).wrap(),
                    ))
                    .spacing(TEXT_SPACING),
                    vstack((
                        meta_text(format!("{}: {}", text.class, class_name)).wrap(),
                        meta_text(format!(
                            "{}: left={} top={}",
                            text.rect, window.rect.left.0, window.rect.top.0
                        ))
                        .wrap(),
                        meta_text(format!(
                            "right={} bottom={}",
                            window.rect.right.0, window.rect.bottom.0
                        ))
                        .wrap(),
                    ))
                    .spacing(TEXT_SPACING),
                    hstack((
                        button(text.apply).accent().on_click({
                            let runtime = runtime.clone();
                            let set_model = set_model.clone();
                            let model = model.clone().with_busy(true);
                            let hwnd = window.hwnd;
                            move || runtime.apply_window(hwnd, set_model.clone(), model.clone())
                        }),
                        button(text.apply_aspect_fit).on_click({
                            let runtime = runtime.clone();
                            let set_model = set_model.clone();
                            let model = model.clone();
                            let hwnd = window.hwnd;
                            move || {
                                apply_aspect_fit(&runtime, hwnd, set_model.clone(), model.clone());
                            }
                        }),
                        button(text.restore).on_click({
                            let runtime = runtime.clone();
                            let set_model = set_model.clone();
                            let model = model.clone().with_busy(true);
                            let hwnd = window.hwnd;
                            move || runtime.restore_window(hwnd, set_model.clone(), model.clone())
                        }),
                    ))
                    .spacing(ACTION_SPACING),
                ))
                .spacing(10.0),
                false,
                PANEL_PADDING,
            )
        }
        None => empty_state(text.empty_selection_title, text.empty_selection_message),
    }
}

fn aspect_controls(model: &GuiModel, set_model: &AsyncSetState<GuiModel>, text: Text) -> Element {
    surface(
        hstack((
            ComboBox::new(AspectPreset::ITEMS)
                .header(text.aspect_preset)
                .selected_index(model.aspect_preset().selected_index())
                .on_selection_changed({
                    let set_model = set_model.clone();
                    let model = model.clone();
                    move |index| {
                        set_model.call(
                            model
                                .clone()
                                .with_aspect_preset(AspectPreset::from_index(index)),
                        );
                    }
                }),
            NumberBox::new(f64::from(model.custom_aspect_width()))
                .header(text.aspect_width)
                .range(1.0, 9999.0)
                .enabled(matches!(model.aspect_preset(), AspectPreset::Custom))
                .on_value_changed({
                    let set_model = set_model.clone();
                    let model = model.clone();
                    move |value| {
                        set_model.call(
                            model
                                .clone()
                                .with_custom_aspect_width(f64_to_positive_u32(value)),
                        );
                    }
                }),
            NumberBox::new(f64::from(model.custom_aspect_height()))
                .header(text.aspect_height)
                .range(1.0, 9999.0)
                .enabled(matches!(model.aspect_preset(), AspectPreset::Custom))
                .on_value_changed({
                    let set_model = set_model.clone();
                    let model = model.clone();
                    move |value| {
                        set_model.call(
                            model
                                .clone()
                                .with_custom_aspect_height(f64_to_positive_u32(value)),
                        );
                    }
                }),
            ComboBox::new(display_items(model, text))
                .header(text.target_display)
                .selected_index(model.target_display_index())
                .on_selection_changed({
                    let set_model = set_model.clone();
                    let model = model.clone();
                    move |index| set_model.call(model.clone().with_target_display_index(index))
                }),
        ))
        .spacing(16.0),
        false,
        PANEL_PADDING,
    )
}

fn display_items(model: &GuiModel, text: Text) -> Vec<String> {
    let mut items = vec![
        text.display_current.to_owned(),
        text.display_primary.to_owned(),
    ];
    items.extend(model.monitors().iter().enumerate().map(|(index, monitor)| {
        format!(
            "{} {}: {}x{}",
            text.display_monitor,
            index + 1,
            monitor.rect.width().0,
            monitor.rect.height().0
        )
    }));
    items
}

fn f64_to_positive_u32(value: f64) -> u32 {
    if value.is_finite() && value >= 1.0 {
        let rounded = value.round().clamp(1.0, f64::from(u32::MAX));
        format!("{rounded:.0}").parse::<u32>().unwrap_or(1)
    } else {
        1
    }
}

fn apply_aspect_fit(
    runtime: &GuiRuntime,
    hwnd: Hwnd,
    set_model: AsyncSetState<GuiModel>,
    model: GuiModel,
) {
    match model.aspect_fit_options() {
        Some(options) => {
            runtime.apply_window_with_options(hwnd, options, set_model, model.with_busy(true));
        }
        None => set_model.call(model.with_status(StatusLine::warning(
            "No aspect ratio",
            "Select a window or enter a custom aspect ratio before applying aspect fit.",
        ))),
    }
}

fn favorites_page(
    runtime: &GuiRuntime,
    model: &GuiModel,
    set_model: &AsyncSetState<GuiModel>,
    text: Text,
) -> Element {
    let selected = model.selected_window().cloned();
    let add_selected = selected.map(|window| {
        button(text.add_selected_favorite)
            .accent()
            .on_click({
                let runtime = runtime.clone();
                let set_model = set_model.clone();
                let model = model.clone().with_busy(true);
                move || {
                    runtime.add_favorite_from_window(
                        window.clone(),
                        set_model.clone(),
                        model.clone(),
                    );
                }
            })
            .into()
    });

    let mut children = vec![body_text(text.favorites_message).wrap().into()];
    children.push(
        add_selected.unwrap_or_else(|| {
            empty_state(text.no_selected_window, text.no_selected_window_message)
        }),
    );

    vstack((
        page_title(text.nav_favorites),
        surface(
            vstack(children).spacing(SECTION_SPACING),
            false,
            PANEL_PADDING,
        ),
    ))
    .spacing(SECTION_SPACING)
    .padding(PAGE_PADDING)
    .into()
}

fn settings_page(
    runtime: &GuiRuntime,
    model: &GuiModel,
    set_model: &AsyncSetState<GuiModel>,
    text: Text,
) -> Element {
    vstack((
        page_title(text.nav_settings),
        body_text(text.settings_intro).wrap(),
        surface(
            vstack((
                section_title(text.environment_controls),
                hstack((
                    button(text.show_taskbar).on_click({
                        let runtime = runtime.clone();
                        let set_model = set_model.clone();
                        let model = model.clone().with_busy(true);
                        move || runtime.set_taskbar_visible(true, set_model.clone(), model.clone())
                    }),
                    button(text.hide_taskbar).on_click({
                        let runtime = runtime.clone();
                        let set_model = set_model.clone();
                        let model = model.clone().with_busy(true);
                        move || runtime.set_taskbar_visible(false, set_model.clone(), model.clone())
                    }),
                    button(text.show_cursor).on_click({
                        let runtime = runtime.clone();
                        let set_model = set_model.clone();
                        let model = model.clone().with_busy(true);
                        move || runtime.set_cursor_visible(true, set_model.clone(), model.clone())
                    }),
                    button(text.hide_cursor).on_click({
                        let runtime = runtime.clone();
                        let set_model = set_model.clone();
                        let model = model.clone().with_busy(true);
                        move || runtime.set_cursor_visible(false, set_model.clone(), model.clone())
                    }),
                ))
                .spacing(ACTION_SPACING),
                ToggleSwitch::new(model.watcher_running())
                    .header(text.watcher_actor)
                    .on_content(text.running)
                    .off_content(text.paused)
                    .on_changed({
                        let set_model = set_model.clone();
                        let model = model.clone();
                        move |running| {
                            let status = if running {
                                StatusLine::info(
                                    "Watcher UI state",
                                    "The watcher actor is still supervised by the runtime.",
                                )
                            } else {
                                StatusLine::warning(
                                    "Watcher pause not wired",
                                    "Controller pause/resume message is planned next.",
                                )
                            };
                            set_model.call(
                                model
                                    .clone()
                                    .with_watcher_running(running)
                                    .with_status(status),
                            );
                        }
                    }),
                ToggleSwitch::new(model.reset_environment_on_exit())
                    .header(text.reset_environment_on_exit)
                    .on_content(text.running)
                    .off_content(text.paused)
                    .on_changed({
                        let runtime = runtime.clone();
                        let set_model = set_model.clone();
                        let model = model.clone();
                        move |reset| {
                            runtime.set_reset_environment_on_exit(reset);
                            set_model.call(model.clone().with_reset_environment_on_exit(reset));
                        }
                    }),
            ))
            .spacing(SECTION_SPACING),
            false,
            PANEL_PADDING,
        ),
    ))
    .spacing(SECTION_SPACING)
    .padding(PAGE_PADDING)
    .into()
}

fn logs_page(model: &GuiModel, set_model: &AsyncSetState<GuiModel>, text: Text) -> Element {
    let logs = recent_logs_text(model);

    grid((
        page_title(text.nav_logs).grid_row(0),
        command_bar(vec![app_bar_button_icon(text.copy_logs, SymbolGlyph::Copy)])
            .default_label_position(CommandBarLabelPos::Right)
            .on_click({
                let set_model = set_model.clone();
                let model = model.clone();
                let logs = logs.clone();
                move |command: String| {
                    if command == text.copy_logs {
                        let status = match Clipboard.set_text(&logs) {
                            Ok(()) => {
                                StatusLine::success(text.logs_copied, text.logs_copied_message)
                            }
                            Err(err) => StatusLine::error("Copy failed", err.to_string()),
                        };
                        set_model.call(model.clone().with_status(status));
                    }
                }
            })
            .grid_row(1),
        body_text(text.logs_intro).wrap().grid_row(2),
        rich_edit_box(logs)
            .read_only()
            .font_size(CAPTION_FONT_SIZE)
            .vertical_alignment(VerticalAlignment::Stretch)
            .grid_row(3),
    ))
    .rows([
        GridLength::Auto,
        GridLength::Auto,
        GridLength::Auto,
        GridLength::Star(1.0),
    ])
    .row_spacing(SECTION_SPACING)
    .padding(PAGE_PADDING)
    .into()
}

fn recent_logs_text(model: &GuiModel) -> String {
    model
        .logs()
        .iter()
        .rev()
        .take(80)
        .cloned()
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn handle_windows_command(
    runtime: &GuiRuntime,
    set_model: &AsyncSetState<GuiModel>,
    model: GuiModel,
    command: &str,
    text: Text,
) {
    match command {
        command if command == text.refresh => {
            set_model.call(model.clone().with_busy(true));
            runtime.refresh_windows(set_model.clone(), model.with_busy(true));
        }
        command if command == text.apply_selected => match model.selected() {
            Some(hwnd) => runtime.apply_window(hwnd, set_model.clone(), model.with_busy(true)),
            None => set_model.call(model.with_status(StatusLine::warning(
                "No window selected",
                "Select a target window before applying borderless.",
            ))),
        },
        command if command == text.apply_aspect_fit => match model.selected() {
            Some(hwnd) => {
                apply_aspect_fit(runtime, hwnd, set_model.clone(), model);
            }
            None => set_model.call(model.with_status(StatusLine::warning(
                "No window selected",
                "Select a target window before applying aspect fit.",
            ))),
        },
        command if command == text.restore_selected => match model.selected() {
            Some(hwnd) => runtime.restore_window(hwnd, set_model.clone(), model.with_busy(true)),
            None => set_model.call(model.with_status(StatusLine::warning(
                "No window selected",
                "Select a target window before restoring it.",
            ))),
        },
        command if command == text.add_favorite => match model.selected_window().cloned() {
            Some(window) => {
                runtime.add_favorite_from_window(window, set_model.clone(), model.with_busy(true));
            }
            None => set_model.call(model.with_status(StatusLine::warning(
                "No window selected",
                "Select a target window before adding a favorite.",
            ))),
        },
        _ => {}
    }
}

fn status_bar(model: &GuiModel, set_model: &AsyncSetState<GuiModel>) -> Element {
    let status = model.status();
    InfoBar::new(status.title())
        .message(status.message())
        .severity(match status.kind() {
            StatusKind::Info => InfoBarSeverity::Informational,
            StatusKind::Success => InfoBarSeverity::Success,
            StatusKind::Warning => InfoBarSeverity::Warning,
            StatusKind::Error => InfoBarSeverity::Error,
        })
        .is_open(status.is_open())
        .is_closable(true)
        .on_close({
            let set_model = set_model.clone();
            let model = model.clone();
            move || set_model.call(model.clone().with_status(StatusLine::closed()))
        })
        .into()
}

fn empty_state(title_text: impl Into<String>, message: impl Into<String>) -> Element {
    surface(
        vstack((section_title(title_text), body_text(message).wrap())).spacing(10.0),
        false,
        PANEL_PADDING,
    )
}

fn surface(content: impl Into<Element>, selected: bool, padding: f64) -> Element {
    let stroke = if selected {
        ThemeRef::Accent
    } else {
        ThemeRef::CardStroke
    };

    border(content)
        .corner_radius(SURFACE_RADIUS)
        .border_brush(stroke)
        .border_thickness(Thickness::uniform(1.0))
        .padding(padding)
        .background(ThemeRef::CardBackground)
        .into()
}

fn title_or_placeholder(window: &WindowSnapshot) -> String {
    let title = window.title.as_str();
    if title.is_empty() {
        "(untitled window)".to_owned()
    } else {
        title.to_owned()
    }
}

fn truncate_middle(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_owned();
    }

    if max_chars <= 3 {
        return "...".chars().take(max_chars).collect();
    }

    let keep = max_chars - 3;
    let prefix_len = keep / 2;
    let suffix_len = keep - prefix_len;
    let prefix = value.chars().take(prefix_len).collect::<String>();
    let suffix = value
        .chars()
        .rev()
        .take(suffix_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();

    format!("{prefix}...{suffix}")
}

#[allow(dead_code)]
fn _keep_hwnd(_: Hwnd) {}
