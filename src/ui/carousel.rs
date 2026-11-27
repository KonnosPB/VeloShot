//! 60+ FPS interactive screenshot history carousel supporting all 5 screen positions.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::{cairo, gdk, glib, graphene, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::AnimationExt;

use crate::annotations::{draw_rounded_rect, rgba_to_cairo_surface};
use crate::clipboard::copy_rgba_to_clipboard;
use crate::config::CarouselPosition;
use crate::history::HistoryItem;
use crate::i18n::tr;
use crate::ui::{AppState, editor, icons, plain_toast, preferences, trigger_capture_flow};

struct CarouselState {
    items: Vec<HistoryItem>,
    selected: usize,
    visual_index: f64,
    visual_velocity: f64,
    last_frame_us: Option<i64>,
    thumb_cache: HashMap<String, cairo::ImageSurface>,
    preview_cache: HashMap<String, gdk::Texture>,
    card_rects: Vec<(usize, f64, f64, f64, f64)>,
}

impl CarouselState {
    fn new(items: Vec<HistoryItem>) -> Self {
        Self {
            items,
            selected: 0,
            visual_index: 0.0,
            visual_velocity: 0.0,
            last_frame_us: None,
            thumb_cache: HashMap::new(),
            preview_cache: HashMap::new(),
            card_rects: Vec::new(),
        }
    }

    fn current_item(&self) -> Option<&HistoryItem> {
        self.items.get(self.selected)
    }

    fn move_selection(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let max = (self.items.len() - 1) as isize;
        let next = (self.selected as isize + delta).clamp(0, max) as usize;
        self.selected = next;
    }

    fn get_or_load_thumb(
        &mut self,
        store: &crate::history::HistoryStore,
        item: &HistoryItem,
    ) -> Option<&cairo::ImageSurface> {
        if !self.thumb_cache.contains_key(&item.id)
            && let Ok(thumb_path) = store.ensure_thumbnail(item)
            && let Ok(dyn_img) = image::open(&thumb_path)
            && let Ok(surface) = rgba_to_cairo_surface(&dyn_img.to_rgba8())
        {
            self.thumb_cache.insert(item.id.clone(), surface);
        }
        self.thumb_cache.get(&item.id)
    }

    fn get_or_load_preview(
        &mut self,
        store: &crate::history::HistoryStore,
        item: &HistoryItem,
    ) -> Option<gdk::Texture> {
        if !self.preview_cache.contains_key(&item.id)
            && let Ok(tex) = gdk::Texture::from_filename(store.image_path(item))
        {
            self.preview_cache.insert(item.id.clone(), tex);
        }
        self.preview_cache.get(&item.id).cloned()
    }
}

/// Opens the interactive history carousel at the user-configured screen position.
pub fn open_carousel(state: AppState) {
    let position = state.config.borrow().carousel_position;
    let items = state.store.list();
    let cstate = Rc::new(RefCell::new(CarouselState::new(items)));

    let toast_overlay = adw::ToastOverlay::new();
    let window = gtk4::ApplicationWindow::builder()
        .application(&state.app)
        .title(tr(
            "Verlauf-Karussell — VeloShot",
            "Carousel History — VeloShot",
        ))
        .decorated(false)
        .maximized(true)
        .css_classes(["overlay-window"])
        .child(&toast_overlay)
        .build();

    let (outer_x, outer_y) = compute_outer_margins();
    let backdrop = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(0)
        .hexpand(true)
        .vexpand(true)
        .css_classes(["carousel-backdrop"])
        .build();

    let content_frame = gtk4::Box::builder()
        .orientation(if position.is_vertical() {
            gtk4::Orientation::Horizontal
        } else {
            gtk4::Orientation::Vertical
        })
        .spacing(14)
        .hexpand(true)
        .vexpand(true)
        .margin_start(outer_x)
        .margin_end(outer_x)
        .margin_top(outer_y)
        .margin_bottom(outer_y)
        .build();
    backdrop.append(&content_frame);

    // Large live preview of the currently selected screenshot
    let preview_stage = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .hexpand(true)
        .vexpand(true)
        .css_classes(["carousel-preview-stage"])
        .build();
    let preview_card = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .hexpand(true)
        .vexpand(true)
        .halign(gtk4::Align::Fill)
        .valign(gtk4::Align::Fill)
        .css_classes(["carousel-preview-card"])
        .build();

    let preview_picture = gtk4::Picture::builder()
        .content_fit(gtk4::ContentFit::Contain)
        .can_shrink(true)
        .hexpand(true)
        .vexpand(true)
        .build();
    preview_card.append(&preview_picture);
    preview_stage.append(&preview_card);

    let enter_class = match position {
        CarouselPosition::Top => "entering-top",
        CarouselPosition::Bottom => "entering-bottom",
        CarouselPosition::Left => "entering-left",
        CarouselPosition::Right => "entering-right",
        CarouselPosition::Center => "entering-center",
    };
    let dock = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(10)
        .css_classes(["carousel-dock", enter_class])
        .build();
    configure_dock_placement(&dock, position);

    let meta_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(2)
        .halign(gtk4::Align::Start)
        .valign(gtk4::Align::End)
        .build();

    let date_lbl = gtk4::Label::builder()
        .css_classes(["carousel-date"])
        .halign(gtk4::Align::Start)
        .build();

    let counter_lbl = gtk4::Label::builder()
        .css_classes(["carousel-counter"])
        .halign(gtk4::Align::Start)
        .build();

    meta_box.append(&date_lbl);
    meta_box.append(&counter_lbl);

    let close_btn = gtk4::Button::builder()
        .icon_name("window-close-symbolic")
        .tooltip_text(tr("Schließen (Esc)", "Close (Esc)"))
        .css_classes(["flat", "circular"])
        .valign(gtk4::Align::Center)
        .build();
    let win_close_btn = window.clone();
    close_btn.connect_clicked(move |_| win_close_btn.close());

    // Carousel DrawingArea
    let drawing_area = gtk4::DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .margin_top(if position.is_vertical() { 0 } else { 10 })
        .can_focus(true)
        .focusable(true)
        .build();
    match position {
        CarouselPosition::Top | CarouselPosition::Bottom => {
            drawing_area.set_content_height(195);
        }
        CarouselPosition::Left | CarouselPosition::Right => {
            drawing_area.set_content_width(252);
        }
        CarouselPosition::Center => {
            drawing_area.set_content_height(310);
        }
    }

    // Action bar buttons
    let actions = gtk4::Box::builder()
        .orientation(if position.is_vertical() {
            gtk4::Orientation::Vertical
        } else {
            gtk4::Orientation::Horizontal
        })
        .spacing(6)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .margin_start(12)
        .margin_end(12)
        .build();

    let row_primary = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk4::Align::Center)
        .build();
    let row_secondary = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk4::Align::Center)
        .build();

    let copy_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-copy", 24))
        .tooltip_text(tr(
            "In Zwischenablage kopieren (Enter / Ctrl+C)",
            "Copy to clipboard (Enter / Ctrl+C)",
        ))
        .css_classes(["suggested-action", "tool-icon-btn"])
        .build();
    let edit_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-edit", 24))
        .tooltip_text(tr("Im Editor öffnen (E)", "Open in editor (E)"))
        .css_classes(["tool-icon-btn"])
        .build();
    let save_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-save", 24))
        .tooltip_text(tr("Speichern unter… (Ctrl+S)", "Save as… (Ctrl+S)"))
        .css_classes(["tool-icon-btn"])
        .build();
    let del_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-delete", 24))
        .tooltip_text(tr("Löschen (Entf)", "Delete (Del)"))
        .css_classes(["destructive-action", "tool-icon-btn"])
        .build();
    let new_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-capture-region", 24))
        .tooltip_text(tr("Neue Aufnahme (N)", "New capture (N)"))
        .css_classes(["tool-icon-btn"])
        .build();
    let prefs_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-settings", 24))
        .tooltip_text(tr("Einstellungen (,)", "Preferences (,)"))
        .css_classes(["tool-icon-btn"])
        .build();
    let flip_btn = if !position.is_vertical() {
        let tooltip = format!(
            "{} (F / Tab)",
            tr(
                "Leiste nach oben / unten verschieben",
                "Move dock to top / bottom",
            )
        );
        Some(
            gtk4::Button::builder()
                .child(&icons::icon_image("jas-flip-vertical", 24))
                .tooltip_text(&tooltip)
                .css_classes(["tool-icon-btn"])
                .build(),
        )
    } else {
        None
    };

    row_primary.append(&copy_btn);
    row_primary.append(&edit_btn);
    row_secondary.append(&save_btn);
    row_secondary.append(&del_btn);
    row_secondary.append(&new_btn);
    row_secondary.append(&prefs_btn);
    if let Some(fb) = &flip_btn {
        row_secondary.append(fb);
    }

    actions.append(&row_primary);
    actions.append(&row_secondary);

    if position.is_vertical() {
        let top_close = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .halign(gtk4::Align::End)
            .margin_top(10)
            .margin_end(10)
            .build();
        top_close.append(&close_btn);
        dock.append(&top_close);
        dock.append(&drawing_area);
        dock.append(&actions);
        meta_box.set_margin_start(14);
        meta_box.set_margin_end(12);
        meta_box.set_margin_bottom(12);
        dock.append(&meta_box);
    } else {
        dock.append(&drawing_area);
        let footer = gtk4::CenterBox::builder()
            .margin_start(18)
            .margin_end(14)
            .margin_bottom(12)
            .build();
        footer.set_start_widget(Some(&meta_box));
        footer.set_center_widget(Some(&actions));
        footer.set_end_widget(Some(&close_btn));
        dock.append(&footer);
    }

    match position {
        CarouselPosition::Top | CarouselPosition::Left => {
            content_frame.append(&dock);
            content_frame.append(&preview_stage);
        }
        CarouselPosition::Bottom | CarouselPosition::Right | CarouselPosition::Center => {
            content_frame.append(&preview_stage);
            content_frame.append(&dock);
        }
    }
    toast_overlay.set_child(Some(&backdrop));

    // Helper to refresh counter label, prominent date, full preview, and button sensitivity
    let update_ui_meta = {
        let cstate = Rc::clone(&cstate);
        let store = state.store.clone();
        let counter_lbl = counter_lbl.clone();
        let date_lbl = date_lbl.clone();
        let preview_card = preview_card.clone();
        let preview_picture = preview_picture.clone();
        let copy_btn = copy_btn.clone();
        let edit_btn = edit_btn.clone();
        let save_btn = save_btn.clone();
        let del_btn = del_btn.clone();
        move || {
            let mut st = cstate.borrow_mut();
            let has_items = !st.items.is_empty();
            let total = st.items.len();
            let sel = st.selected;
            if let Some(item) = st.current_item().cloned() {
                let time_str = item.formatted_time();
                let summary = format!("{} / {}  •  {}×{}", sel + 1, total, item.width, item.height);
                counter_lbl.set_text(&summary);
                date_lbl.set_text(&time_str);
                let tex = st.get_or_load_preview(&store, &item);
                preview_picture.set_paintable(tex.as_ref());
                preview_card.set_visible(true);
            } else {
                date_lbl.set_text(tr("Verlauf", "History"));
                counter_lbl.set_text(tr(
                    "Keine Screenshots vorhanden",
                    "No screenshots in history",
                ));
                preview_picture.set_paintable(None::<&gdk::Paintable>);
                preview_card.set_visible(false);
            }
            drop(st);
            copy_btn.set_sensitive(has_items);
            edit_btn.set_sensitive(has_items);
            save_btn.set_sensitive(has_items);
            del_btn.set_sensitive(has_items);
        }
    };
    update_ui_meta();

    // 60+ FPS critically damped spring physics tick callback
    let cstate_tick = Rc::clone(&cstate);
    let area_tick = drawing_area.clone();
    drawing_area.add_tick_callback(move |area, clock| {
        let now = clock.frame_time();
        let mut st = cstate_tick.borrow_mut();
        let dt = match st.last_frame_us {
            Some(prev) => ((now - prev) as f64 / 1_000_000.0).clamp(0.001, 0.033),
            None => 1.0 / 60.0,
        };
        st.last_frame_us = Some(now);

        let target = st.selected as f64;
        let diff = target - st.visual_index;
        if diff.abs() > 0.001 || st.visual_velocity.abs() > 0.002 {
            let stiffness = 190.0;
            let damping = 24.0;
            let accel = stiffness * diff - damping * st.visual_velocity;
            st.visual_velocity += accel * dt;
            st.visual_index += st.visual_velocity * dt;
            if (target - st.visual_index).abs() <= 0.001 && st.visual_velocity.abs() <= 0.002 {
                st.visual_index = target;
                st.visual_velocity = 0.0;
            }
            area.queue_draw();
        }
        let _ = &area_tick;
        glib::ControlFlow::Continue
    });

    // Draw function
    let cstate_draw = Rc::clone(&cstate);
    let store_draw = state.store.clone();
    drawing_area.set_draw_func(move |_area, cr, width, height| {
        let w = width as f64;
        let h = height as f64;
        let mut st = cstate_draw.borrow_mut();
        st.card_rects.clear();

        if st.items.is_empty() {
            let _ = cr.save();
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
            cr.set_font_size(15.0);
            cr.set_source_rgba(0.85, 0.87, 0.92, 0.72);
            let msg = tr(
                "Noch keine Screenshots in der Historie — Taste „Neu“ oder Druck drücken",
                "No screenshots yet — click New or press Print",
            );
            if let Ok(ext) = cr.text_extents(msg) {
                cr.move_to((w - ext.width()) * 0.5, h * 0.5 + 5.0);
                let _ = cr.show_text(msg);
            }
            let _ = cr.restore();
            return;
        }

        let visual_idx = st.visual_index;
        let selected_idx = st.selected;
        let count = st.items.len();

        // Preload surfaces for visible items
        let visible_indices: Vec<usize> = (0..count)
            .filter(|&i| (i as f64 - visual_idx).abs() <= 4.5)
            .collect();
        for &idx in &visible_indices {
            let item = st.items[idx].clone();
            let _ = st.get_or_load_thumb(&store_draw, &item);
        }

        // Draw back-to-front so centered card sits cleanly on top
        let mut ordered = visible_indices;
        ordered.sort_by(|&a, &b| {
            let da = (a as f64 - visual_idx).abs();
            let db = (b as f64 - visual_idx).abs();
            db.total_cmp(&da)
        });

        let vertical = position.is_vertical();
        let center_x = w * 0.5;
        let center_y = h * 0.5;
        let base_card_w = if vertical {
            (w - 36.0).clamp(140.0, 220.0)
        } else if position == CarouselPosition::Center {
            280.0
        } else {
            210.0
        };
        let base_card_h = if vertical {
            132.0
        } else if position == CarouselPosition::Center {
            (h - 46.0).clamp(140.0, 220.0)
        } else {
            (h - 32.0).clamp(110.0, 156.0)
        };
        let spacing = if vertical {
            base_card_h * 0.92
        } else {
            base_card_w * 0.92
        };

        let mut hit_rects = Vec::with_capacity(ordered.len());
        for idx in ordered {
            let offset = idx as f64 - visual_idx;
            let dist = offset.abs();
            let scale = (1.0 - dist * 0.14).clamp(0.62, 1.0);
            let alpha = (1.0 - dist * 0.24).clamp(0.22, 1.0);
            let cw = base_card_w * scale;
            let ch = base_card_h * scale;

            let (cx, cy) = if vertical {
                (center_x - cw * 0.5, center_y + offset * spacing - ch * 0.5)
            } else {
                (center_x + offset * spacing - cw * 0.5, center_y - ch * 0.5)
            };

            hit_rects.push((idx, cx, cy, cw, ch));

            // Card shadow
            let _ = cr.save();
            draw_rounded_rect(cr, cx + 2.0, cy + 4.0, cw, ch, 12.0);
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.35 * alpha);
            let _ = cr.fill();
            let _ = cr.restore();

            // Card background & thumbnail
            let _ = cr.save();
            draw_rounded_rect(cr, cx, cy, cw, ch, 12.0);
            cr.clip();

            cr.set_source_rgba(0.14, 0.15, 0.19, alpha);
            cr.rectangle(cx, cy, cw, ch);
            let _ = cr.fill();

            let item_id = &st.items[idx].id;
            if let Some(surface) = st.thumb_cache.get(item_id) {
                let sw = surface.width() as f64;
                let sh = surface.height() as f64;
                if sw > 0.0 && sh > 0.0 {
                    let fit = (cw / sw).min(ch / sh);
                    let dw = sw * fit;
                    let dh = sh * fit;
                    let dx = cx + (cw - dw) * 0.5;
                    let dy = cy + (ch - dh) * 0.5;
                    let _ = cr.save();
                    cr.translate(dx, dy);
                    cr.scale(fit, fit);
                    let _ = cr.set_source_surface(surface, 0.0, 0.0);
                    let _ = cr.paint_with_alpha(alpha);
                    let _ = cr.restore();
                }
            }
            let _ = cr.restore();

            // Border / active focus ring
            let _ = cr.save();
            draw_rounded_rect(cr, cx, cy, cw, ch, 12.0);
            if idx == selected_idx {
                cr.set_source_rgba(0.24, 0.62, 0.98, 0.96);
                cr.set_line_width(3.0);
            } else {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.18 * alpha);
                cr.set_line_width(1.2);
            }
            let _ = cr.stroke();
            let _ = cr.restore();
        }
        // Store front-to-back for hit-testing
        hit_rects.reverse();
        st.card_rects = hit_rects;
    });

    // Actions closures
    let do_copy = Rc::new({
        let cstate = Rc::clone(&cstate);
        let store = state.store.clone();
        let toast_overlay = toast_overlay.clone();
        let window = window.clone();
        move |close_after: bool| {
            let item = cstate.borrow().current_item().cloned();
            if let Some(item) = item {
                match store.load_image(&item).and_then(|img| copy_rgba_to_clipboard(&img)) {
                    Ok(()) => {
                        if close_after {
                            window.close();
                        } else {
                            toast_overlay.add_toast(plain_toast(tr(
                                "In Zwischenablage kopiert",
                                "Copied to clipboard",
                            )));
                        }
                    }
                    Err(err) => {
                        toast_overlay.add_toast(plain_toast(&format!(
                            "{}: {err}",
                            tr("Fehler", "Error")
                        )));
                    }
                }
            }
        }
    });

    let do_edit = Rc::new({
        let cstate = Rc::clone(&cstate);
        let state = state.clone();
        let window = window.clone();
        let toast_overlay = toast_overlay.clone();
        move || {
            let item = cstate.borrow().current_item().cloned();
            if let Some(item) = item {
                match state.store.load_image(&item) {
                    Ok(img) => {
                        window.close();
                        editor::open_editor(state.clone(), img, Some(item));
                    }
                    Err(err) => {
                        toast_overlay.add_toast(plain_toast(&format!(
                            "{}: {err}",
                            tr("Fehler", "Error")
                        )));
                    }
                }
            }
        }
    });

    let do_save = Rc::new({
        let cstate = Rc::clone(&cstate);
        let state = state.clone();
        let window = window.clone();
        let toast_overlay = toast_overlay.clone();
        move || {
            let item = cstate.borrow().current_item().cloned();
            if let Some(item) = item {
                let (save_dir, initial_name) = {
                    let cfg = state.config.borrow();
                    (
                        cfg.ensure_effective_save_dir(),
                        cfg.format_export_filename(item.created_at, item.width, item.height),
                    )
                };
                let dialog = gtk4::FileDialog::builder()
                    .title(tr("Screenshot speichern", "Save Screenshot"))
                    .initial_name(&initial_name)
                    .initial_folder(&gtk4::gio::File::for_path(save_dir))
                    .build();
                let store = state.store.clone();
                let export_fmt = state.config.borrow().export_format;
                let toast_overlay = toast_overlay.clone();
                dialog.save(
                    Some(&window),
                    gtk4::gio::Cancellable::NONE,
                    move |res| {
                        if let Ok(file) = res
                            && let Some(dest) = file.path()
                        {
                            match store
                                .load_image(&item)
                                .and_then(|img| crate::capture::export_rgba_image(&img, &dest, export_fmt))
                            {
                                Ok(()) => toast_overlay.add_toast(plain_toast(&format!(
                                    "{}: {}",
                                    tr("Gespeichert", "Saved"),
                                    dest.file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("Bild")
                                ))),
                                Err(err) => toast_overlay.add_toast(plain_toast(&format!(
                                    "{}: {err}",
                                    tr("Fehler", "Error")
                                ))),
                            }
                        }
                    },
                );
            }
        }
    });

    let do_delete = Rc::new({
        let cstate = Rc::clone(&cstate);
        let store = state.store.clone();
        let area = drawing_area.clone();
        let update_ui_meta = update_ui_meta.clone();
        let toast_overlay = toast_overlay.clone();
        move || {
            let item = cstate.borrow().current_item().cloned();
            if let Some(item) = item {
                if let Err(err) = store.delete(&item.id) {
                    toast_overlay.add_toast(plain_toast(&format!(
                        "{}: {err}",
                        tr("Löschen fehlgeschlagen", "Delete failed")
                    )));
                    return;
                }
                let mut st = cstate.borrow_mut();
                st.thumb_cache.remove(&item.id);
                st.preview_cache.remove(&item.id);
                st.items.retain(|i| i.id != item.id);
                if st.selected >= st.items.len() && !st.items.is_empty() {
                    st.selected = st.items.len() - 1;
                }
                st.visual_index = st.selected as f64;
                drop(st);
                update_ui_meta();
                area.queue_draw();
            }
        }
    });

    // Wire buttons
    {
        let do_copy = Rc::clone(&do_copy);
        copy_btn.connect_clicked(move |_| do_copy(true));
    }
    {
        let do_edit = Rc::clone(&do_edit);
        edit_btn.connect_clicked(move |_| do_edit());
    }
    {
        let do_save = Rc::clone(&do_save);
        save_btn.connect_clicked(move |_| do_save());
    }
    {
        let do_delete = Rc::clone(&do_delete);
        del_btn.connect_clicked(move |_| do_delete());
    }
    {
        let state_new = state.clone();
        let win_new = window.clone();
        new_btn.connect_clicked(move |_| {
            win_new.close();
            trigger_capture_flow(state_new.clone(), None);
        });
    }
    {
        let state_prefs = state.clone();
        let win_prefs = window.clone();
        prefs_btn.connect_clicked(move |_| {
            preferences::open_preferences(state_prefs.clone(), Some(win_prefs.upcast_ref()));
        });
    }
    let flip_css = gtk4::CssProvider::new();
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &flip_css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
    }
    {
        let flip_css = flip_css.clone();
        window.connect_destroy(move |_| {
            if let Some(display) = gdk::Display::default() {
                gtk4::style_context_remove_provider_for_display(&display, &flip_css);
            }
        });
    }

    let do_flip: Option<Rc<dyn Fn()>> = if !position.is_vertical() {
        let at_bottom = Rc::new(Cell::new(position != CarouselPosition::Top));
        let cur_dock_ty = Rc::new(Cell::new(0.0_f64));
        let cur_preview_ty = Rc::new(Cell::new(0.0_f64));
        let active_anim: Rc<RefCell<Option<adw::SpringAnimation>>> = Rc::new(RefCell::new(None));
        let anim_gen = Rc::new(Cell::new(0_u64));
        let dock_flip = dock.clone();
        let preview_flip = preview_stage.clone();
        let frame_flip = content_frame.clone();
        let state_flip = state.clone();
        let flip_css = flip_css.clone();

        Some(Rc::new(move || {
            let old_dock_y = dock_flip
                .compute_bounds(&frame_flip)
                .map(|b| f64::from(b.y()))
                .unwrap_or(0.0);
            let old_preview_y = preview_flip
                .compute_bounds(&frame_flip)
                .map(|b| f64::from(b.y()))
                .unwrap_or(0.0);
            let container_h = f64::from(frame_flip.height());
            let dock_h = f64::from(dock_flip.height());
            let prev_dock_ty = cur_dock_ty.get();
            let prev_preview_ty = cur_preview_ty.get();

            let my_gen = anim_gen.get().wrapping_add(1);
            anim_gen.set(my_gen);
            if let Some(prev_anim) = active_anim.borrow_mut().take() {
                prev_anim.skip();
            }

            let next_bottom = !at_bottom.get();
            at_bottom.set(next_bottom);
            let next_pos = if next_bottom {
                CarouselPosition::Bottom
            } else {
                CarouselPosition::Top
            };
            {
                let mut cfg = state_flip.config.borrow_mut();
                cfg.carousel_position = next_pos;
                let _ = cfg.save();
            }

            let (delta_dock, delta_preview) = compute_flip_deltas(
                old_dock_y,
                old_preview_y,
                prev_dock_ty,
                prev_preview_ty,
                container_h,
                dock_h,
                14.0,
                next_bottom,
            );

            configure_dock_placement(&dock_flip, next_pos);
            if next_bottom {
                frame_flip.reorder_child_after(&dock_flip, Some(&preview_flip));
            } else {
                frame_flip.reorder_child_after(&preview_flip, Some(&dock_flip));
            }

            dock_flip.add_css_class("flipping");
            preview_flip.add_css_class("flipping");
            cur_dock_ty.set(delta_dock);
            cur_preview_ty.set(delta_preview);
            apply_flip_css(&flip_css, delta_dock, delta_preview);

            let flip_css_tick = flip_css.clone();
            let cur_dock_tick = Rc::clone(&cur_dock_ty);
            let cur_preview_tick = Rc::clone(&cur_preview_ty);
            let target = adw::CallbackAnimationTarget::new(move |val| {
                let d_ty = delta_dock * val;
                let p_ty = delta_preview * val;
                cur_dock_tick.set(d_ty);
                cur_preview_tick.set(p_ty);
                apply_flip_css(&flip_css_tick, d_ty, p_ty);
            });

            let anim = adw::SpringAnimation::new(
                &dock_flip,
                1.0,
                0.0,
                adw::SpringParams::new(0.90, 1.0, 280.0),
                target,
            );

            let anim_gen_done = Rc::clone(&anim_gen);
            let dock_done = dock_flip.clone();
            let preview_done = preview_flip.clone();
            let cur_dock_done = Rc::clone(&cur_dock_ty);
            let cur_preview_done = Rc::clone(&cur_preview_ty);
            let flip_css_done = flip_css.clone();
            let active_anim_done = Rc::clone(&active_anim);
            anim.connect_done(move |_| {
                if anim_gen_done.get() == my_gen {
                    dock_done.remove_css_class("flipping");
                    preview_done.remove_css_class("flipping");
                    cur_dock_done.set(0.0);
                    cur_preview_done.set(0.0);
                    flip_css_done.load_from_string("");
                    *active_anim_done.borrow_mut() = None;
                }
            });

            *active_anim.borrow_mut() = Some(anim.clone());
            anim.play();
        }))
    } else {
        None
    };

    if let (Some(fb), Some(flip)) = (flip_btn, do_flip.as_ref().map(Rc::clone)) {
        fb.connect_clicked(move |_| flip());
    }

    // Mouse / Trackpad scroll
    let scroll_ctrl = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::BOTH_AXES);
    let scroll_accum = Rc::new(Cell::new(0.0_f64));
    let cstate_scroll = Rc::clone(&cstate);
    let area_scroll = drawing_area.clone();
    let update_scroll = update_ui_meta.clone();
    scroll_ctrl.connect_scroll(move |_, dx, dy| {
        let delta = if dy.abs() > dx.abs() { dy } else { dx };
        let next = scroll_accum.get() + delta;
        if next >= 0.45 {
            scroll_accum.set(0.0);
            cstate_scroll.borrow_mut().move_selection(1);
            update_scroll();
            area_scroll.queue_draw();
        } else if next <= -0.45 {
            scroll_accum.set(0.0);
            cstate_scroll.borrow_mut().move_selection(-1);
            update_scroll();
            area_scroll.queue_draw();
        } else {
            scroll_accum.set(next);
        }
        glib::Propagation::Stop
    });
    drawing_area.add_controller(scroll_ctrl);

    // Click card to select; double-click to edit
    let click_card = gtk4::GestureClick::new();
    let cstate_click = Rc::clone(&cstate);
    let area_click = drawing_area.clone();
    let update_click = update_ui_meta.clone();
    let do_edit_click = Rc::clone(&do_edit);
    click_card.connect_pressed(move |_, n_press, x, y| {
        let hit = {
            let st = cstate_click.borrow();
            st.card_rects
                .iter()
                .find(|(_, cx, cy, cw, ch)| x >= *cx && x <= *cx + *cw && y >= *cy && y <= *cy + *ch)
                .map(|(idx, _, _, _, _)| *idx)
        };
        if let Some(idx) = hit {
            cstate_click.borrow_mut().selected = idx;
            update_click();
            area_click.queue_draw();
            if n_press >= 2 {
                do_edit_click();
            }
        }
    });
    drawing_area.add_controller(click_card);

    // Double-click the large preview to open it in the Editor
    let click_preview = gtk4::GestureClick::new();
    let do_edit_preview = Rc::clone(&do_edit);
    click_preview.connect_pressed(move |_, n_press, _, _| {
        if n_press >= 2 {
            do_edit_preview();
        }
    });
    preview_picture.add_controller(click_preview);

    // Click outside dock & preview card closes carousel
    let click_outside = gtk4::GestureClick::new();
    let dock_ref = dock.clone();
    let preview_ref = preview_card.clone();
    let backdrop_ref = backdrop.clone();
    let win_outside = window.clone();
    click_outside.connect_pressed(move |_, _, x, y| {
        let pt = graphene::Point::new(x as f32, y as f32);
        let in_dock = dock_ref
            .compute_bounds(&backdrop_ref)
            .is_some_and(|b| b.contains_point(&pt));
        let in_preview = preview_ref.is_visible()
            && preview_ref
                .compute_bounds(&backdrop_ref)
                .is_some_and(|b| b.contains_point(&pt));
        if !in_dock && !in_preview {
            win_outside.close();
        }
    });
    backdrop.add_controller(click_outside);

    // Keyboard navigation
    let key_ctrl = gtk4::EventControllerKey::new();
    let win_key = window.clone();
    let cstate_key = Rc::clone(&cstate);
    let area_key = drawing_area.clone();
    let update_key = update_ui_meta.clone();
    let do_copy_key = Rc::clone(&do_copy);
    let do_edit_key = Rc::clone(&do_edit);
    let do_save_key = Rc::clone(&do_save);
    let do_delete_key = Rc::clone(&do_delete);
    let do_flip_key = do_flip.clone();
    let state_key = state.clone();

    key_ctrl.connect_key_pressed(move |_, key, _, mods| {
        let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
        match key {
            gdk::Key::Escape => {
                win_key.close();
                glib::Propagation::Stop
            }
            gdk::Key::Left | gdk::Key::Up => {
                cstate_key.borrow_mut().move_selection(-1);
                update_key();
                area_key.queue_draw();
                glib::Propagation::Stop
            }
            gdk::Key::Right | gdk::Key::Down => {
                cstate_key.borrow_mut().move_selection(1);
                update_key();
                area_key.queue_draw();
                glib::Propagation::Stop
            }
            gdk::Key::Return | gdk::Key::KP_Enter => {
                do_copy_key(true);
                glib::Propagation::Stop
            }
            gdk::Key::c | gdk::Key::C if ctrl => {
                do_copy_key(false);
                glib::Propagation::Stop
            }
            gdk::Key::s | gdk::Key::S if ctrl => {
                do_save_key();
                glib::Propagation::Stop
            }
            gdk::Key::e | gdk::Key::E => {
                do_edit_key();
                glib::Propagation::Stop
            }
            gdk::Key::Delete => {
                do_delete_key();
                glib::Propagation::Stop
            }
            gdk::Key::n | gdk::Key::N => {
                win_key.close();
                trigger_capture_flow(state_key.clone(), None);
                glib::Propagation::Stop
            }
            gdk::Key::f | gdk::Key::F | gdk::Key::Tab if !ctrl => {
                if let Some(flip) = &do_flip_key {
                    flip();
                }
                glib::Propagation::Stop
            }
            gdk::Key::comma => {
                preferences::open_preferences(state_key.clone(), Some(win_key.upcast_ref()));
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    window.add_controller(key_ctrl);

    window.present();
    let dock_enter = dock.clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(16), move || {
        dock_enter.remove_css_class(enter_class);
    });
    drawing_area.grab_focus();
}

fn outer_margins_from_size(width: i32, height: i32) -> (i32, i32) {
    let mx = ((width as f64) * 0.05).round() as i32;
    let my = ((height as f64) * 0.05).round() as i32;
    (mx.clamp(40, 180), my.clamp(28, 120))
}

fn compute_outer_margins() -> (i32, i32) {
    if let Some(display) = gdk::Display::default()
        && let Some(obj) = display.monitors().item(0)
        && let Ok(monitor) = obj.downcast::<gdk::Monitor>()
    {
        let geom = monitor.geometry();
        return outer_margins_from_size(geom.width(), geom.height());
    }
    (72, 48)
}

fn configure_dock_placement(dock: &gtk4::Box, position: CarouselPosition) {
    match position {
        CarouselPosition::Top => {
            dock.set_halign(gtk4::Align::Fill);
            dock.set_valign(gtk4::Align::Start);
            dock.set_width_request(-1);
            dock.set_margin_top(0);
            dock.set_margin_bottom(0);
            dock.set_margin_start(0);
            dock.set_margin_end(0);
        }
        CarouselPosition::Bottom => {
            dock.set_halign(gtk4::Align::Fill);
            dock.set_valign(gtk4::Align::End);
            dock.set_width_request(-1);
            dock.set_margin_top(0);
            dock.set_margin_bottom(0);
            dock.set_margin_start(0);
            dock.set_margin_end(0);
        }
        CarouselPosition::Left => {
            dock.set_halign(gtk4::Align::Start);
            dock.set_valign(gtk4::Align::Fill);
            dock.set_margin_start(0);
            dock.set_margin_top(0);
            dock.set_margin_bottom(0);
        }
        CarouselPosition::Right => {
            dock.set_halign(gtk4::Align::End);
            dock.set_valign(gtk4::Align::Fill);
            dock.set_margin_end(0);
            dock.set_margin_top(0);
            dock.set_margin_bottom(0);
        }
        CarouselPosition::Center => {
            dock.set_halign(gtk4::Align::Center);
            dock.set_valign(gtk4::Align::End);
            dock.set_width_request(920);
            dock.set_margin_bottom(0);
        }
    }
}

fn apply_flip_css(provider: &gtk4::CssProvider, dock_ty: f64, preview_ty: f64) {
    let css = format!(
        ".carousel-dock.flipping {{ transform: translateY({dock_ty:.2}px); }}\n\
         .carousel-preview-stage.flipping {{ transform: translateY({preview_ty:.2}px); }}"
    );
    provider.load_from_string(&css);
}

#[allow(clippy::too_many_arguments)]
fn compute_flip_deltas(
    old_dock_y: f64,
    old_preview_y: f64,
    cur_dock_ty: f64,
    cur_preview_ty: f64,
    container_h: f64,
    dock_h: f64,
    spacing: f64,
    next_bottom: bool,
) -> (f64, f64) {
    let (new_dock_y, new_preview_y) = if next_bottom {
        ((container_h - dock_h).max(0.0), 0.0)
    } else {
        (0.0, dock_h + spacing)
    };
    (
        (old_dock_y + cur_dock_ty) - new_dock_y,
        (old_preview_y + cur_preview_ty) - new_preview_y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_percent_outer_margins() {
        assert_eq!(outer_margins_from_size(1920, 1080), (96, 54));
        assert_eq!(outer_margins_from_size(2560, 1440), (128, 72));
        assert_eq!(outer_margins_from_size(640, 480), (40, 28));
    }

    #[test]
    fn flip_deltas_top_bottom_and_midflight_reversal() {
        let container_h = 900.0;
        let dock_h = 240.0;
        let spacing = 14.0;

        // Top -> Bottom from rest
        let (dd_tb, dp_tb) =
            compute_flip_deltas(0.0, 254.0, 0.0, 0.0, container_h, dock_h, spacing, true);
        assert_eq!(dd_tb, -660.0);
        assert_eq!(dp_tb, 254.0);

        // Bottom -> Top from rest
        let (dd_bt, dp_bt) =
            compute_flip_deltas(660.0, 0.0, 0.0, 0.0, container_h, dock_h, spacing, false);
        assert_eq!(dd_bt, 660.0);
        assert_eq!(dp_bt, -254.0);

        // Mid-flight reversal while dock has animated halfway down from Top -> Bottom
        // Visual dock y was 660 + (-330) = 330; reversing back to Top (new_dock_y = 0) -> delta +330.
        // Visual preview y was 0 + 127 = 127; reversing back to Top (new_preview_y = 254) -> delta -127.
        let (dd_mid, dp_mid) = compute_flip_deltas(
            660.0,
            0.0,
            -330.0,
            127.0,
            container_h,
            dock_h,
            spacing,
            false,
        );
        assert_eq!(dd_mid, 330.0);
        assert_eq!(dp_mid, -127.0);
    }
}
