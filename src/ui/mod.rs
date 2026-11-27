//! GTK4 + Libadwaita UI components for VeloShot.

pub mod carousel;
pub mod editor;
pub mod icons;
pub mod main_window;
pub mod overlay;
pub mod preferences;

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::{CssProvider, STYLE_PROVIDER_PRIORITY_APPLICATION, gdk, prelude::WidgetExt};
use libadwaita as adw;

use crate::config::{AfterCaptureAction, AppConfig};
use crate::history::{HistoryItem, HistoryStore};

/// Shared runtime state passed across windows inside the single-instance GTK application.
#[derive(Clone)]
pub struct AppState {
    pub app: adw::Application,
    pub config: Rc<RefCell<AppConfig>>,
    pub store: HistoryStore,
}

impl AppState {
    pub fn new(app: adw::Application) -> Self {
        Self {
            app,
            config: Rc::new(RefCell::new(AppConfig::load())),
            store: HistoryStore::new_default(),
        }
    }

    pub fn reload_config(&self) {
        let cfg = AppConfig::load();
        crate::i18n::set_language(cfg.language);
        *self.config.borrow_mut() = cfg;
    }
}

/// Installs custom application CSS and registers embedded symbolic icons.
pub fn install_css() {
    icons::register_icons();
    let provider = CssProvider::new();
    let mut css = String::from(
        r#"
        window.overlay-window,
        window.overlay-window.background,
        window.overlay-window toastoverlay {
            background-color: transparent;
            background-image: none;
            box-shadow: none;
        }
        .carousel-backdrop {
            background-color: rgba(0, 0, 0, 0.60);
            transition: background-color 220ms ease;
        }
        .carousel-dock {
            background-color: #1c1c22;
            border: 1px solid rgba(255, 255, 255, 0.14);
            border-radius: 18px;
            box-shadow: 0 12px 36px rgba(0, 0, 0, 0.45);
            color: #f3f4f6;
            transition: opacity 220ms ease, transform 260ms cubic-bezier(0.16, 1.0, 0.3, 1.0);
        }
        .carousel-dock.entering-bottom {
            opacity: 0.0;
            transform: translateY(32px) scale(0.97);
        }
        .carousel-dock.entering-top {
            opacity: 0.0;
            transform: translateY(-32px) scale(0.97);
        }
        .carousel-dock.entering-left {
            opacity: 0.0;
            transform: translateX(-32px) scale(0.97);
        }
        .carousel-dock.entering-right {
            opacity: 0.0;
            transform: translateX(32px) scale(0.97);
        }
        .carousel-dock.entering-center {
            opacity: 0.0;
            transform: scale(0.92);
        }
        .carousel-dock.flipping,
        .carousel-preview-stage.flipping {
            transition: none;
        }
        .carousel-dock label {
            color: #f3f4f6;
        }
        .carousel-counter {
            font-variant-numeric: tabular-nums;
            font-size: 16px;
            font-weight: 500;
            opacity: 0.88;
        }
        .carousel-preview-card {
            background-color: #181a21;
            border: 1px solid rgba(255, 255, 255, 0.14);
            border-radius: 18px;
            box-shadow: 0 16px 48px rgba(0, 0, 0, 0.55);
            padding: 14px;
        }
        .carousel-date {
            font-size: 22px;
            font-weight: 700;
            color: #ffffff;
            letter-spacing: 0.3px;
            font-variant-numeric: tabular-nums;
        }
        button.color-swatch {
            min-width: 22px;
            min-height: 22px;
            padding: 0;
            border-radius: 999px;
            border: 2px solid rgba(255, 255, 255, 0.35);
            transition: transform 130ms cubic-bezier(0.2, 0.8, 0.2, 1), box-shadow 130ms ease;
        }
        button.color-swatch:hover {
            transform: scale(1.14);
        }
        button.color-swatch.active-swatch {
            border: 2.5px solid #3584e4;
            box-shadow: 0 0 0 2px rgba(53, 132, 228, 0.35);
        }
        .editor-subbar {
            min-height: 0;
        }
        .editor-subbar spinbutton,
        .editor-subbar spinbutton > text,
        .editor-subbar spinbutton > button,
        .editor-subbar button,
        .editor-subbar menubutton > button,
        .editor-subbar .linked > button {
            min-height: 26px;
            padding-top: 2px;
            padding-bottom: 2px;
        }
        .editor-subbar spinbutton > button {
            min-width: 24px;
            padding-left: 4px;
            padding-right: 4px;
        }
        .editor-subbar button.color-swatch {
            min-width: 22px;
            min-height: 22px;
            padding: 0;
        }
        .swatch-chip {
            min-width: 18px;
            min-height: 18px;
            border-radius: 6px;
            border: 1.5px solid rgba(255, 255, 255, 0.45);
        }
        .editor-canvas-frame {
            background-color: #1c1c20;
        }
        .capture-toolbar {
            background-color: rgba(20, 22, 28, 0.84);
            border: 1px solid rgba(255, 255, 255, 0.16);
            border-radius: 18px;
            box-shadow: 0 10px 30px rgba(0, 0, 0, 0.50);
            color: #f3f4f6;
            padding: 10px 14px;
            transition: transform 340ms cubic-bezier(0.22, 1.0, 0.36, 1.0), opacity 180ms ease;
        }
        .capture-toolbar.pos-top {
            transform: translateY(0px);
        }
        .capture-toolbar.pos-top.ghost-hidden {
            opacity: 0.0;
            transform: translateY(-16px) scale(0.96);
        }
        .capture-toolbar.ghost-hidden {
            opacity: 0.0;
        }
        .capture-toolbar button {
            color: #f3f4f6;
            border-radius: 12px;
            min-width: 48px;
            min-height: 46px;
            padding: 6px 10px;
            transition: background-color 140ms ease, transform 140ms cubic-bezier(0.2, 0.8, 0.2, 1);
        }
        .capture-toolbar button:hover {
            transform: translateY(-1px) scale(1.05);
        }
        .capture-toolbar button:active {
            transform: scale(0.95);
        }
        button.tool-icon-btn,
        togglebutton.tool-icon-btn {
            min-width: 44px;
            min-height: 42px;
            padding: 6px 8px;
            transition: background-color 140ms ease, transform 120ms ease;
        }
        .tool-icon-btn.tool-size-small,
        .linked > .tool-icon-btn.tool-size-small {
            min-width: 30px;
            min-height: 30px;
            padding: 3px 5px;
        }
        .tool-icon-btn.tool-size-medium,
        .linked > .tool-icon-btn.tool-size-medium {
            min-width: 44px;
            min-height: 42px;
            padding: 6px 8px;
        }
        .tool-icon-btn.tool-size-large,
        .linked > .tool-icon-btn.tool-size-large {
            min-width: 60px;
            min-height: 56px;
            padding: 10px 12px;
        }
        .editor-bar-btn.tool-size-small,
        .linked > .editor-bar-btn.tool-size-small {
            min-width: 28px;
            min-height: 28px;
            padding: 3px 5px;
        }
        .editor-bar-btn.tool-size-medium,
        .linked > .editor-bar-btn.tool-size-medium {
            min-width: 36px;
            min-height: 36px;
            padding: 5px 8px;
        }
        .editor-bar-btn.tool-size-large,
        .linked > .editor-bar-btn.tool-size-large {
            min-width: 48px;
            min-height: 46px;
            padding: 8px 11px;
        }
        button.tool-icon-btn:hover {
            transform: scale(1.06);
        }
        button.tool-icon-btn:active {
            transform: scale(0.95);
        }
        .editor-sidebar {
            transition: opacity 200ms ease, transform 220ms cubic-bezier(0.16, 1.0, 0.3, 1.0);
        }
        .editor-sidebar.entering-left {
            opacity: 0.0;
            transform: translateX(-18px);
        }
        "#,
    );
    for (idx, (_, color)) in crate::annotations::Color::PALETTE.iter().enumerate() {
        css.push_str(&format!(
            "button.swatch-{idx} {{ background: rgb({},{},{}); }}\n",
            (color.r * 255.0) as u8,
            (color.g * 255.0) as u8,
            (color.b * 255.0) as u8
        ));
    }
    const NONE_SWATCH_BG: &str =
        "linear-gradient(135deg, #2b2e37 42%, #e01b24 45%, #e01b24 55%, #464b59 58%)";
    for (idx, (_, color)) in crate::annotations::Color::FILL_PRESETS.iter().enumerate() {
        let bg = if idx == 0 {
            NONE_SWATCH_BG.to_string()
        } else {
            format!(
                "rgba({},{},{},{:.2})",
                (color.r * 255.0) as u8,
                (color.g * 255.0) as u8,
                (color.b * 255.0) as u8,
                color.a
            )
        };
        css.push_str(&format!(
            ".swatch-chip.fill-swatch-{idx}, button.fill-swatch-{idx} {{ background: {bg}; }}\n"
        ));
    }
    for (idx, style) in crate::annotations::BeautifyStyle::ALL.iter().enumerate() {
        let bg = if *style == crate::annotations::BeautifyStyle::None {
            NONE_SWATCH_BG.to_string()
        } else {
            let (c1, c2) = style.gradient_stops();
            format!(
                "linear-gradient(135deg, rgb({},{},{}), rgb({},{},{}))",
                (c1.0 * 255.0) as u8,
                (c1.1 * 255.0) as u8,
                (c1.2 * 255.0) as u8,
                (c2.0 * 255.0) as u8,
                (c2.1 * 255.0) as u8,
                (c2.2 * 255.0) as u8
            )
        };
        css.push_str(&format!(
            ".swatch-chip.frame-swatch-{idx}, button.frame-swatch-{idx} {{ background: {bg}; }}\n"
        ));
    }
    provider.load_from_string(&css);

    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Creates an `adw::Toast` with Pango markup characters safely escaped.
pub fn plain_toast(message: &str) -> adw::Toast {
    adw::Toast::new(&gtk4::glib::markup_escape_text(message))
}

/// Starts a interactive capture flow and executes the configured post-capture action.
pub fn trigger_capture_flow(state: AppState, parent_to_hide: Option<gtk4::Window>) {
    if let Some(win) = &parent_to_hide {
        win.set_visible(false);
    }

    let rx = crate::capture::spawn_capture_async();
    gtk4::glib::MainContext::default().spawn_local(async move {
        // Short yield so a hidden parent window finishes unmapping before portal grabs screen
        gtk4::glib::timeout_future(std::time::Duration::from_millis(120)).await;
        let result = rx.recv().await;
        match result {
            Ok(Ok(full_img)) => {
                overlay::open_region_overlay(state, full_img);
            }
            Ok(Err(err)) => {
                if let Some(win) = &parent_to_hide {
                    win.set_visible(true);
                }
                crate::logging::log_warn("Aufnahme", &err.to_string());
            }
            Err(_) => {
                if let Some(win) = &parent_to_hide {
                    win.set_visible(true);
                }
            }
        }
    });
}

/// Starts an interactive portal capture (used for Wayland window selection).
pub fn trigger_interactive_capture(state: AppState) {
    let rx = crate::capture::spawn_interactive_capture_async();
    gtk4::glib::MainContext::default().spawn_local(async move {
        gtk4::glib::timeout_future(std::time::Duration::from_millis(120)).await;
        match rx.recv().await {
            Ok(Ok(img)) => handle_captured_region(state, img),
            Ok(Err(err)) => eprintln!("Fenster-Aufnahme abgebrochen/fehlgeschlagen: {err}"),
            Err(_) => {}
        }
    });
}

/// Waits `delay_secs` seconds and then opens the interactive region overlay.
pub fn trigger_delayed_capture(state: AppState, delay_secs: u64) {
    let rx = crate::capture::spawn_delayed_capture_async(delay_secs);
    gtk4::glib::MainContext::default().spawn_local(async move {
        match rx.recv().await {
            Ok(Ok(full_img)) => overlay::open_region_overlay(state, full_img),
            Ok(Err(err)) => eprintln!("Verzögerte Aufnahme fehlgeschlagen: {err}"),
            Err(_) => {}
        }
    });
}

/// Handles a freshly cropped screenshot: saves to history, optionally copies, and routes UI.
pub fn handle_captured_region(state: AppState, cropped: image::RgbaImage) {
    let cfg = state.config.borrow().clone();
    if cfg.auto_copy_clipboard
        && let Err(err) = crate::clipboard::copy_rgba_to_clipboard(&cropped)
    {
        eprintln!("Zwischenablage-Fehler: {err}");
    }

    let item: Option<HistoryItem> = match state.store.add_image(&cropped, cfg.max_history) {
        Ok(item) => Some(item),
        Err(err) => {
            eprintln!("Historie konnte nicht gespeichert werden: {err}");
            None
        }
    };

    match cfg.after_capture {
        AfterCaptureAction::OpenEditor => {
            editor::open_editor(state, cropped, item);
        }
        AfterCaptureAction::OpenCarousel => {
            carousel::open_carousel(state);
        }
        AfterCaptureAction::CopyOnly => {}
    }
}
