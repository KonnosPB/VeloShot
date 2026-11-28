//! System tray / StatusNotifierItem integration via pure-Rust `ksni`.

use std::io::Cursor;
use std::sync::OnceLock;

use gtk4::cairo;
use image::ImageFormat;
use ksni::menu::{MenuItem, StandardItem};
use ksni::{Icon, ToolTip, Tray, TrayMethods};

use crate::annotations::{cairo_surface_to_rgba, draw_rounded_rect};
use crate::i18n::tr;

/// Commands triggered from the desktop top-bar / taskbar tray icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    Capture,
    History,
    MainWindow,
    Settings,
    Quit,
}

struct SnapTray {
    tx: async_channel::Sender<TrayCommand>,
}

impl SnapTray {
    fn send(&self, cmd: TrayCommand) {
        let _ = self.tx.try_send(cmd);
    }
}

impl Tray for SnapTray {
    fn id(&self) -> String {
        "veloshot".to_string()
    }

    fn title(&self) -> String {
        "VeloShot".to_string()
    }

    fn icon_name(&self) -> String {
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        tray_icon_pixmaps()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "VeloShot".to_string(),
            description: tr(
                "Klick: Bereich aufnehmen • Rechtsklick: Karussell & Einstellungen",
                "Click: Capture region • Right-click: Carousel & Settings",
            )
            .to_string(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayCommand::Capture);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let tx_cap = self.tx.clone();
        let tx_hist = self.tx.clone();
        let tx_win = self.tx.clone();
        let tx_prefs = self.tx.clone();
        let tx_quit = self.tx.clone();
        let icons = tray_menu_icons();

        vec![
            MenuItem::Standard(StandardItem {
                label: tr("Bereich aufnehmen", "Capture Region").to_string(),
                icon_name: String::new(),
                icon_data: icons.capture.clone(),
                activate: Box::new(move |_| {
                    let _ = tx_cap.try_send(TrayCommand::Capture);
                }),
                ..Default::default()
            }),
            MenuItem::Standard(StandardItem {
                label: tr("Verlauf-Karussell öffnen", "Open Carousel History").to_string(),
                icon_name: String::new(),
                icon_data: icons.history.clone(),
                activate: Box::new(move |_| {
                    let _ = tx_hist.try_send(TrayCommand::History);
                }),
                ..Default::default()
            }),
            MenuItem::Standard(StandardItem {
                label: tr("Hauptfenster öffnen", "Open Main Window").to_string(),
                icon_name: String::new(),
                icon_data: icons.window.clone(),
                activate: Box::new(move |_| {
                    let _ = tx_win.try_send(TrayCommand::MainWindow);
                }),
                ..Default::default()
            }),
            MenuItem::Standard(StandardItem {
                label: tr("Einstellungen", "Preferences").to_string(),
                icon_name: String::new(),
                icon_data: icons.settings.clone(),
                activate: Box::new(move |_| {
                    let _ = tx_prefs.try_send(TrayCommand::Settings);
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(StandardItem {
                label: tr("Beenden", "Quit").to_string(),
                icon_name: String::new(),
                icon_data: icons.quit.clone(),
                activate: Box::new(move |_| {
                    let _ = tx_quit.try_send(TrayCommand::Quit);
                }),
                ..Default::default()
            }),
        ]
    }
}

/// Spawns the StatusNotifierItem tray service in the background Tokio runtime.
pub fn spawn_tray() -> async_channel::Receiver<TrayCommand> {
    let (tx, rx) = async_channel::unbounded();
    crate::capture::tokio_runtime().spawn(async move {
        let tray = SnapTray { tx };
        let res = if std::env::var_os("SNAP").is_some() {
            tray.disable_dbus_name(true).spawn().await
        } else {
            tray.spawn().await
        };
        match res {
            Ok(handle) => {
                // Keep the tray handle alive for the lifetime of the daemon process.
                std::future::pending::<()>().await;
                drop(handle);
            }
            Err(err) => {
                eprintln!("Taskleisten-Symbol konnte nicht registriert werden: {err}");
            }
        }
    });
    rx
}

/// Returns cached multi-size branded ARGB32 pixmaps (`22`, `32`, `48`, `64` px) for sharp panel rendering.
fn tray_icon_pixmaps() -> Vec<Icon> {
    static CACHE: OnceLock<Vec<Icon>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            [22, 32, 48, 64]
                .into_iter()
                .map(render_branded_tray_icon)
                .collect()
        })
        .clone()
}

/// Renders the VeloShot squircle + viewfinder + lens emblem via Cairo into SNI ARGB32 format.
fn render_branded_tray_icon(size: i32) -> Icon {
    let s = size as f64;
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, size, size).expect("cairo surface");
    {
        let cr = cairo::Context::new(&surface).expect("cairo context");
        cr.set_operator(cairo::Operator::Clear);
        let _ = cr.paint();
        cr.set_operator(cairo::Operator::Over);

        let margin = 0.0;
        let box_s = s;
        let radius = s * 0.20;

        // Clean GNOME Blue -> Indigo squircle badge
        draw_rounded_rect(&cr, margin, margin, box_s, box_s, radius);
        let bg = cairo::LinearGradient::new(0.0, 0.0, s, s);
        bg.add_color_stop_rgba(0.0, 0.208, 0.518, 0.894, 1.0); // #3584e4
        bg.add_color_stop_rgba(1.0, 0.357, 0.275, 0.839, 1.0); // #5b46d6
        let _ = cr.set_source(&bg);
        let _ = cr.fill();

        // Bold white viewfinder corner brackets
        let p0 = s * 0.19;
        let p1 = s * 0.81;
        let arm = s * 0.16;
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.98);
        cr.set_line_width((s * 0.09).max(1.9));
        cr.set_line_cap(cairo::LineCap::Round);
        cr.set_line_join(cairo::LineJoin::Round);
        for (cx, cy, dx, dy) in [
            (p0, p0, arm, arm),
            (p1, p0, -arm, arm),
            (p0, p1, arm, -arm),
            (p1, p1, -arm, -arm),
        ] {
            cr.move_to(cx + dx, cy);
            cr.line_to(cx, cy);
            cr.line_to(cx, cy + dy);
            let _ = cr.stroke();
        }

        // Clean centered aperture ring + subtle warm accent dot
        let cx = s * 0.5;
        let cy = s * 0.5;
        cr.arc(cx, cy, s * 0.155, 0.0, std::f64::consts::TAU);
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.98);
        cr.set_line_width((s * 0.078).max(1.7));
        let _ = cr.stroke();

        cr.arc(cx, cy, (s * 0.058).max(1.4), 0.0, std::f64::consts::TAU);
        cr.set_source_rgba(1.0, 0.541, 0.169, 1.0); // #ff8a2b
        let _ = cr.fill();
    }

    surface.flush();
    let stride = surface.stride() as usize;
    let raw = surface.data().expect("cairo surface data");
    let mut data = vec![0u8; (size * size * 4) as usize];

    for y in 0..size as usize {
        let src_row = y * stride;
        let dst_row = y * size as usize * 4;
        for x in 0..size as usize {
            let si = src_row + x * 4;
            let di = dst_row + x * 4;
            // Cairo ARgb32 on little-endian: [B, G, R, A] premultiplied
            let b = raw[si];
            let g = raw[si + 1];
            let r = raw[si + 2];
            let a = raw[si + 3];
            let (ur, ug, ub) = if a == 0 {
                (0, 0, 0)
            } else if a == 255 {
                (r, g, b)
            } else {
                let af = a as f64 / 255.0;
                (
                    (r as f64 / af).round().clamp(0.0, 255.0) as u8,
                    (g as f64 / af).round().clamp(0.0, 255.0) as u8,
                    (b as f64 / af).round().clamp(0.0, 255.0) as u8,
                )
            };
            // StatusNotifierItem expects network byte order: [A, R, G, B]
            data[di] = a;
            data[di + 1] = ur;
            data[di + 2] = ug;
            data[di + 3] = ub;
        }
    }

    Icon {
        width: size,
        height: size,
        data,
    }
}

struct TrayMenuIcons {
    capture: Vec<u8>,
    history: Vec<u8>,
    window: Vec<u8>,
    settings: Vec<u8>,
    quit: Vec<u8>,
}

fn tray_menu_icons() -> &'static TrayMenuIcons {
    static CACHE: OnceLock<TrayMenuIcons> = OnceLock::new();
    CACHE.get_or_init(|| TrayMenuIcons {
        capture: render_png_menu_icon(draw_menu_capture),
        history: render_png_menu_icon(draw_menu_history),
        window: render_png_menu_icon(draw_menu_window),
        settings: render_png_menu_icon(draw_menu_settings),
        quit: render_png_menu_icon(draw_menu_quit),
    })
}

fn render_png_menu_icon(draw_fn: fn(&cairo::Context)) -> Vec<u8> {
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, 24, 24).expect("cairo surface");
    {
        let cr = cairo::Context::new(&surface).expect("cairo context");
        cr.set_operator(cairo::Operator::Clear);
        let _ = cr.paint();
        cr.set_operator(cairo::Operator::Over);
        draw_fn(&cr);
    }
    let Ok(rgba) = cairo_surface_to_rgba(&mut surface) else {
        return Vec::new();
    };
    let mut buf = Cursor::new(Vec::new());
    if rgba.write_to(&mut buf, ImageFormat::Png).is_ok() {
        buf.into_inner()
    } else {
        Vec::new()
    }
}

fn draw_menu_capture(cr: &cairo::Context) {
    draw_rounded_rect(cr, 2.5, 2.5, 19.0, 19.0, 5.5);
    cr.set_source_rgba(0.431, 0.659, 0.996, 0.18);
    let _ = cr.fill();

    cr.set_source_rgba(0.902, 0.922, 0.949, 0.94);
    cr.set_line_width(1.85);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    for (cx, cy, dx, dy) in [
        (5.0, 5.0, 3.5, 3.5),
        (19.0, 5.0, -3.5, 3.5),
        (5.0, 19.0, 3.5, -3.5),
        (19.0, 19.0, -3.5, -3.5),
    ] {
        cr.move_to(cx + dx, cy);
        cr.line_to(cx, cy);
        cr.line_to(cx, cy + dy);
        let _ = cr.stroke();
    }

    cr.set_source_rgba(0.431, 0.659, 0.996, 0.96);
    cr.set_line_width(1.9);
    cr.move_to(12.0, 8.5);
    cr.line_to(12.0, 15.5);
    let _ = cr.stroke();
    cr.move_to(8.5, 12.0);
    cr.line_to(15.5, 12.0);
    let _ = cr.stroke();
}

fn draw_menu_history(cr: &cairo::Context) {
    draw_rounded_rect(cr, 2.5, 6.5, 4.6, 11.0, 1.3);
    cr.set_source_rgba(0.431, 0.659, 0.996, 0.30);
    let _ = cr.fill();

    draw_rounded_rect(cr, 16.9, 6.5, 4.6, 11.0, 1.3);
    cr.set_source_rgba(0.655, 0.545, 0.980, 0.30);
    let _ = cr.fill();

    draw_rounded_rect(cr, 7.0, 4.2, 10.0, 15.6, 2.0);
    cr.set_source_rgba(0.431, 0.659, 0.996, 0.24);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(0.902, 0.922, 0.949, 0.94);
    cr.set_line_width(1.55);
    let _ = cr.stroke();

    cr.arc(12.0, 12.0, 1.7, 0.0, std::f64::consts::TAU);
    cr.set_source_rgba(0.431, 0.659, 0.996, 0.96);
    let _ = cr.fill();
}

fn draw_menu_window(cr: &cairo::Context) {
    draw_rounded_rect(cr, 3.2, 4.2, 17.6, 15.6, 2.2);
    cr.set_source_rgba(0.431, 0.659, 0.996, 0.16);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(0.902, 0.922, 0.949, 0.92);
    cr.set_line_width(1.65);
    let _ = cr.stroke();

    cr.move_to(3.5, 8.6);
    cr.line_to(20.5, 8.6);
    cr.set_source_rgba(0.431, 0.659, 0.996, 0.90);
    cr.set_line_width(1.5);
    let _ = cr.stroke();

    cr.arc(6.2, 6.4, 0.9, 0.0, std::f64::consts::TAU);
    cr.set_source_rgba(0.902, 0.922, 0.949, 0.92);
    let _ = cr.fill();

    cr.arc(9.0, 6.4, 0.9, 0.0, std::f64::consts::TAU);
    cr.set_source_rgba(0.431, 0.659, 0.996, 0.92);
    let _ = cr.fill();
}

fn draw_menu_settings(cr: &cairo::Context) {
    let cx = 12.0;
    let cy = 12.0;

    for i in 0..8 {
        let ang = (i as f64) * std::f64::consts::FRAC_PI_4;
        cr.save().ok();
        cr.translate(cx, cy);
        cr.rotate(ang);
        draw_rounded_rect(cr, -1.5, -9.2, 3.0, 3.5, 0.8);
        cr.set_source_rgba(0.902, 0.922, 0.949, 0.92);
        let _ = cr.fill();
        cr.restore().ok();
    }

    cr.arc(cx, cy, 6.4, 0.0, std::f64::consts::TAU);
    cr.set_source_rgba(0.431, 0.659, 0.996, 0.24);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(0.902, 0.922, 0.949, 0.92);
    cr.set_line_width(1.5);
    let _ = cr.stroke();

    cr.arc(cx, cy, 2.7, 0.0, std::f64::consts::TAU);
    cr.set_source_rgba(0.137, 0.153, 0.196, 0.95);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(0.431, 0.659, 0.996, 0.95);
    cr.set_line_width(1.55);
    let _ = cr.stroke();
}

fn draw_menu_quit(cr: &cairo::Context) {
    cr.arc(12.0, 12.0, 8.4, 0.0, std::f64::consts::TAU);
    cr.set_source_rgba(0.878, 0.424, 0.459, 0.24);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(0.878, 0.424, 0.459, 0.90);
    cr.set_line_width(1.65);
    let _ = cr.stroke();

    cr.set_source_rgba(0.902, 0.922, 0.949, 0.96);
    cr.set_line_width(1.85);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.move_to(9.0, 9.0);
    cr.line_to(15.0, 15.0);
    let _ = cr.stroke();
    cr.move_to(15.0, 9.0);
    cr.line_to(9.0, 15.0);
    let _ = cr.stroke();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_pixmaps_have_expected_sizes_and_colored_pixels() {
        let icons = tray_icon_pixmaps();
        assert_eq!(icons.len(), 4);
        for (icon, expected) in icons.iter().zip([22, 32, 48, 64]) {
            assert_eq!(icon.width, expected);
            assert_eq!(icon.height, expected);
            assert_eq!(icon.data.len(), (expected * expected * 4) as usize);
            // Outer corner stays transparent for rounded squircle silhouette
            assert_eq!(icon.data[0], 0);
            // Center lens area is opaque
            let mid = ((expected as usize / 2) * expected as usize + expected as usize / 2) * 4;
            assert_eq!(icon.data[mid], 255);
            // Upper squircle region is opaque vibrant blue-violet
            let badge =
                (expected as usize / 6) * expected as usize * 4 + (expected as usize / 2) * 4;
            assert_eq!(icon.data[badge], 255);
            assert!(icon.data[badge + 3] > 150);
        }
    }

    #[test]
    fn tray_menu_icons_are_valid_pngs() {
        let icons = tray_menu_icons();
        for png in [
            &icons.capture,
            &icons.history,
            &icons.window,
            &icons.settings,
            &icons.quit,
        ] {
            assert!(png.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]));
            let decoded = image::load_from_memory_with_format(png, ImageFormat::Png).unwrap();
            assert_eq!(decoded.width(), 24);
            assert_eq!(decoded.height(), 24);
        }
    }
}
