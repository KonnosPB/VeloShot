//! interactive fullscreen region selector rendered on GPU via GTK4 Snapshot/GSK,
//! paired with an animated frosted-glass capture mini-bar that auto-fades during drag,
//! springs smoothly between top/bottom, and is guaranteed never to appear in the screenshot.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::{gdk, glib, graphene, gsk, pango, prelude::*, subclass::prelude::*};
use image::RgbaImage;

use crate::capture::crop_region;
use crate::i18n::tr;
use crate::ui::{
    AppState, handle_captured_region, icons, trigger_delayed_capture, trigger_interactive_capture,
};

#[derive(Clone, Copy, Default)]
pub(super) struct DragState {
    active: bool,
    start_x: f64,
    start_y: f64,
    cur_x: f64,
    cur_y: f64,
}

impl DragState {
    fn normalized_rect(self) -> (f64, f64, f64, f64) {
        let x0 = self.start_x.min(self.cur_x);
        let y0 = self.start_y.min(self.cur_y);
        let w = (self.cur_x - self.start_x).abs();
        let h = (self.cur_y - self.start_y).abs();
        (x0, y0, w, h)
    }
}

#[derive(Clone)]
struct MonitorSlice {
    tooltip: String,
    norm_x: f64,
    norm_y: f64,
    norm_w: f64,
    norm_h: f64,
}

mod imp {
    use super::*;

    pub struct OverlayCanvas {
        pub(super) texture: RefCell<Option<gdk::MemoryTexture>>,
        pub(super) img_size: Cell<(f64, f64)>,
        pub(super) drag: Cell<DragState>,
        pub(super) mouse: Cell<(f64, f64)>,
        pub(super) hovering_toolbar: Cell<bool>,
        pub(super) highlight_norm: Cell<Option<(f64, f64, f64, f64)>>,
        pub(super) last_highlight_norm: Cell<Option<(f64, f64, f64, f64)>>,
        pub(super) highlight_alpha: Cell<f32>,
        pub(super) loupe_alpha: Cell<f32>,
        pub(super) last_tick_us: Cell<Option<i64>>,
        pub(super) hud_font: pango::FontDescription,
    }

    impl Default for OverlayCanvas {
        fn default() -> Self {
            Self {
                texture: RefCell::new(None),
                img_size: Cell::new((1.0, 1.0)),
                drag: Cell::new(DragState::default()),
                mouse: Cell::new((0.0, 0.0)),
                hovering_toolbar: Cell::new(false),
                highlight_norm: Cell::new(None),
                last_highlight_norm: Cell::new(None),
                highlight_alpha: Cell::new(0.0),
                loupe_alpha: Cell::new(1.0),
                last_tick_us: Cell::new(None),
                hud_font: pango::FontDescription::from_string("Sans Bold 10"),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for OverlayCanvas {
        const NAME: &'static str = "VeloShotOverlayCanvas";
        type Type = super::OverlayCanvas;
        type ParentType = gtk4::Widget;
    }

    impl ObjectImpl for OverlayCanvas {}

    impl WidgetImpl for OverlayCanvas {
        fn snapshot(&self, snapshot: &gtk4::Snapshot) {
            let widget = self.obj();
            let w = widget.width() as f32;
            let h = widget.height() as f32;
            if w <= 1.0 || h <= 1.0 {
                return;
            }

            let tex_guard = self.texture.borrow();
            let Some(texture) = tex_guard.as_ref() else {
                return;
            };
            let (img_w, img_h) = self.img_size.get();
            let drag = self.drag.get();
            let (mx, my) = self.mouse.get();

            // 1. Frozen desktop background (single GPU texture blit)
            snapshot.append_texture(texture, &graphene::Rect::new(0.0, 0.0, w, h));

            // 2. Dim mask outside selection (with smooth monitor highlight fade)
            let (rx, ry, rw, rh) = drag.normalized_rect();
            let hl_alpha = self.highlight_alpha.get();
            if drag.active && rw >= 2.0 && rh >= 2.0 {
                let dim_color = gdk::RGBA::new(0.0, 0.0, 0.0, 0.44);
                let x0 = (rx as f32).clamp(0.0, w);
                let y0 = (ry as f32).clamp(0.0, h);
                let x1 = ((rx + rw) as f32).clamp(x0, w);
                let y1 = ((ry + rh) as f32).clamp(y0, h);
                append_dim_around_rect(snapshot, w, h, x0, y0, x1, y1, &dim_color);

                let accent = gdk::RGBA::new(0.24, 0.62, 0.98, 0.98);
                let sel_rect = gsk::RoundedRect::from_rect(
                    graphene::Rect::new(x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0)),
                    0.0,
                );
                snapshot.append_border(&sel_rect, &[2.0; 4], &[accent; 4]);
            } else if hl_alpha > 0.01
                && let Some((nx, ny, nw, nh)) = self.last_highlight_norm.get()
            {
                let base_dim = 0.28 * (1.0 - hl_alpha) + 0.46 * hl_alpha;
                let dim_color = gdk::RGBA::new(0.0, 0.0, 0.0, base_dim);
                let hx = (nx as f32 * w).clamp(0.0, w);
                let hy = (ny as f32 * h).clamp(0.0, h);
                let hw = (nw as f32 * w).clamp(1.0, w - hx);
                let hh = (nh as f32 * h).clamp(1.0, h - hy);
                append_dim_around_rect(snapshot, w, h, hx, hy, hx + hw, hy + hh, &dim_color);

                let accent = gdk::RGBA::new(0.24, 0.62, 0.98, 0.98 * hl_alpha);
                let mon_rect =
                    gsk::RoundedRect::from_rect(graphene::Rect::new(hx, hy, hw, hh), 0.0);
                snapshot.append_border(&mon_rect, &[4.0; 4], &[accent; 4]);
                return;
            } else {
                snapshot.append_color(
                    &gdk::RGBA::new(0.0, 0.0, 0.0, 0.28),
                    &graphene::Rect::new(0.0, 0.0, w, h),
                );
            }

            // Smooth GPU opacity fade for crosshair + 6x loupe when entering/leaving mini-bar
            let eff_alpha = if drag.active {
                1.0
            } else {
                self.loupe_alpha.get()
            };
            if eff_alpha <= 0.01 {
                return;
            }
            snapshot.push_opacity(eff_alpha as f64);

            // 3. Fullscreen crosshair lines
            let cross_color = gdk::RGBA::new(1.0, 1.0, 1.0, 0.72);
            let cx_line = (mx as f32).floor().clamp(0.0, w - 1.0);
            let cy_line = (my as f32).floor().clamp(0.0, h - 1.0);
            snapshot.append_color(&cross_color, &graphene::Rect::new(cx_line, 0.0, 1.0, h));
            snapshot.append_color(&cross_color, &graphene::Rect::new(0.0, cy_line, w, 1.0));

            // 4. interactive 6x GPU live loupe & pixel HUD
            let scale_x = img_w / w as f64;
            let scale_y = img_h / h as f64;
            let img_cx = (mx * scale_x).clamp(0.0, img_w - 1.0);
            let img_cy = (my * scale_y).clamp(0.0, img_h - 1.0);

            let loupe_size = 144.0_f32;
            let zoom = 6.0_f32;
            let hud_h = 28.0_f32;
            let margin = 22.0_f32;

            let lx = if mx as f32 + margin + loupe_size + 12.0 > w {
                (mx as f32 - margin - loupe_size).max(8.0)
            } else {
                mx as f32 + margin
            };
            let ly = if my as f32 + margin + loupe_size + hud_h + 14.0 > h {
                (my as f32 - margin - loupe_size - hud_h - 6.0).max(8.0)
            } else {
                my as f32 + margin
            };

            let loupe_rect = graphene::Rect::new(lx, ly, loupe_size, loupe_size);
            let loupe_rounded = gsk::RoundedRect::from_rect(loupe_rect, 12.0);
            snapshot.push_rounded_clip(&loupe_rounded);
            snapshot.append_color(&gdk::RGBA::new(0.08, 0.08, 0.10, 1.0), &loupe_rect);

            let half = loupe_size * 0.5;
            snapshot.save();
            snapshot.translate(&graphene::Point::new(lx + half, ly + half));
            snapshot.scale(zoom, zoom);
            snapshot.append_scaled_texture(
                texture,
                gsk::ScalingFilter::Nearest,
                &graphene::Rect::new(
                    (-img_cx.floor() - 0.5) as f32,
                    (-img_cy.floor() - 0.5) as f32,
                    img_w as f32,
                    img_h as f32,
                ),
            );
            snapshot.restore();

            let grid_color = gdk::RGBA::new(1.0, 1.0, 1.0, 0.14);
            let mut g = zoom;
            while g < loupe_size {
                snapshot.append_color(
                    &grid_color,
                    &graphene::Rect::new(lx + g, ly, 1.0, loupe_size),
                );
                snapshot.append_color(
                    &grid_color,
                    &graphene::Rect::new(lx, ly + g, loupe_size, 1.0),
                );
                g += zoom;
            }

            let red = gdk::RGBA::new(0.95, 0.25, 0.25, 0.96);
            let center_box = gsk::RoundedRect::from_rect(
                graphene::Rect::new(lx + half - zoom * 0.5, ly + half - zoom * 0.5, zoom, zoom),
                0.0,
            );
            snapshot.append_border(&center_box, &[1.5; 4], &[red; 4]);
            snapshot.pop();

            let white_border = gdk::RGBA::new(1.0, 1.0, 1.0, 0.85);
            snapshot.append_border(&loupe_rounded, &[2.0; 4], &[white_border; 4]);

            // 5. Coordinate / selection size HUD pill
            let label = if drag.active && rw >= 1.0 && rh >= 1.0 {
                format!(
                    "{} × {} px  ({:.0}, {:.0})",
                    (rw * scale_x).round() as u32,
                    (rh * scale_y).round() as u32,
                    img_cx.round(),
                    img_cy.round()
                )
            } else {
                format!(
                    "X: {:.0}  Y: {:.0}  ({})",
                    img_cx.round(),
                    img_cy.round(),
                    tr("Ziehen | Enter = Vollbild", "Drag | Enter = Fullscreen")
                )
            };

            let layout = widget.create_pango_layout(Some(&label));
            layout.set_font_description(Some(&self.hud_font));
            let (tw, th) = layout.pixel_size();
            let pill_w = (tw as f32 + 20.0).max(loupe_size);
            let pill_x = lx.min(w - pill_w - 8.0).max(8.0);
            let pill_y = ly + loupe_size + 6.0;

            let pill_rect = graphene::Rect::new(pill_x, pill_y, pill_w, hud_h);
            let pill_rounded = gsk::RoundedRect::from_rect(pill_rect, 8.0);
            snapshot.push_rounded_clip(&pill_rounded);
            snapshot.append_color(&gdk::RGBA::new(0.08, 0.09, 0.12, 0.90), &pill_rect);
            snapshot.pop();
            snapshot.append_border(
                &pill_rounded,
                &[1.0; 4],
                &[gdk::RGBA::new(1.0, 1.0, 1.0, 0.22); 4],
            );

            snapshot.save();
            snapshot.translate(&graphene::Point::new(
                pill_x + 10.0,
                pill_y + (hud_h - th as f32) * 0.5,
            ));
            snapshot.append_layout(&layout, &gdk::RGBA::new(0.96, 0.97, 0.99, 1.0));
            snapshot.restore();

            snapshot.pop();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn append_dim_around_rect(
    snapshot: &gtk4::Snapshot,
    w: f32,
    h: f32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    dim_color: &gdk::RGBA,
) {
    if y0 > 0.0 {
        snapshot.append_color(dim_color, &graphene::Rect::new(0.0, 0.0, w, y0));
    }
    if h > y1 {
        snapshot.append_color(dim_color, &graphene::Rect::new(0.0, y1, w, h - y1));
    }
    if x0 > 0.0 && y1 > y0 {
        snapshot.append_color(dim_color, &graphene::Rect::new(0.0, y0, x0, y1 - y0));
    }
    if w > x1 && y1 > y0 {
        snapshot.append_color(dim_color, &graphene::Rect::new(x1, y0, w - x1, y1 - y0));
    }
}

glib::wrapper! {
    pub struct OverlayCanvas(ObjectSubclass<imp::OverlayCanvas>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl OverlayCanvas {
    fn new(full_img: &RgbaImage) -> Self {
        let obj: Self = glib::Object::new();
        let w = full_img.width() as i32;
        let h = full_img.height() as i32;
        let bytes = glib::Bytes::from(full_img.as_raw().as_slice());
        let texture = gdk::MemoryTexture::new(
            w,
            h,
            gdk::MemoryFormat::R8g8b8a8,
            &bytes,
            (full_img.width() * 4) as usize,
        );
        let imp = obj.imp();
        *imp.texture.borrow_mut() = Some(texture);
        imp.img_size.set((w as f64, h as f64));
        imp.mouse.set((w as f64 * 0.5, h as f64 * 0.5));
        obj.set_hexpand(true);
        obj.set_vexpand(true);
        obj.set_can_focus(true);
        obj.set_focusable(true);
        obj
    }
}

/// Discovers connected monitors from GDK and normalizes their coordinates against the desktop union.
fn discover_monitor_slices() -> Vec<MonitorSlice> {
    let Some(display) = gdk::Display::default() else {
        return Vec::new();
    };
    let model = display.monitors();
    let count = model.n_items();
    let mut rects = Vec::new();

    for idx in 0..count {
        if let Some(mon) = model.item(idx).and_downcast::<gdk::Monitor>() {
            let geo = mon.geometry();
            let desc = mon
                .description()
                .or_else(|| mon.connector())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{} {}", tr("Bildschirm", "Display"), idx + 1));
            rects.push((idx, geo, desc));
        }
    }
    if rects.is_empty() {
        return Vec::new();
    }

    let min_x = rects.iter().map(|(_, g, _)| g.x()).min().unwrap_or(0);
    let min_y = rects.iter().map(|(_, g, _)| g.y()).min().unwrap_or(0);
    let max_x = rects
        .iter()
        .map(|(_, g, _)| g.x() + g.width())
        .max()
        .unwrap_or(1);
    let max_y = rects
        .iter()
        .map(|(_, g, _)| g.y() + g.height())
        .max()
        .unwrap_or(1);
    let union_w = (max_x - min_x).max(1) as f64;
    let union_h = (max_y - min_y).max(1) as f64;

    rects
        .into_iter()
        .map(|(idx, g, desc)| MonitorSlice {
            tooltip: format!(
                "Monitor {}: {desc} ({}×{}) — {}",
                idx + 1,
                g.width(),
                g.height(),
                tr("Klick für Sofort-Aufnahme", "Click for instant capture")
            ),
            norm_x: (g.x() - min_x) as f64 / union_w,
            norm_y: (g.y() - min_y) as f64 / union_h,
            norm_w: g.width() as f64 / union_w,
            norm_h: g.height() as f64 / union_h,
        })
        .collect()
}

/// Opens the fullscreen interactive selection overlay on top of the frozen screenshot.
pub fn open_region_overlay(state: AppState, full_img: RgbaImage) {
    let canvas = OverlayCanvas::new(&full_img);
    let img_rc = Rc::new(full_img);
    let completed = Rc::new(Cell::new(false));

    let window = gtk4::Window::builder()
        .application(&state.app)
        .title(tr(
            "Bereich auswählen — VeloShot",
            "Select Region — VeloShot",
        ))
        .decorated(false)
        .fullscreened(true)
        .build();

    if let Some(cursor) = gdk::Cursor::from_name("crosshair", None) {
        window.set_cursor(Some(&cursor));
    }

    let overlay = gtk4::Overlay::new();
    overlay.set_child(Some(&canvas));

    // --- Floating frosted-glass mini-bar (GPU-composited slide, never in screenshot) ---
    let dynamic_css = gtk4::CssProvider::new();
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &dynamic_css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
    }
    let css_cleanup = dynamic_css.clone();
    window.connect_close_request(move |_| {
        if let Some(display) = gdk::Display::default() {
            gtk4::style_context_remove_provider_for_display(&display, &css_cleanup);
        }
        glib::Propagation::Proceed
    });

    let toolbar = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Start)
        .margin_top(28)
        .css_classes(["capture-toolbar", "pos-bottom", "ghost-hidden"])
        .build();

    let sel_badge = gtk4::Button::builder()
        .child(&icons::icon_image("jas-capture-region", 28))
        .css_classes(["suggested-action"])
        .tooltip_text(tr(
            "Bereich auswählen — Ziehen mit 6×-Live-Lupe (Leiste blendet beim Ziehen aus)",
            "Select region — drag with 6× live loupe (bar hides while dragging)",
        ))
        .build();
    let canvas_focus = canvas.clone();
    sel_badge.connect_clicked(move |_| {
        canvas_focus.grab_focus();
    });
    toolbar.append(&sel_badge);

    // Monitor buttons (each smoothly highlights its exact monitor on hover!)
    let monitors = discover_monitor_slices();
    if monitors.is_empty() {
        let full_btn = gtk4::Button::builder()
            .child(&icons::icon_image("jas-capture-monitor", 28))
            .tooltip_text(tr(
                "Gesamten Monitor aufnehmen (Enter)",
                "Capture entire monitor (Enter)",
            ))
            .build();
        let win_full = window.clone();
        let img_full = Rc::clone(&img_rc);
        let state_full = state.clone();
        let done_full = Rc::clone(&completed);
        full_btn.connect_clicked(move |_| {
            if !done_full.get() {
                done_full.set(true);
                let full = (*img_full).clone();
                win_full.close();
                handle_captured_region(state_full.clone(), full);
            }
        });
        toolbar.append(&full_btn);
    } else {
        let multi_monitor = monitors.len() > 1;
        for (idx, mon) in monitors.iter().enumerate() {
            let btn = if multi_monitor {
                let b = gtk4::Button::builder().tooltip_text(&mon.tooltip).build();
                let inner = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
                inner.append(&icons::icon_image("jas-capture-monitor", 28));
                inner.append(&gtk4::Label::new(Some(&format!("{}", idx + 1))));
                b.set_child(Some(&inner));
                b
            } else {
                gtk4::Button::builder()
                    .child(&icons::icon_image("jas-capture-monitor", 28))
                    .tooltip_text(&mon.tooltip)
                    .build()
            };

            let mon_hover = gtk4::EventControllerMotion::new();
            let canvas_enter = canvas.clone();
            let norm = (mon.norm_x, mon.norm_y, mon.norm_w, mon.norm_h);
            mon_hover.connect_enter(move |_, _, _| {
                let imp = canvas_enter.imp();
                imp.highlight_norm.set(Some(norm));
                imp.last_highlight_norm.set(Some(norm));
                canvas_enter.queue_draw();
            });
            let canvas_leave = canvas.clone();
            mon_hover.connect_leave(move |_| {
                canvas_leave.imp().highlight_norm.set(None);
                canvas_leave.queue_draw();
            });
            btn.add_controller(mon_hover);

            let win_m = window.clone();
            let img_m = Rc::clone(&img_rc);
            let state_m = state.clone();
            let done_m = Rc::clone(&completed);
            btn.connect_clicked(move |_| {
                if done_m.get() {
                    return;
                }
                let iw = img_m.width() as f64;
                let ih = img_m.height() as f64;
                let px = (norm.0 * iw).round().max(0.0) as u32;
                let py = (norm.1 * ih).round().max(0.0) as u32;
                let pw = (norm.2 * iw).round().max(1.0) as u32;
                let ph = (norm.3 * ih).round().max(1.0) as u32;
                if let Ok(cropped) = crop_region(&img_m, px, py, pw, ph) {
                    done_m.set(true);
                    win_m.close();
                    handle_captured_region(state_m.clone(), cropped);
                }
            });
            toolbar.append(&btn);
        }

        if monitors.len() > 1 {
            let all_btn = gtk4::Button::builder()
                .child(&icons::icon_image("jas-capture-all-monitors", 28))
                .tooltip_text(tr(
                    "Alle Monitore gemeinsam aufnehmen (Enter)",
                    "Capture all monitors together (Enter)",
                ))
                .build();
            let all_hover = gtk4::EventControllerMotion::new();
            let canvas_enter = canvas.clone();
            all_hover.connect_enter(move |_, _, _| {
                let imp = canvas_enter.imp();
                let full = (0.0, 0.0, 1.0, 1.0);
                imp.highlight_norm.set(Some(full));
                imp.last_highlight_norm.set(Some(full));
                canvas_enter.queue_draw();
            });
            let canvas_leave = canvas.clone();
            all_hover.connect_leave(move |_| {
                canvas_leave.imp().highlight_norm.set(None);
                canvas_leave.queue_draw();
            });
            all_btn.add_controller(all_hover);

            let win_all = window.clone();
            let img_all = Rc::clone(&img_rc);
            let state_all = state.clone();
            let done_all = Rc::clone(&completed);
            all_btn.connect_clicked(move |_| {
                if !done_all.get() {
                    done_all.set(true);
                    let full = (*img_all).clone();
                    win_all.close();
                    handle_captured_region(state_all.clone(), full);
                }
            });
            toolbar.append(&all_btn);
        }
    }

    // Window mode via xdg-desktop-portal interactive picker
    let window_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-capture-window", 28))
        .tooltip_text(tr(
            "Einzelnes Fenster über den GNOME-Dialog auswählen",
            "Choose single window via GNOME dialog",
        ))
        .build();
    let win_w = window.clone();
    let state_w = state.clone();
    let done_w = Rc::clone(&completed);
    window_btn.connect_clicked(move |_| {
        if !done_w.get() {
            done_w.set(true);
            win_w.close();
            trigger_interactive_capture(state_w.clone());
        }
    });
    toolbar.append(&window_btn);

    toolbar.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));

    // Delayed capture buttons (3s / 5s)
    for secs in [3_u64, 5_u64] {
        let timer_btn = gtk4::Button::builder()
            .tooltip_text(
                tr(
                    "In {secs} Sekunden neu aufnehmen (z. B. für geöffnete Menüs)",
                    "Re-capture in {secs} seconds (e.g. for open menus)",
                )
                .replace("{secs}", &secs.to_string()),
            )
            .build();
        let inner = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        inner.append(&icons::icon_image("jas-timer", 28));
        inner.append(&gtk4::Label::new(Some(&format!("{secs}s"))));
        timer_btn.set_child(Some(&inner));
        let win_t = window.clone();
        let state_t = state.clone();
        let done_t = Rc::clone(&completed);
        timer_btn.connect_clicked(move |_| {
            if !done_t.get() {
                done_t.set(true);
                win_t.close();
                trigger_delayed_capture(state_t.clone(), secs);
            }
        });
        toolbar.append(&timer_btn);
    }

    toolbar.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));

    // GPU-composited slide between bottom and top via CSS transform
    let at_bottom = Rc::new(Cell::new(true));
    let flip_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-flip-vertical", 28))
        .tooltip_text(tr(
            "Leiste nach oben / unten verschieben",
            "Move toolbar to top / bottom",
        ))
        .build();
    let toolbar_flip = toolbar.clone();
    let at_bottom_flip = Rc::clone(&at_bottom);
    flip_btn.connect_clicked(move |_| {
        let next_bottom = !at_bottom_flip.get();
        at_bottom_flip.set(next_bottom);
        if next_bottom {
            toolbar_flip.remove_css_class("pos-top");
            toolbar_flip.add_css_class("pos-bottom");
        } else {
            toolbar_flip.remove_css_class("pos-bottom");
            toolbar_flip.add_css_class("pos-top");
        }
    });
    toolbar.append(&flip_btn);

    let cancel_btn = gtk4::Button::builder()
        .child(&icons::icon_image("window-close-symbolic", 24))
        .tooltip_text(tr("Abbrechen (Esc)", "Cancel (Esc)"))
        .css_classes(["flat"])
        .build();
    let win_cancel = window.clone();
    cancel_btn.connect_clicked(move |_| win_cancel.close());
    toolbar.append(&cancel_btn);

    // Hover awareness on floating toolbar: smoothly fade out crosshair & loupe
    let tb_motion = gtk4::EventControllerMotion::new();
    let canvas_tb_enter = canvas.clone();
    let win_tb_enter = window.clone();
    tb_motion.connect_enter(move |_, _, _| {
        canvas_tb_enter.imp().hovering_toolbar.set(true);
        win_tb_enter.set_cursor(None);
        canvas_tb_enter.queue_draw();
    });
    let canvas_tb_leave = canvas.clone();
    let win_tb_leave = window.clone();
    tb_motion.connect_leave(move |_| {
        canvas_tb_leave.imp().hovering_toolbar.set(false);
        if let Some(cursor) = gdk::Cursor::from_name("crosshair", None) {
            win_tb_leave.set_cursor(Some(&cursor));
        }
        canvas_tb_leave.queue_draw();
    });
    toolbar.add_controller(tb_motion);

    overlay.add_overlay(&toolbar);

    // 60+ FPS tick callback: drives GPU CSS position setup + smooth loupe/highlight fades
    let entered = Rc::new(Cell::new(false));
    let toolbar_tick = toolbar.clone();
    let entered_tick = Rc::clone(&entered);
    let css_tick = dynamic_css.clone();
    canvas.add_tick_callback(move |area, clock| {
        let imp = area.imp();
        let now = clock.frame_time();
        let dt = match imp.last_tick_us.get() {
            Some(prev) => ((now - prev) as f32 / 1_000_000.0).clamp(0.001, 0.05),
            None => 1.0 / 60.0,
        };
        imp.last_tick_us.set(Some(now));

        // First configured frame: install exact GPU translateY for bottom dock & reveal smoothly
        if !entered_tick.get() && area.height() > 120 && toolbar_tick.height() > 24 {
            entered_tick.set(true);
            let delta = (area.height() - toolbar_tick.height() - 56).max(0);
            css_tick.load_from_string(&format!(
                ".capture-toolbar.pos-bottom {{ transform: translateY({delta}px); }}\n\
                 .capture-toolbar.pos-bottom.ghost-hidden {{ opacity: 0.0; transform: translateY({}px) scale(0.96); }}",
                delta + 18
            ));
            toolbar_tick.remove_css_class("ghost-hidden");
        }

        let mut needs_redraw = false;

        let target_loupe = if imp.hovering_toolbar.get() { 0.0 } else { 1.0 };
        let cur_loupe = imp.loupe_alpha.get();
        if (target_loupe - cur_loupe).abs() > 0.005 {
            let next = cur_loupe + (target_loupe - cur_loupe) * (1.0 - (-15.0 * dt).exp());
            imp.loupe_alpha.set(if (target_loupe - next).abs() <= 0.005 {
                target_loupe
            } else {
                next
            });
            needs_redraw = true;
        }

        let target_hl = if imp.highlight_norm.get().is_some() {
            1.0
        } else {
            0.0
        };
        let cur_hl = imp.highlight_alpha.get();
        if (target_hl - cur_hl).abs() > 0.005 {
            let next = cur_hl + (target_hl - cur_hl) * (1.0 - (-14.0 * dt).exp());
            let final_hl = if (target_hl - next).abs() <= 0.005 {
                target_hl
            } else {
                next
            };
            imp.highlight_alpha.set(final_hl);
            if final_hl <= 0.005 && imp.highlight_norm.get().is_none() {
                imp.last_highlight_norm.set(None);
            }
            needs_redraw = true;
        }

        if needs_redraw {
            area.queue_draw();
        }
        glib::ControlFlow::Continue
    });

    // Motion tracking for live crosshair and 6x GPU loupe
    let motion = gtk4::EventControllerMotion::new();
    let canvas_move = canvas.clone();
    motion.connect_motion(move |_, x, y| {
        let imp = canvas_move.imp();
        imp.mouse.set((x, y));
        let mut d = imp.drag.get();
        if d.active {
            d.cur_x = x;
            d.cur_y = y;
            imp.drag.set(d);
        }
        canvas_move.queue_draw();
    });
    canvas.add_controller(motion);

    // Left-button drag to select region (auto-fades floating toolbar out of the way!)
    let drag_ctrl = gtk4::GestureDrag::new();
    drag_ctrl.set_button(1);
    let canvas_start = canvas.clone();
    let toolbar_start = toolbar.clone();
    drag_ctrl.connect_drag_begin(move |_, x, y| {
        toolbar_start.add_css_class("ghost-hidden");
        toolbar_start.set_can_target(false);
        let imp = canvas_start.imp();
        imp.mouse.set((x, y));
        imp.drag.set(DragState {
            active: true,
            start_x: x,
            start_y: y,
            cur_x: x,
            cur_y: y,
        });
        canvas_start.queue_draw();
    });

    let canvas_update = canvas.clone();
    drag_ctrl.connect_drag_update(move |_, offset_x, offset_y| {
        let imp = canvas_update.imp();
        let mut d = imp.drag.get();
        if d.active {
            d.cur_x = d.start_x + offset_x;
            d.cur_y = d.start_y + offset_y;
            imp.mouse.set((d.cur_x, d.cur_y));
            imp.drag.set(d);
            canvas_update.queue_draw();
        }
    });

    let canvas_end = canvas.clone();
    let img_end = Rc::clone(&img_rc);
    let state_end = state.clone();
    let win_end = window.clone();
    let done_end = Rc::clone(&completed);
    let toolbar_end = toolbar.clone();
    drag_ctrl.connect_drag_end(move |_, offset_x, offset_y| {
        let imp = canvas_end.imp();
        let mut d = imp.drag.get();
        if !d.active || done_end.get() {
            return;
        }
        d.cur_x = d.start_x + offset_x;
        d.cur_y = d.start_y + offset_y;
        d.active = false;
        imp.drag.set(d);

        let w_logical = canvas_end.width() as f64;
        let h_logical = canvas_end.height() as f64;
        if w_logical <= 1.0 || h_logical <= 1.0 {
            toolbar_end.remove_css_class("ghost-hidden");
            toolbar_end.set_can_target(true);
            return;
        }

        let (rx, ry, rw, rh) = d.normalized_rect();
        if rw < 4.0 || rh < 4.0 {
            toolbar_end.remove_css_class("ghost-hidden");
            toolbar_end.set_can_target(true);
            canvas_end.queue_draw();
            return;
        }

        let scale_x = img_end.width() as f64 / w_logical;
        let scale_y = img_end.height() as f64 / h_logical;

        let px = (rx * scale_x).round().max(0.0) as u32;
        let py = (ry * scale_y).round().max(0.0) as u32;
        let pw = (rw * scale_x).round().max(1.0) as u32;
        let ph = (rh * scale_y).round().max(1.0) as u32;

        if let Ok(cropped) = crop_region(&img_end, px, py, pw, ph) {
            done_end.set(true);
            win_end.close();
            handle_captured_region(state_end.clone(), cropped);
        } else {
            toolbar_end.remove_css_class("ghost-hidden");
            toolbar_end.set_can_target(true);
        }
    });
    canvas.add_controller(drag_ctrl);

    // Right-click cancels
    let rclick = gtk4::GestureClick::new();
    rclick.set_button(3);
    let win_rclick = window.clone();
    rclick.connect_released(move |_, _, _, _| {
        win_rclick.close();
    });
    canvas.add_controller(rclick);

    // Keyboard: Esc cancels, Enter captures full screen
    let key_ctrl = gtk4::EventControllerKey::new();
    let win_key = window.clone();
    let img_key = Rc::clone(&img_rc);
    let state_key = state.clone();
    let done_key = Rc::clone(&completed);
    key_ctrl.connect_key_pressed(move |_, key, _, _| match key {
        gdk::Key::Escape => {
            win_key.close();
            glib::Propagation::Stop
        }
        gdk::Key::Return | gdk::Key::KP_Enter => {
            if !done_key.get() {
                done_key.set(true);
                let full = (*img_key).clone();
                win_key.close();
                handle_captured_region(state_key.clone(), full);
            }
            glib::Propagation::Stop
        }
        _ => glib::Propagation::Proceed,
    });
    window.add_controller(key_ctrl);

    window.set_child(Some(&overlay));
    window.present();
    canvas.grab_focus();
}
