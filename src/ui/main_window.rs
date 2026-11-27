//! Clean GNOME Adwaita hub window for quick capture, carousel launch, and settings.

use gtk4::prelude::*;
use libadwaita as adw;

use crate::config::APP_ID;
use crate::i18n::tr;
use crate::ui::{AppState, carousel, preferences, trigger_capture_flow};

/// Opens (or focuses) the main application window.
pub fn open_main_window(state: AppState) {
    let header = adw::HeaderBar::new();
    let prefs_btn = gtk4::Button::builder()
        .icon_name("emblem-system-symbolic")
        .tooltip_text(tr("Einstellungen", "Preferences"))
        .build();
    header.pack_end(&prefs_btn);

    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    root.append(&header);

    let status_page = adw::StatusPage::builder()
        .icon_name(APP_ID)
        .title("VeloShot")
        .description(tr(
            "Schnelles Wayland-Screenshot-Tool mit Lupe, Verlaufs-Karussell und Annotations-Editor.",
            "Fast Wayland screenshot tool with live loupe, carousel history, and annotation editor.",
        ))
        .vexpand(true)
        .build();

    let action_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(12)
        .halign(gtk4::Align::Center)
        .margin_bottom(16)
        .build();

    let btn_row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(12)
        .halign(gtk4::Align::Center)
        .build();

    let capture_btn = gtk4::Button::builder()
        .label(tr("Bereich aufnehmen", "Capture Region"))
        .css_classes(["suggested-action", "pill"])
        .build();
    let carousel_btn = gtk4::Button::builder()
        .label(tr("Karussell öffnen", "Open Carousel"))
        .css_classes(["pill"])
        .build();

    btn_row.append(&capture_btn);
    btn_row.append(&carousel_btn);
    action_box.append(&btn_row);

    let format_info = |count: usize, cfg: &crate::config::AppConfig| {
        tr(
            "{count} von max. {max} Screenshots in der Historie  •  Karussell: {pos}  •  Kürzel: {cap}",
            "{count} of max {max} screenshots in history  •  Carousel: {pos}  •  Shortcut: {cap}",
        )
        .replace("{count}", &count.to_string())
        .replace("{max}", &cfg.max_history.to_string())
        .replace("{pos}", cfg.carousel_position.label())
        .replace("{cap}", &cfg.capture_shortcut)
    };

    let count = state.store.list().len();
    let cfg = state.config.borrow().clone();
    let info_lbl = gtk4::Label::builder()
        .label(format_info(count, &cfg))
        .css_classes(["dim-label"])
        .build();
    action_box.append(&info_lbl);

    status_page.set_child(Some(&action_box));
    root.append(&status_page);

    let window = adw::ApplicationWindow::builder()
        .application(&state.app)
        .title("VeloShot")
        .icon_name(APP_ID)
        .default_width(620)
        .default_height(500)
        .content(&root)
        .build();

    let state_cap = state.clone();
    let win_cap = window.clone();
    capture_btn.connect_clicked(move |_| {
        trigger_capture_flow(state_cap.clone(), Some(win_cap.clone().upcast()));
    });

    let state_car = state.clone();
    carousel_btn.connect_clicked(move |_| {
        carousel::open_carousel(state_car.clone());
    });

    let state_prefs = state.clone();
    let win_prefs = window.clone();
    prefs_btn.connect_clicked(move |_| {
        preferences::open_preferences(state_prefs.clone(), Some(win_prefs.upcast_ref()));
    });

    // Update info label whenever window becomes active again
    let state_active = state.clone();
    let info_active = info_lbl.clone();
    window.connect_is_active_notify(move |win| {
        if win.is_active() {
            let count = state_active.store.list().len();
            let cfg = state_active.config.borrow().clone();
            info_active.set_text(&format_info(count, &cfg));
        }
    });

    window.present();
}
