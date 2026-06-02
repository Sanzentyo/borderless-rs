use crate::locale::{Locale, Text};
use crate::model::{AspectPreset, GuiModel, Page, SearchQuery, StatusKind, StatusLine};
use crate::runtime::GuiRuntime;
use anyhow::Result;
use borderless_core::profile::ProfileSpan;
use borderless_core::{Hwnd, WindowSnapshot};
use windows_reactor::{
    App, AsyncSetState, Backdrop, ComboBox, CommandBarLabelPos, Element, ElementExt, GridLength,
    InfoBar, InfoBarSeverity, InnerConstraints, NavViewItem, NavViewPaneDisplayMode,
    NavigationView, NumberBox, RenderCx, RequestedTheme, SymbolGlyph, ThemeRef, Thickness,
    TitleBar, ToggleSwitch, VerticalAlignment, app_bar_button_icon, app_bar_separator, body,
    body_strong, border, button, caption, command_bar, grid, hstack, scroll_viewer,
    set_requested_theme, subtitle, vstack,
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

pub fn run() -> Result<()> {
    let _span = ProfileSpan::start("gui.run");
    tracing_subscriber::fmt().with_env_filter("info").init();
    ProfileSpan::mark("gui.run: tracing initialized");
    let runtime = GuiRuntime::boot()?;
    ProfileSpan::mark("gui.run: runtime booted");
    let initial_model = runtime.initial_model();
    ProfileSpan::mark("gui.run: initial model loaded");
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
    let content = match model.page() {
        Page::Windows => windows_page(runtime, &model, &set_model, text),
        Page::Favorites => favorites_page(runtime, &model, &set_model, text),
        Page::Settings => settings_page(runtime, &model, &set_model, text),
        Page::Logs => logs_page(&model, text),
    };

    let nav_view = NavigationView::new(nav_items(text), content)
        .pane_title(text.app_title)
        .header(text.page(model.page()))
        .selected_tag(model.page().tag())
        .pane_open(model.nav_pane_open())
        .pane_display_mode(NavViewPaneDisplayMode::LeftCompact)
        .settings_visible(false)
        .auto_suggest_placeholder(text.search_windows)
        .on_search_text_changed({
            let set_model = set_model.clone();
            let model = model.clone();
            move |query| set_model.call(model.clone().with_query(SearchQuery::new(query)))
        })
        .on_selection_changed({
            let set_model = set_model.clone();
            let model = model.clone();
            move |tag: String| {
                if let Some(page) = Page::from_tag(&tag) {
                    set_model.call(model.clone().with_page(page));
                }
            }
        })
        .pane_toggle_button_visible(false)
        .back_button_visible(false);

    grid((
        app_title_bar(text, &model, &set_model).grid_row(0),
        nav_view.grid_row(1),
    ))
    .rows([GridLength::Auto, GridLength::Star(1.0)])
    .columns([GridLength::Star(1.0)])
    .into()
}

fn app_title_bar(text: Text, model: &GuiModel, set_model: &AsyncSetState<GuiModel>) -> Element {
    TitleBar::new(text.app_title)
        .pane_toggle_button_visible(true)
        .on_pane_toggle_requested({
            let set_model = set_model.clone();
            let model = model.clone();
            move || {
                let next = model.nav_pane().toggled();
                set_model.call(model.clone().with_nav_pane(next));
            }
        })
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

fn windows_page(
    runtime: &GuiRuntime,
    model: &GuiModel,
    set_model: &AsyncSetState<GuiModel>,
    text: Text,
) -> Element {
    let rows = model.filtered_windows();
    let selected = model.selected_window().cloned();
    let list = if rows.is_empty() {
        empty_state(text.empty_windows_title, text.empty_windows_message)
    } else {
        vstack(window_cards(runtime, model, set_model, &rows, text))
            .spacing(12.0)
            .into()
    };

    grid((
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
        .grid_row(0),
        status_bar(model, set_model).grid_row(1),
        aspect_controls(model, set_model, text).grid_row(2),
        hstack((
            scroll_viewer(list)
                .width(LIST_PANE_WIDTH)
                .vertical_alignment(VerticalAlignment::Stretch),
            detail_panel(runtime, model, set_model, selected, text)
                .width(DETAILS_PANE_WIDTH)
                .vertical_alignment(VerticalAlignment::Stretch),
        ))
        .spacing(SECTION_SPACING)
        .grid_row(3)
        .vertical_alignment(VerticalAlignment::Stretch),
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

fn window_cards(
    runtime: &GuiRuntime,
    model: &GuiModel,
    set_model: &AsyncSetState<GuiModel>,
    windows: &[WindowSnapshot],
    text: Text,
) -> Vec<Element> {
    windows
        .iter()
        .map(|window| window_card(runtime, model, set_model, window, text))
        .collect()
}

fn window_card(
    runtime: &GuiRuntime,
    model: &GuiModel,
    set_model: &AsyncSetState<GuiModel>,
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
            vstack((
                body_strong(title).wrap(),
                caption(if selected {
                    text.selected
                } else {
                    text.targetable
                }),
            ))
            .spacing(TEXT_SPACING),
            vstack((
                caption(format!("{}: {}", text.process, process_name)).wrap(),
                caption(format!(
                    "{}: {}    {}: {}",
                    text.pid, window.pid, text.hwnd, window.hwnd
                ))
                .wrap(),
                caption(format!(
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
        .spacing(10.0)
        .padding(CARD_PADDING),
        selected,
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
                    subtitle(text.details),
                    body_strong(title).wrap(),
                    vstack((
                        caption(format!("{}: {}", text.process, process_name)).wrap(),
                        caption(format!("{}: {}", text.pid, window.pid)).wrap(),
                        caption(format!("{}: {}", text.hwnd, window.hwnd)).wrap(),
                    ))
                    .spacing(TEXT_SPACING),
                    vstack((
                        caption(format!("{}: {}", text.class, class_name)).wrap(),
                        caption(format!(
                            "{}: left={} top={}",
                            text.rect, window.rect.left.0, window.rect.top.0
                        ))
                        .wrap(),
                        caption(format!(
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
                .spacing(10.0)
                .padding(PANEL_PADDING),
                false,
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
        .spacing(16.0)
        .padding(PANEL_PADDING),
        false,
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

    let mut children = vec![body(text.favorites_message).wrap().into()];
    children.push(
        add_selected.unwrap_or_else(|| {
            empty_state(text.no_selected_window, text.no_selected_window_message)
        }),
    );

    surface(
        vstack(children)
            .spacing(SECTION_SPACING)
            .padding(PANEL_PADDING),
        false,
    )
    .margin(PAGE_PADDING)
}

fn settings_page(
    runtime: &GuiRuntime,
    model: &GuiModel,
    set_model: &AsyncSetState<GuiModel>,
    text: Text,
) -> Element {
    vstack((
        body(text.settings_intro).wrap(),
        surface(
            vstack((
                subtitle(text.environment_controls),
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
            .spacing(SECTION_SPACING)
            .padding(PANEL_PADDING),
            false,
        ),
    ))
    .spacing(SECTION_SPACING)
    .padding(PAGE_PADDING)
    .into()
}

fn logs_page(model: &GuiModel, text: Text) -> Element {
    let rows = model
        .logs()
        .iter()
        .rev()
        .take(80)
        .map(|line| caption(line).wrap().into())
        .collect::<Vec<Element>>();

    grid((
        body(text.logs_intro).wrap().grid_row(0),
        scroll_viewer(vstack(rows).spacing(4.0)).grid_row(1),
    ))
    .rows([GridLength::Auto, GridLength::Star(1.0)])
    .row_spacing(SECTION_SPACING)
    .padding(PAGE_PADDING)
    .into()
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
        vstack((subtitle(title_text), body(message).wrap()))
            .spacing(10.0)
            .padding(PANEL_PADDING),
        false,
    )
}

fn surface(content: impl Into<Element>, selected: bool) -> Element {
    let stroke = if selected {
        ThemeRef::Accent
    } else {
        ThemeRef::CardStroke
    };

    border(content)
        .corner_radius(SURFACE_RADIUS)
        .border_brush(stroke)
        .border_thickness(Thickness::uniform(1.0))
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
