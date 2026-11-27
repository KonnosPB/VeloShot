//! GNOME Adwaita screenshot annotation studio:
//! Rectangle, Arrow, Ellipse, Highlighter, Freehand, Step Badges, Text,
//! Speech Bubble, Blur/Pixelate, Spotlight, Crop, Beautify Frames, and Zoom.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::{cairo, gdk, glib, prelude::*};
use image::RgbaImage;
use libadwaita as adw;

use crate::annotations::{
    Annotation, AnnotationStack, AnnotationTool, BeautifyStyle, Color, apply_all_blurs,
    draw_beautify_backdrop_and_clip, draw_rounded_rect, draw_selection_box,
    draw_vector_annotations, expand_image_to_fit_bounds, normalize_rect,
    render_final_image_with_beautify, rgba_to_cairo_surface,
};
use crate::capture::crop_region;
use crate::clipboard::{copy_rgba_to_clipboard, copy_text_to_clipboard};
use crate::history::HistoryItem;
use crate::i18n::tr;
use crate::ocr;
use crate::ui::{AppState, icons, plain_toast};

#[derive(Clone)]
struct EditorSnapshot {
    base_img: RgbaImage,
    items: Vec<Annotation>,
}

struct EditorState {
    base_img: RgbaImage,
    history_item: Option<HistoryItem>,
    tool: AnnotationTool,
    color: Color,
    fill_color: Color,
    size_value: f64,
    blur_block_size: u32,
    beautify: BeautifyStyle,
    zoom: f64,
    stack: AnnotationStack,
    undo_snapshots: Vec<EditorSnapshot>,
    redo_snapshots: Vec<EditorSnapshot>,
    preview: Option<Annotation>,
    crop_preview: Option<(f64, f64, f64, f64)>,
    freehand_points: Vec<(f64, f64)>,
    blurred_surface: cairo::ImageSurface,
    drag_start: Option<(f64, f64)>,
    selected_index: Option<usize>,
    move_last_pos: Option<(f64, f64)>,
    dragging_tail: bool,
}

impl EditorState {
    fn rebuild_blurred_surface(&mut self) {
        let blurred = apply_all_blurs(&self.base_img, &self.stack.items, self.preview.as_ref());
        if let Ok(surface) = rgba_to_cairo_surface(&blurred) {
            self.blurred_surface = surface;
        }
    }

    fn current_snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            base_img: self.base_img.clone(),
            items: self.stack.items.clone(),
        }
    }

    fn apply_snapshot(&mut self, snap: EditorSnapshot) {
        self.base_img = snap.base_img;
        self.stack.items = snap.items;
        self.selected_index = None;
        self.rebuild_blurred_surface();
    }

    fn push_undo_snapshot(&mut self) {
        let snap = self.current_snapshot();
        if self.undo_snapshots.len() >= 40 {
            self.undo_snapshots.remove(0);
        }
        self.undo_snapshots.push(snap);
        self.redo_snapshots.clear();
    }

    fn can_undo(&self) -> bool {
        !self.undo_snapshots.is_empty()
    }

    fn can_redo(&self) -> bool {
        !self.redo_snapshots.is_empty()
    }

    fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_snapshots.pop() {
            let cur = self.current_snapshot();
            self.redo_snapshots.push(cur);
            self.apply_snapshot(prev);
            true
        } else {
            false
        }
    }

    fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_snapshots.pop() {
            let cur = self.current_snapshot();
            self.undo_snapshots.push(cur);
            self.apply_snapshot(next);
            true
        } else {
            false
        }
    }

    fn widget_to_image(&self, wx: f64, wy: f64) -> (f64, f64) {
        let pad = self.beautify.padding();
        (wx / self.zoom - pad, wy / self.zoom - pad)
    }

    fn canvas_dimensions(&self) -> (i32, i32) {
        let pad = self.beautify.padding() * 2.0;
        let w = ((self.base_img.width() as f64 + pad) * self.zoom)
            .round()
            .max(100.0) as i32;
        let h = ((self.base_img.height() as f64 + pad) * self.zoom)
            .round()
            .max(100.0) as i32;
        (w, h)
    }

    fn expand_canvas_for_bubble(&mut self, index: usize, push_undo: bool) -> bool {
        let Some((min_x, min_y, max_x, max_y)) = self
            .stack
            .items
            .get(index)
            .and_then(|a| a.speech_bubble_bounds())
        else {
            return false;
        };
        let pre_snap = if push_undo {
            Some(self.current_snapshot())
        } else {
            None
        };
        if let Some(expanded) = expand_image_to_fit_bounds(
            &self.base_img,
            &mut self.stack.items,
            min_x,
            min_y,
            max_x,
            max_y,
            16.0,
        ) {
            if let Some(snap) = pre_snap {
                if self.undo_snapshots.len() >= 40 {
                    self.undo_snapshots.remove(0);
                }
                self.undo_snapshots.push(snap);
                self.redo_snapshots.clear();
            }
            self.base_img = expanded;
            self.rebuild_blurred_surface();
            true
        } else {
            false
        }
    }
}

/// Opens the annotation editor for `image` (optionally linked to a `HistoryItem`).
pub fn open_editor(state: AppState, image: RgbaImage, history_item: Option<HistoryItem>) {
    let initial_surface = match rgba_to_cairo_surface(&image) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("Editor-Surface konnte nicht erstellt werden: {err}");
            return;
        }
    };

    let img_w = image.width() as i32;
    let img_h = image.height() as i32;

    let ed = Rc::new(RefCell::new(EditorState {
        base_img: image,
        history_item,
        tool: AnnotationTool::Rectangle,
        color: Color::RED,
        fill_color: Color::TRANSPARENT,
        size_value: 4.0,
        blur_block_size: 14,
        beautify: BeautifyStyle::None,
        zoom: 1.0,
        stack: AnnotationStack::default(),
        undo_snapshots: Vec::new(),
        redo_snapshots: Vec::new(),
        preview: None,
        crop_preview: None,
        freehand_points: Vec::new(),
        blurred_surface: initial_surface,
        drag_start: None,
        selected_index: None,
        move_last_pos: None,
        dragging_tail: false,
    }));
    let syncing_controls = Rc::new(Cell::new(false));

    let toast_overlay = adw::ToastOverlay::new();
    let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    toast_overlay.set_child(Some(&main_box));

    let window = adw::ApplicationWindow::builder()
        .application(&state.app)
        .title("Editor — VeloShot")
        .default_width((img_w + 160).clamp(940, 1420))
        .default_height((img_h + 210).clamp(620, 920))
        .content(&toast_overlay)
        .build();

    let icon_size = state.config.borrow().editor_icon_size;
    let hdr_px = icon_size.header_px();
    let tool_px = icon_size.tool_px();
    let tool_size_cls = icon_size.css_class();
    let sidebar_pad = match icon_size {
        crate::config::EditorIconSize::Small => 4,
        crate::config::EditorIconSize::Medium => 8,
        crate::config::EditorIconSize::Large => 10,
    };

    // --- HeaderBar: Undo/Redo, Title, Copy/Export/Apply ---
    let header = adw::HeaderBar::new();

    let undo_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-undo", hdr_px))
        .tooltip_text(tr("Rückgängig (Ctrl+Z)", "Undo (Ctrl+Z)"))
        .css_classes(["editor-bar-btn", tool_size_cls])
        .sensitive(false)
        .build();
    let redo_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-redo", hdr_px))
        .tooltip_text(tr("Wiederholen (Ctrl+Y)", "Redo (Ctrl+Y)"))
        .css_classes(["editor-bar-btn", tool_size_cls])
        .sensitive(false)
        .build();
    let hist_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .css_classes(["linked"])
        .build();
    hist_box.append(&undo_btn);
    hist_box.append(&redo_btn);
    header.pack_start(&hist_box);

    let history_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-history", hdr_px))
        .tooltip_text(tr(
            "Verlauf-Karussell öffnen (Ctrl+H)",
            "Open Carousel History (Ctrl+H)",
        ))
        .css_classes(["editor-bar-btn", tool_size_cls])
        .build();
    {
        let state_hist = state.clone();
        history_btn.connect_clicked(move |_| {
            crate::ui::carousel::open_carousel(state_hist.clone());
        });
    }
    header.pack_start(&history_btn);

    let apply_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-apply", hdr_px))
        .tooltip_text(tr(
            "Übernehmen — in Historie speichern & kopieren",
            "Apply — save to history & copy",
        ))
        .css_classes(["suggested-action", "editor-bar-btn", tool_size_cls])
        .build();
    let copy_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-copy", hdr_px))
        .tooltip_text(tr(
            "In Zwischenablage kopieren (Ctrl+C)",
            "Copy to clipboard (Ctrl+C)",
        ))
        .css_classes(["editor-bar-btn", tool_size_cls])
        .build();
    let save_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-save", hdr_px))
        .tooltip_text(tr("Exportieren… (Ctrl+S)", "Export… (Ctrl+S)"))
        .css_classes(["editor-bar-btn", tool_size_cls])
        .build();
    let ocr_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-ocr", hdr_px))
        .tooltip_text(tr(
            "Text im Bild per OCR (Tesseract) erkennen & kopieren",
            "Recognize & copy text via OCR (Tesseract)",
        ))
        .css_classes(["editor-bar-btn", tool_size_cls])
        .build();
    header.pack_end(&apply_btn);
    header.pack_end(&save_btn);
    header.pack_end(&copy_btn);
    if ocr::is_tesseract_available() {
        header.pack_end(&ocr_btn);
    }
    main_box.append(&header);

    // --- Left Vertical Tool Sidebar ---
    let tool_scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .propagate_natural_width(true)
        .halign(gtk4::Align::Start)
        .vexpand(true)
        .css_classes(["editor-sidebar", "entering-left"])
        .build();

    let tool_group = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .valign(gtk4::Align::Start)
        .margin_top(sidebar_pad)
        .margin_bottom(sidebar_pad)
        .margin_start(sidebar_pad)
        .margin_end(sidebar_pad)
        .css_classes(["linked"])
        .build();

    let select_btn = gtk4::ToggleButton::builder()
        .child(&icons::icon_image("jas-select", tool_px))
        .css_classes(["tool-icon-btn", tool_size_cls])
        .tooltip_text(tr(
            "Objekte anklicken, verschieben & mit Entf löschen (V)",
            "Select, move & delete objects with Del (V)",
        ))
        .build();
    let rect_btn = gtk4::ToggleButton::builder()
        .child(&icons::icon_image("jas-rect", tool_px))
        .css_classes(["tool-icon-btn", tool_size_cls])
        .active(true)
        .group(&select_btn)
        .tooltip_text(tr("Rechteck zeichnen (1)", "Draw rectangle (1)"))
        .build();
    let arrow_btn = gtk4::ToggleButton::builder()
        .child(&icons::icon_image("jas-arrow", tool_px))
        .css_classes(["tool-icon-btn", tool_size_cls])
        .group(&select_btn)
        .tooltip_text(tr("Pfeil ziehen (2)", "Draw arrow (2)"))
        .build();
    let ellipse_btn = gtk4::ToggleButton::builder()
        .child(&icons::icon_image("jas-ellipse", tool_px))
        .css_classes(["tool-icon-btn", tool_size_cls])
        .group(&select_btn)
        .tooltip_text(tr(
            "Kreis / Ellipse ziehen (3)",
            "Draw circle / ellipse (3)",
        ))
        .build();
    let marker_btn = gtk4::ToggleButton::builder()
        .child(&icons::icon_image("jas-marker", tool_px))
        .css_classes(["tool-icon-btn", tool_size_cls])
        .group(&select_btn)
        .tooltip_text(tr(
            "Halbtransparenter Textmarker (4)",
            "Semi-transparent highlighter (4)",
        ))
        .build();
    let pen_btn = gtk4::ToggleButton::builder()
        .child(&icons::icon_image("jas-pen", tool_px))
        .css_classes(["tool-icon-btn", tool_size_cls])
        .group(&select_btn)
        .tooltip_text(tr("Freihand zeichnen (5)", "Freehand drawing (5)"))
        .build();
    let step_btn = gtk4::ToggleButton::builder()
        .child(&icons::icon_image("jas-step", tool_px))
        .css_classes(["tool-icon-btn", tool_size_cls])
        .group(&select_btn)
        .tooltip_text(tr(
            "Nummerierte Schritte per Klick setzen (6)",
            "Place numbered steps on click (6)",
        ))
        .build();
    let text_btn = gtk4::ToggleButton::builder()
        .child(&icons::icon_image("jas-text", tool_px))
        .css_classes(["tool-icon-btn", tool_size_cls])
        .group(&select_btn)
        .tooltip_text(tr("Text platzieren (7)", "Place text (7)"))
        .build();
    let bubble_btn = gtk4::ToggleButton::builder()
        .child(&icons::icon_image("jas-bubble", tool_px))
        .css_classes(["tool-icon-btn", tool_size_cls])
        .group(&select_btn)
        .tooltip_text(tr(
            "Sprechblase ziehen & beschriften (8)",
            "Draw & label speech bubble (8)",
        ))
        .build();
    let blur_btn = gtk4::ToggleButton::builder()
        .child(&icons::icon_image("jas-blur", tool_px))
        .css_classes(["tool-icon-btn", tool_size_cls])
        .group(&select_btn)
        .tooltip_text(tr(
            "Bereich verpixeln / unkenntlich machen (9)",
            "Pixelate / redact region (9)",
        ))
        .build();
    let spotlight_btn = gtk4::ToggleButton::builder()
        .child(&icons::icon_image("jas-spotlight", tool_px))
        .css_classes(["tool-icon-btn", tool_size_cls])
        .group(&select_btn)
        .tooltip_text(tr(
            "Umgebung abdunkeln & Bereich hervorheben (0)",
            "Dim surroundings & highlight region (0)",
        ))
        .build();
    let crop_btn = gtk4::ToggleButton::builder()
        .child(&icons::icon_image("jas-crop", tool_px))
        .css_classes(["tool-icon-btn", tool_size_cls])
        .group(&select_btn)
        .tooltip_text(tr("Bild zuschneiden (C)", "Crop image (C)"))
        .build();

    for btn in [
        &select_btn,
        &rect_btn,
        &arrow_btn,
        &ellipse_btn,
        &marker_btn,
        &pen_btn,
        &step_btn,
        &text_btn,
        &bubble_btn,
        &blur_btn,
        &spotlight_btn,
        &crop_btn,
    ] {
        tool_group.append(btn);
    }
    tool_scroller.set_child(Some(&tool_group));

    // --- Style & Effects Strip: Colors, Sizes, Beautify Frame, Rotate/Flip, Zoom ---
    let subbar = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(6)
        .margin_start(12)
        .margin_end(12)
        .margin_top(3)
        .margin_bottom(3)
        .css_classes(["editor-subbar"])
        .build();

    let mut swatch_buttons: Vec<(Color, gtk4::Button)> = Vec::new();
    for (idx, (_, color)) in Color::PALETTE.iter().enumerate() {
        let swatch_cls = format!("swatch-{idx}");
        let btn = gtk4::Button::builder()
            .tooltip_text(Color::palette_label(idx))
            .valign(gtk4::Align::Center)
            .css_classes(if idx == 0 {
                vec!["color-swatch", &swatch_cls, "active-swatch"]
            } else {
                vec!["color-swatch", &swatch_cls]
            })
            .build();
        subbar.append(&btn);
        swatch_buttons.push((*color, btn));
    }

    let drawing_area = gtk4::DrawingArea::builder()
        .content_width(img_w)
        .content_height(img_h)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .can_focus(true)
        .focusable(true)
        .build();
    if let Some(cursor) = gdk::Cursor::from_name("crosshair", None) {
        drawing_area.set_cursor(Some(&cursor));
    }

    let swatch_buttons_rc = Rc::new(swatch_buttons);
    for (idx, (color, btn)) in swatch_buttons_rc.iter().enumerate() {
        let ed_color = Rc::clone(&ed);
        let swatches = Rc::clone(&swatch_buttons_rc);
        let area_color = drawing_area.clone();
        let c = *color;
        btn.connect_clicked(move |_| {
            let mut st = ed_color.borrow_mut();
            st.color = c;
            let mut changed = false;
            if let Some(sel) = st.selected_index
                && let Some(ann) = st.stack.items.get_mut(sel)
            {
                changed = ann.set_color(c);
            }
            drop(st);
            for (s_idx, (_, s_btn)) in swatches.iter().enumerate() {
                if s_idx == idx {
                    s_btn.add_css_class("active-swatch");
                } else {
                    s_btn.remove_css_class("active-swatch");
                }
            }
            if changed {
                area_color.queue_draw();
            }
        });
    }

    subbar.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
    let size_icon = icons::icon_image("jas-stroke-width", 20);
    size_icon.set_tooltip_text(Some(tr("Linien- & Schriftstärke", "Stroke & font size")));
    subbar.append(&size_icon);
    let size_spin = gtk4::SpinButton::with_range(1.0, 64.0, 1.0);
    size_spin.set_value(4.0);
    size_spin.set_valign(gtk4::Align::Center);
    size_spin.set_tooltip_text(Some(tr("Linien- & Schriftstärke", "Stroke & font size")));
    {
        let ed_size = Rc::clone(&ed);
        let area_size = drawing_area.clone();
        let sync_size = Rc::clone(&syncing_controls);
        size_spin.connect_value_changed(move |s| {
            let val = s.value();
            let mut st = ed_size.borrow_mut();
            st.size_value = val;
            let mut changed = false;
            if !sync_size.get()
                && let Some(sel) = st.selected_index
                && let Some(ann) = st.stack.items.get_mut(sel)
            {
                changed = ann.set_size(val);
            }
            drop(st);
            if changed {
                area_size.queue_draw();
            }
        });
    }
    subbar.append(&size_spin);

    let blur_icon = icons::icon_image("jas-blur", 20);
    blur_icon.set_tooltip_text(Some(tr("Verpixelungsstärke", "Pixelation strength")));
    subbar.append(&blur_icon);
    let blur_spin = gtk4::SpinButton::with_range(6.0, 48.0, 2.0);
    blur_spin.set_value(14.0);
    blur_spin.set_valign(gtk4::Align::Center);
    blur_spin.set_tooltip_text(Some(tr("Verpixelungsstärke", "Pixelation strength")));
    {
        let ed_blur = Rc::clone(&ed);
        let area_blur = drawing_area.clone();
        let sync_blur = Rc::clone(&syncing_controls);
        blur_spin.connect_value_changed(move |s| {
            let bs = s.value().round() as u32;
            let mut st = ed_blur.borrow_mut();
            st.blur_block_size = bs;
            let mut changed = false;
            if !sync_blur.get()
                && let Some(sel) = st.selected_index
                && let Some(ann) = st.stack.items.get_mut(sel)
            {
                changed = ann.set_blur_block_size(bs);
            }
            if changed {
                st.rebuild_blurred_surface();
            }
            drop(st);
            if changed {
                area_blur.queue_draw();
            }
        });
    }
    subbar.append(&blur_spin);

    fn update_swatch_picker_ui(
        chip: &gtk4::Box,
        buttons: &[gtk4::Button],
        prefix: &str,
        active_idx: usize,
    ) {
        for (i, btn) in buttons.iter().enumerate() {
            chip.remove_css_class(&format!("{prefix}{i}"));
            if i == active_idx {
                btn.add_css_class("active-swatch");
            } else {
                btn.remove_css_class("active-swatch");
            }
        }
        chip.add_css_class(&format!("{prefix}{active_idx}"));
    }

    subbar.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
    let fill_tooltip = tr(
        "Hintergrundfarbe für Formen, Text & Sprechblasen (Standard: Transparent)",
        "Background fill for shapes, text & speech bubbles (default: Transparent)",
    );
    let fill_preview_chip = gtk4::Box::builder()
        .css_classes(["swatch-chip", "fill-swatch-0"])
        .valign(gtk4::Align::Center)
        .build();
    let fill_btn_content = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    fill_btn_content.append(&icons::icon_image("jas-fill", 20));
    fill_btn_content.append(&fill_preview_chip);

    let fill_popover = gtk4::Popover::new();
    let fill_grid = gtk4::Grid::builder()
        .row_spacing(6)
        .column_spacing(6)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();
    let mut fill_buttons: Vec<gtk4::Button> = Vec::new();
    for idx in 0..Color::FILL_PRESETS.len() {
        let swatch_cls = format!("fill-swatch-{idx}");
        let btn = gtk4::Button::builder()
            .tooltip_text(Color::preset_label(idx))
            .css_classes(if idx == 0 {
                vec!["color-swatch", &swatch_cls, "active-swatch"]
            } else {
                vec!["color-swatch", &swatch_cls]
            })
            .build();
        fill_grid.attach(&btn, (idx % 5) as i32, (idx / 5) as i32, 1, 1);
        fill_buttons.push(btn);
    }
    fill_popover.set_child(Some(&fill_grid));
    let fill_buttons_rc = Rc::new(fill_buttons);
    let set_fill_ui_index = Rc::new({
        let chip = fill_preview_chip.clone();
        let btns = Rc::clone(&fill_buttons_rc);
        move |idx: usize| {
            update_swatch_picker_ui(&chip, &btns, "fill-swatch-", idx);
        }
    });
    for (idx, btn) in fill_buttons_rc.iter().enumerate() {
        let ed_bf = Rc::clone(&ed);
        let area_bf = drawing_area.clone();
        let set_ui = Rc::clone(&set_fill_ui_index);
        let pop = fill_popover.clone();
        btn.connect_clicked(move |_| {
            let fill = Color::FILL_PRESETS
                .get(idx)
                .map(|(_, c)| *c)
                .unwrap_or(Color::TRANSPARENT);
            let mut st = ed_bf.borrow_mut();
            st.fill_color = fill;
            let mut changed = false;
            if let Some(sel) = st.selected_index
                && let Some(ann) = st.stack.items.get_mut(sel)
            {
                changed = ann.set_fill(fill);
            }
            drop(st);
            set_ui(idx);
            pop.popdown();
            if changed {
                area_bf.queue_draw();
            }
        });
    }
    let fill_menu_btn = gtk4::MenuButton::builder()
        .child(&fill_btn_content)
        .popover(&fill_popover)
        .tooltip_text(fill_tooltip)
        .valign(gtk4::Align::Center)
        .build();
    subbar.append(&fill_menu_btn);

    let frame_tooltip = tr(
        "Beautify-Rahmen (Verlauf-Hintergrund, abgerundete Ecken & Schatten)",
        "Beautify frame (gradient backdrop, rounded corners & drop shadow)",
    );
    let frame_preview_chip = gtk4::Box::builder()
        .css_classes(["swatch-chip", "frame-swatch-0"])
        .valign(gtk4::Align::Center)
        .build();
    let frame_btn_content = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    frame_btn_content.append(&icons::icon_image("jas-frame", 20));
    frame_btn_content.append(&frame_preview_chip);

    let frame_popover = gtk4::Popover::new();
    let frame_grid = gtk4::Grid::builder()
        .row_spacing(6)
        .column_spacing(6)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();
    let mut frame_buttons: Vec<gtk4::Button> = Vec::new();
    for (idx, style) in BeautifyStyle::ALL.iter().enumerate() {
        let swatch_cls = format!("frame-swatch-{idx}");
        let btn = gtk4::Button::builder()
            .tooltip_text(style.label())
            .css_classes(if idx == 0 {
                vec!["color-swatch", &swatch_cls, "active-swatch"]
            } else {
                vec!["color-swatch", &swatch_cls]
            })
            .build();
        frame_grid.attach(&btn, (idx % 5) as i32, (idx / 5) as i32, 1, 1);
        frame_buttons.push(btn);
    }
    frame_popover.set_child(Some(&frame_grid));
    let frame_buttons_rc = Rc::new(frame_buttons);
    let frame_menu_btn = gtk4::MenuButton::builder()
        .child(&frame_btn_content)
        .popover(&frame_popover)
        .tooltip_text(frame_tooltip)
        .valign(gtk4::Align::Center)
        .build();
    subbar.append(&frame_menu_btn);

    let transform_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .valign(gtk4::Align::Center)
        .css_classes(["linked"])
        .build();
    let rotate_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-rotate", 18))
        .tooltip_text(tr(
            "Bild um 90° im Uhrzeigersinn drehen",
            "Rotate image 90° clockwise",
        ))
        .build();
    let flip_btn = gtk4::Button::builder()
        .child(&icons::icon_image("jas-flip", 18))
        .tooltip_text(tr("Bild horizontal spiegeln", "Flip image horizontally"))
        .build();
    transform_box.append(&rotate_btn);
    transform_box.append(&flip_btn);
    subbar.append(&transform_box);

    let zoom_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .hexpand(true)
        .halign(gtk4::Align::End)
        .valign(gtk4::Align::Center)
        .css_classes(["linked"])
        .build();
    let zoom_out_btn = gtk4::Button::builder()
        .icon_name("zoom-out-symbolic")
        .tooltip_text(tr("Herauszoomen", "Zoom out"))
        .build();
    let zoom_reset_btn = gtk4::Button::builder()
        .label("100%")
        .tooltip_text(tr("Zoom auf 100% zurücksetzen", "Reset zoom to 100%"))
        .build();
    let zoom_in_btn = gtk4::Button::builder()
        .icon_name("zoom-in-symbolic")
        .tooltip_text(tr("Hineinzoomen", "Zoom in"))
        .build();
    zoom_box.append(&zoom_out_btn);
    zoom_box.append(&zoom_reset_btn);
    zoom_box.append(&zoom_in_btn);
    subbar.append(&zoom_box);
    main_box.append(&subbar);

    // --- Canvas inside ScrolledWindow ---
    let scrolled = gtk4::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .css_classes(["editor-canvas-frame"])
        .build();
    scrolled.set_child(Some(&drawing_area));

    let body_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .hexpand(true)
        .vexpand(true)
        .build();
    body_box.append(&tool_scroller);
    body_box.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
    body_box.append(&scrolled);
    main_box.append(&body_box);

    // Inline popover for Text / SpeechBubble text input
    let popover = gtk4::Popover::new();
    popover.set_parent(&drawing_area);
    let pop_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let pop_entry = gtk4::Entry::builder()
        .placeholder_text(tr("Text eingeben…", "Enter text…"))
        .width_chars(24)
        .build();
    let pop_ok = gtk4::Button::builder()
        .label("OK")
        .css_classes(["suggested-action"])
        .build();
    pop_box.append(&pop_entry);
    pop_box.append(&pop_ok);
    popover.set_child(Some(&pop_box));

    #[derive(Clone)]
    enum PendingTextTarget {
        Text { x: f64, y: f64 },
        Bubble { x: f64, y: f64, w: f64, h: f64 },
        EditExisting { index: usize },
    }
    let pending_target: Rc<RefCell<Option<PendingTextTarget>>> = Rc::new(RefCell::new(None));

    fn popup_text_editor(
        popover: &gtk4::Popover,
        entry: &gtk4::Entry,
        initial_text: &str,
        wx: f64,
        wy: f64,
    ) {
        entry.set_text(initial_text);
        if !initial_text.is_empty() {
            entry.select_region(0, -1);
        }
        popover.set_pointing_to(Some(&gdk::Rectangle::new(
            wx.round() as i32,
            wy.round() as i32,
            1,
            1,
        )));
        popover.popup();
        entry.grab_focus();
    }

    let pop_cleanup = popover.clone();
    window.connect_close_request(move |_| {
        pop_cleanup.unparent();
        glib::Propagation::Proceed
    });

    let sync_history_btns = {
        let ed = Rc::clone(&ed);
        let undo_btn = undo_btn.clone();
        let redo_btn = redo_btn.clone();
        move || {
            let st = ed.borrow();
            undo_btn.set_sensitive(st.can_undo());
            redo_btn.set_sensitive(st.can_redo());
        }
    };

    let sync_canvas_size = {
        let ed = Rc::clone(&ed);
        let area = drawing_area.clone();
        let zoom_reset_btn = zoom_reset_btn.clone();
        move || {
            let st = ed.borrow();
            let (cw, ch) = st.canvas_dimensions();
            zoom_reset_btn.set_label(&format!("{}%", (st.zoom * 100.0).round() as u32));
            drop(st);
            area.set_content_width(cw);
            area.set_content_height(ch);
            area.queue_draw();
        }
    };

    // Beautify frame swatch clicks
    for (idx, btn) in frame_buttons_rc.iter().enumerate() {
        let ed = Rc::clone(&ed);
        let sync_canvas_size = sync_canvas_size.clone();
        let chip = frame_preview_chip.clone();
        let btns = Rc::clone(&frame_buttons_rc);
        let pop = frame_popover.clone();
        btn.connect_clicked(move |_| {
            ed.borrow_mut().beautify = BeautifyStyle::from_index(idx as u32);
            update_swatch_picker_ui(&chip, &btns, "frame-swatch-", idx);
            pop.popdown();
            sync_canvas_size();
        });
    }

    // Zoom buttons + Ctrl+Scroll
    {
        let ed = Rc::clone(&ed);
        let sync_canvas_size = sync_canvas_size.clone();
        zoom_out_btn.connect_clicked(move |_| {
            let mut st = ed.borrow_mut();
            st.zoom = (st.zoom - 0.15).clamp(0.25, 3.0);
            drop(st);
            sync_canvas_size();
        });
    }
    {
        let ed = Rc::clone(&ed);
        let sync_canvas_size = sync_canvas_size.clone();
        zoom_reset_btn.connect_clicked(move |_| {
            ed.borrow_mut().zoom = 1.0;
            sync_canvas_size();
        });
    }
    {
        let ed = Rc::clone(&ed);
        let sync_canvas_size = sync_canvas_size.clone();
        zoom_in_btn.connect_clicked(move |_| {
            let mut st = ed.borrow_mut();
            st.zoom = (st.zoom + 0.15).clamp(0.25, 3.0);
            drop(st);
            sync_canvas_size();
        });
    }
    {
        let scroll_zoom =
            gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
        let ed = Rc::clone(&ed);
        let sync_canvas_size = sync_canvas_size.clone();
        scroll_zoom.connect_scroll(move |ctrl, _dx, dy| {
            let mods = ctrl.current_event_state();
            if mods.contains(gdk::ModifierType::CONTROL_MASK) {
                let mut st = ed.borrow_mut();
                st.zoom = (st.zoom - dy * 0.1).clamp(0.25, 3.0);
                drop(st);
                sync_canvas_size();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        scrolled.add_controller(scroll_zoom);
    }

    // Draw func (handles Zoom, Beautify gradient + shadow, Base image, Annotations, Crop preview)
    let ed_draw = Rc::clone(&ed);
    drawing_area.set_draw_func(move |_area, cr, _w, _h| {
        let Ok(st) = ed_draw.try_borrow() else {
            return;
        };
        let img_w = st.base_img.width() as f64;
        let img_h = st.base_img.height() as f64;
        let pad = st.beautify.padding();

        let _ = cr.save();
        cr.scale(st.zoom, st.zoom);

        if pad > 0.0 {
            let _ = draw_beautify_backdrop_and_clip(cr, st.beautify, img_w, img_h);
            draw_rounded_rect(cr, pad, pad, img_w, img_h, 14.0);
            cr.clip();
            cr.translate(pad, pad);
        }

        let _ = cr.set_source_surface(&st.blurred_surface, 0.0, 0.0);
        let _ = cr.paint();
        let _ = draw_vector_annotations(cr, &st.stack.items, st.preview.as_ref(), img_w, img_h);

        if st.tool == AnnotationTool::Select
            && let Some(idx) = st.selected_index
            && let Some(ann) = st.stack.items.get(idx)
        {
            let _ = draw_selection_box(cr, ann);
        }

        if let Some((cx, cy, cw, ch)) = st.crop_preview {
            let (nx, ny, nw, nh) = normalize_rect(cx, cy, cw, ch);
            if nw >= 2.0 && nh >= 2.0 {
                let _ = cr.save();
                cr.set_fill_rule(cairo::FillRule::EvenOdd);
                cr.rectangle(0.0, 0.0, img_w, img_h);
                cr.rectangle(nx, ny, nw, nh);
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.50);
                let _ = cr.fill();

                cr.set_source_rgba(0.98, 0.80, 0.14, 0.96);
                cr.set_line_width(2.0);
                cr.set_dash(&[6.0, 4.0], 0.0);
                cr.rectangle(nx, ny, nw, nh);
                let _ = cr.stroke();
                let _ = cr.restore();
            }
        }
        let _ = cr.restore();
    });

    // Tool selection wiring
    let bind_tool = |btn: &gtk4::ToggleButton, tool: AnnotationTool, default_size: Option<f64>| {
        let ed = Rc::clone(&ed);
        let size_spin = size_spin.clone();
        let area = drawing_area.clone();
        btn.connect_toggled(move |b| {
            if b.is_active() {
                let mut st = ed.borrow_mut();
                st.tool = tool;
                if tool != AnnotationTool::Select {
                    st.selected_index = None;
                }
                drop(st);
                if let Some(sz) = default_size {
                    size_spin.set_value(sz);
                }
                area.queue_draw();
            }
        });
    };
    bind_tool(&select_btn, AnnotationTool::Select, None);
    bind_tool(&rect_btn, AnnotationTool::Rectangle, Some(4.0));
    bind_tool(&arrow_btn, AnnotationTool::Arrow, Some(4.0));
    bind_tool(&ellipse_btn, AnnotationTool::Ellipse, Some(4.0));
    bind_tool(&marker_btn, AnnotationTool::Highlighter, Some(18.0));
    bind_tool(&pen_btn, AnnotationTool::Freehand, Some(4.0));
    bind_tool(&step_btn, AnnotationTool::StepBadge, Some(16.0));
    bind_tool(&text_btn, AnnotationTool::Text, Some(20.0));
    bind_tool(&bubble_btn, AnnotationTool::SpeechBubble, Some(16.0));
    bind_tool(&blur_btn, AnnotationTool::Blur, None);
    bind_tool(&spotlight_btn, AnnotationTool::Spotlight, None);
    bind_tool(&crop_btn, AnnotationTool::Crop, None);

    // Commit pending text / speech bubble
    let commit_text_entry = Rc::new({
        let ed = Rc::clone(&ed);
        let pending_target = Rc::clone(&pending_target);
        let pop_entry = pop_entry.clone();
        let popover = popover.clone();
        let area = drawing_area.clone();
        let sync_history_btns = sync_history_btns.clone();
        let sync_canvas_size = sync_canvas_size.clone();
        move || {
            let text = pop_entry.text().trim().to_string();
            popover.popdown();
            let target = pending_target.borrow_mut().take();
            if text.is_empty() {
                return;
            }
            if let Some(target) = target {
                let mut st = ed.borrow_mut();
                match target {
                    PendingTextTarget::EditExisting { index } => {
                        let unchanged = st.stack.items.get(index).and_then(|a| a.current_text())
                            == Some(text.as_str());
                        if !unchanged && index < st.stack.items.len() {
                            st.push_undo_snapshot();
                            if let Some(ann) = st.stack.items.get_mut(index) {
                                ann.set_text(text);
                            }
                            drop(st);
                            sync_history_btns();
                            area.queue_draw();
                        }
                    }
                    PendingTextTarget::Text { x, y } => {
                        let color = st.color;
                        let fill_color = st.fill_color;
                        let font_size = st.size_value.max(12.0);
                        st.push_undo_snapshot();
                        st.stack.items.push(Annotation::Text {
                            x,
                            y,
                            text,
                            color,
                            fill: fill_color,
                            font_size,
                        });
                        drop(st);
                        sync_history_btns();
                        area.queue_draw();
                    }
                    PendingTextTarget::Bubble { x, y, w, h } => {
                        let color = st.color;
                        let fill_color = st.fill_color;
                        let font_size = st.size_value.max(12.0);
                        st.push_undo_snapshot();
                        st.stack.items.push(Annotation::SpeechBubble {
                            x,
                            y,
                            w,
                            h,
                            tail_x: x + w * 0.25,
                            tail_y: y + h + 26.0,
                            text,
                            fill: fill_color,
                            border: color,
                            text_color: fill_color.contrast_text_color(),
                            font_size,
                        });
                        let new_idx = st.stack.items.len() - 1;
                        let expanded = st.expand_canvas_for_bubble(new_idx, false);
                        drop(st);
                        sync_history_btns();
                        if expanded {
                            sync_canvas_size();
                        } else {
                            area.queue_draw();
                        }
                    }
                }
            }
        }
    });

    {
        let commit = Rc::clone(&commit_text_entry);
        pop_entry.connect_activate(move |_| commit());
    }
    {
        let commit = Rc::clone(&commit_text_entry);
        pop_ok.connect_clicked(move |_| commit());
    }

    // Drag gesture for all drawing/manipulation tools
    let drag = gtk4::GestureDrag::new();
    drag.set_button(1);

    {
        let ed = Rc::clone(&ed);
        let pending_target = Rc::clone(&pending_target);
        let pop_entry = pop_entry.clone();
        let popover = popover.clone();
        let area = drawing_area.clone();
        let sync_history_btns = sync_history_btns.clone();
        let sync_ctrl_begin = Rc::clone(&syncing_controls);
        let size_spin_begin = size_spin.clone();
        let blur_spin_begin = blur_spin.clone();
        let set_fill_ui_begin = Rc::clone(&set_fill_ui_index);
        drag.connect_drag_begin(move |_, wx, wy| {
            let Ok(mut st) = ed.try_borrow_mut() else {
                return;
            };
            let (ix, iy) = st.widget_to_image(wx, wy);
            match st.tool {
                AnnotationTool::Select => {
                    // Check if grabbing the draggable speech-bubble tail handle first
                    let grabbing_tail = st
                        .selected_index
                        .and_then(|idx| st.stack.items.get(idx))
                        .and_then(|ann| ann.speech_bubble_tail())
                        .is_some_and(|(tx, ty)| (ix - tx).hypot(iy - ty) <= 14.0);

                    if grabbing_tail {
                        st.dragging_tail = true;
                        st.drag_start = Some((ix, iy));
                        st.move_last_pos = Some((ix, iy));
                        drop(st);
                        area.queue_draw();
                        return;
                    }

                    st.dragging_tail = false;
                    let hit = st.stack.hit_test_topmost(ix, iy);
                    st.selected_index = hit;
                    st.drag_start = hit.map(|_| (ix, iy));
                    st.move_last_pos = hit.map(|_| (ix, iy));
                    let sel_size = hit
                        .and_then(|idx| st.stack.items.get(idx))
                        .and_then(|ann| ann.current_size());
                    let sel_blur = hit
                        .and_then(|idx| st.stack.items.get(idx))
                        .and_then(|ann| ann.current_blur_block_size());
                    let sel_fill = hit
                        .and_then(|idx| st.stack.items.get(idx))
                        .and_then(|ann| ann.current_fill());
                    drop(st);
                    if sel_size.is_some() || sel_blur.is_some() || sel_fill.is_some() {
                        sync_ctrl_begin.set(true);
                        if let Some(sz) = sel_size {
                            size_spin_begin.set_value(sz);
                        }
                        if let Some(bs) = sel_blur {
                            blur_spin_begin.set_value(bs as f64);
                        }
                        if let Some(bf) = sel_fill {
                            set_fill_ui_begin(bf.preset_index() as usize);
                        }
                        sync_ctrl_begin.set(false);
                    }
                    area.queue_draw();
                }
                AnnotationTool::Text => {
                    // Release RefCell borrow BEFORE popover.popup(), which synchronously
                    // cancels GestureDrag and fires drag_end!
                    drop(st);
                    *pending_target.borrow_mut() = Some(PendingTextTarget::Text { x: ix, y: iy });
                    pop_entry.set_text("");
                    popover.set_pointing_to(Some(&gdk::Rectangle::new(
                        wx.round() as i32,
                        wy.round() as i32,
                        1,
                        1,
                    )));
                    popover.popup();
                    pop_entry.grab_focus();
                }
                AnnotationTool::StepBadge => {
                    let num = st.stack.next_step_number();
                    let color = st.color;
                    let radius = st.size_value.clamp(12.0, 32.0);
                    st.push_undo_snapshot();
                    st.stack.items.push(Annotation::StepBadge {
                        x: ix,
                        y: iy,
                        number: num,
                        color,
                        radius,
                    });
                    drop(st);
                    sync_history_btns();
                    area.queue_draw();
                }
                AnnotationTool::Freehand => {
                    st.drag_start = Some((ix, iy));
                    st.freehand_points = vec![(ix, iy)];
                }
                _ => {
                    st.drag_start = Some((ix, iy));
                }
            }
        });
    }

    {
        let ed = Rc::clone(&ed);
        let area = drawing_area.clone();
        drag.connect_drag_update(move |_, wox, woy| {
            let Ok(mut st) = ed.try_borrow_mut() else {
                return;
            };
            if st.tool == AnnotationTool::Select {
                if let (Some(idx), Some((sx, sy)), Some((lx, ly))) =
                    (st.selected_index, st.drag_start, st.move_last_pos)
                {
                    let cur_x = sx + wox / st.zoom;
                    let cur_y = sy + woy / st.zoom;
                    let dx = cur_x - lx;
                    let dy = cur_y - ly;
                    st.move_last_pos = Some((cur_x, cur_y));
                    if st.dragging_tail {
                        if let Some(Annotation::SpeechBubble { tail_x, tail_y, .. }) =
                            st.stack.items.get_mut(idx)
                        {
                            *tail_x = cur_x;
                            *tail_y = cur_y;
                        }
                        area.queue_draw();
                        return;
                    }
                    let mut is_blur = false;
                    if let Some(ann) = st.stack.items.get_mut(idx) {
                        ann.translate(dx, dy);
                        is_blur = matches!(ann, Annotation::Blur { .. });
                    }
                    if is_blur {
                        st.rebuild_blurred_surface();
                    }
                    area.queue_draw();
                }
                return;
            }
            let Some((sx, sy)) = st.drag_start else {
                return;
            };
            let ox = wox / st.zoom;
            let oy = woy / st.zoom;
            let ex = sx + ox;
            let ey = sy + oy;
            let tool = st.tool;
            let color = st.color;
            let fill = st.fill_color;
            let size = st.size_value;
            let block_size = st.blur_block_size;

            if tool == AnnotationTool::Crop {
                st.crop_preview = Some((sx, sy, ox, oy));
                area.queue_draw();
                return;
            }

            if tool == AnnotationTool::Freehand {
                let should_push = st
                    .freehand_points
                    .last()
                    .map(|&(lx, ly)| (ex - lx).hypot(ey - ly) >= 2.0)
                    .unwrap_or(true);
                if should_push {
                    st.freehand_points.push((ex, ey));
                }
                st.preview = Some(Annotation::Freehand {
                    points: st.freehand_points.clone(),
                    color,
                    line_width: size.clamp(1.0, 24.0),
                });
                area.queue_draw();
                return;
            }

            let preview = match tool {
                AnnotationTool::Rectangle => Some(Annotation::Rectangle {
                    x: sx,
                    y: sy,
                    w: ox,
                    h: oy,
                    color,
                    fill,
                    line_width: size.clamp(1.0, 24.0),
                }),
                AnnotationTool::Arrow => Some(Annotation::Arrow {
                    x1: sx,
                    y1: sy,
                    x2: ex,
                    y2: ey,
                    color,
                    line_width: size.clamp(2.0, 24.0),
                }),
                AnnotationTool::Ellipse => Some(Annotation::Ellipse {
                    x: sx,
                    y: sy,
                    w: ox,
                    h: oy,
                    color,
                    fill,
                    line_width: size.clamp(1.0, 24.0),
                }),
                AnnotationTool::Highlighter => Some(Annotation::Highlighter {
                    x1: sx,
                    y1: sy,
                    x2: ex,
                    y2: ey,
                    color,
                    width: size.max(14.0),
                }),
                AnnotationTool::SpeechBubble => Some(Annotation::SpeechBubble {
                    x: sx,
                    y: sy,
                    w: ox,
                    h: oy,
                    tail_x: sx + ox * 0.25,
                    tail_y: sy + oy + 26.0,
                    text: "…".to_string(),
                    fill,
                    border: color,
                    text_color: fill.contrast_text_color(),
                    font_size: size.max(12.0),
                }),
                AnnotationTool::Blur => Some(Annotation::Blur {
                    x: sx,
                    y: sy,
                    w: ox,
                    h: oy,
                    block_size,
                }),
                AnnotationTool::Spotlight => Some(Annotation::Spotlight {
                    x: sx,
                    y: sy,
                    w: ox,
                    h: oy,
                }),
                _ => None,
            };
            st.preview = preview;
            if tool == AnnotationTool::Blur {
                st.rebuild_blurred_surface();
            }
            area.queue_draw();
        });
    }

    {
        let ed = Rc::clone(&ed);
        let area = drawing_area.clone();
        let sync_history_btns = sync_history_btns.clone();
        let sync_canvas_size = sync_canvas_size.clone();
        let pending_target = Rc::clone(&pending_target);
        let pop_entry = pop_entry.clone();
        let popover = popover.clone();
        let toast_overlay = toast_overlay.clone();

        drag.connect_drag_end(move |_, wox, woy| {
            let Ok(mut st) = ed.try_borrow_mut() else {
                return;
            };
            let Some((sx, sy)) = st.drag_start.take() else {
                return;
            };
            let ox = wox / st.zoom;
            let oy = woy / st.zoom;
            let ex = sx + ox;
            let ey = sy + oy;
            st.preview = None;
            st.crop_preview = None;
            let (nx, ny, nw, nh) = normalize_rect(sx, sy, ox, oy);
            let tool = st.tool;
            let color = st.color;
            let fill = st.fill_color;
            let size = st.size_value;
            let block_size = st.blur_block_size;

            match tool {
                AnnotationTool::Select => {
                    st.move_last_pos = None;
                    st.dragging_tail = false;
                    if let Some(idx) = st.selected_index
                        && st.expand_canvas_for_bubble(idx, true)
                    {
                        drop(st);
                        sync_history_btns();
                        sync_canvas_size();
                        return;
                    }
                }
                AnnotationTool::Rectangle if nw >= 4.0 && nh >= 4.0 => {
                    st.push_undo_snapshot();
                    st.stack.items.push(Annotation::Rectangle {
                        x: nx,
                        y: ny,
                        w: nw,
                        h: nh,
                        color,
                        fill,
                        line_width: size.clamp(1.0, 24.0),
                    });
                }
                AnnotationTool::Arrow if (ex - sx).hypot(ey - sy) >= 8.0 => {
                    st.push_undo_snapshot();
                    st.stack.items.push(Annotation::Arrow {
                        x1: sx,
                        y1: sy,
                        x2: ex,
                        y2: ey,
                        color,
                        line_width: size.clamp(2.0, 24.0),
                    });
                }
                AnnotationTool::Ellipse if nw >= 4.0 && nh >= 4.0 => {
                    st.push_undo_snapshot();
                    st.stack.items.push(Annotation::Ellipse {
                        x: nx,
                        y: ny,
                        w: nw,
                        h: nh,
                        color,
                        fill,
                        line_width: size.clamp(1.0, 24.0),
                    });
                }
                AnnotationTool::Highlighter if (ex - sx).hypot(ey - sy) >= 6.0 => {
                    st.push_undo_snapshot();
                    st.stack.items.push(Annotation::Highlighter {
                        x1: sx,
                        y1: sy,
                        x2: ex,
                        y2: ey,
                        color,
                        width: size.max(14.0),
                    });
                }
                AnnotationTool::Freehand => {
                    let pts = std::mem::take(&mut st.freehand_points);
                    if pts.len() >= 2 {
                        st.push_undo_snapshot();
                        st.stack.items.push(Annotation::Freehand {
                            points: pts,
                            color,
                            line_width: size.clamp(1.0, 24.0),
                        });
                    }
                }
                AnnotationTool::Blur if nw >= 4.0 && nh >= 4.0 => {
                    st.push_undo_snapshot();
                    st.stack.items.push(Annotation::Blur {
                        x: nx,
                        y: ny,
                        w: nw,
                        h: nh,
                        block_size,
                    });
                    st.rebuild_blurred_surface();
                }
                AnnotationTool::Spotlight if nw >= 6.0 && nh >= 6.0 => {
                    st.push_undo_snapshot();
                    st.stack.items.push(Annotation::Spotlight {
                        x: nx,
                        y: ny,
                        w: nw,
                        h: nh,
                    });
                }
                AnnotationTool::Crop if nw >= 10.0 && nh >= 10.0 => {
                    let cx = nx.round().max(0.0) as u32;
                    let cy = ny.round().max(0.0) as u32;
                    let cw = nw.round().max(1.0) as u32;
                    let ch = nh.round().max(1.0) as u32;
                    if let Ok(cropped) = crop_region(&st.base_img, cx, cy, cw, ch) {
                        st.push_undo_snapshot();
                        st.base_img = cropped;
                        for ann in &mut st.stack.items {
                            ann.translate(-(cx as f64), -(cy as f64));
                        }
                        st.selected_index = None;
                        st.rebuild_blurred_surface();
                        drop(st);
                        sync_history_btns();
                        sync_canvas_size();
                        toast_overlay
                            .add_toast(plain_toast(tr("Bild zugeschnitten", "Image cropped")));
                        return;
                    }
                }
                AnnotationTool::SpeechBubble => {
                    let bw = if nw < 24.0 { 170.0 } else { nw.max(90.0) };
                    let bh = if nh < 20.0 { 72.0 } else { nh.max(48.0) };
                    let pad = st.beautify.padding();
                    let zoom = st.zoom;
                    *pending_target.borrow_mut() = Some(PendingTextTarget::Bubble {
                        x: nx,
                        y: ny,
                        w: bw,
                        h: bh,
                    });
                    drop(st);
                    pop_entry.set_text("");
                    popover.set_pointing_to(Some(&gdk::Rectangle::new(
                        ((nx + bw * 0.5 + pad) * zoom).round() as i32,
                        ((ny + bh * 0.5 + pad) * zoom).round() as i32,
                        1,
                        1,
                    )));
                    popover.popup();
                    pop_entry.grab_focus();
                    area.queue_draw();
                    return;
                }
                _ => {
                    if tool == AnnotationTool::Blur {
                        st.rebuild_blurred_surface();
                    }
                }
            }
            st.freehand_points.clear();
            drop(st);
            sync_history_btns();
            area.queue_draw();
        });
    }
    drawing_area.add_controller(drag);

    // Double-click any Text or SpeechBubble annotation to edit its text inline
    let dbl_click = gtk4::GestureClick::new();
    dbl_click.set_button(1);
    {
        let ed = Rc::clone(&ed);
        let pending_target = Rc::clone(&pending_target);
        let area = drawing_area.clone();
        let pop_entry = pop_entry.clone();
        let popover = popover.clone();
        dbl_click.connect_pressed(move |_, n_press, wx, wy| {
            if n_press != 2 {
                return;
            }
            let Ok(mut st) = ed.try_borrow_mut() else {
                return;
            };
            let (ix, iy) = st.widget_to_image(wx, wy);
            let Some(idx) = st.stack.hit_test_topmost(ix, iy) else {
                return;
            };
            let Some(existing) = st
                .stack
                .items
                .get(idx)
                .and_then(|ann| ann.current_text())
                .map(str::to_string)
            else {
                return;
            };
            st.selected_index = Some(idx);
            st.drag_start = None;
            st.move_last_pos = None;
            st.dragging_tail = false;
            st.preview = None;
            st.crop_preview = None;
            // Release RefCell borrow before popover.popup(), which cancels GestureDrag!
            drop(st);
            *pending_target.borrow_mut() = Some(PendingTextTarget::EditExisting { index: idx });
            popup_text_editor(&popover, &pop_entry, &existing, wx, wy);
            area.queue_draw();
        });
    }
    drawing_area.add_controller(dbl_click);

    // Undo / Redo actions
    let do_undo = Rc::new({
        let ed = Rc::clone(&ed);
        let sync_history_btns = sync_history_btns.clone();
        let sync_canvas_size = sync_canvas_size.clone();
        move || {
            let changed = ed.borrow_mut().undo();
            if changed {
                sync_history_btns();
                sync_canvas_size();
            }
        }
    });

    let do_redo = Rc::new({
        let ed = Rc::clone(&ed);
        let sync_history_btns = sync_history_btns.clone();
        let sync_canvas_size = sync_canvas_size.clone();
        move || {
            let changed = ed.borrow_mut().redo();
            if changed {
                sync_history_btns();
                sync_canvas_size();
            }
        }
    });

    {
        let do_undo = Rc::clone(&do_undo);
        undo_btn.connect_clicked(move |_| do_undo());
    }
    {
        let do_redo = Rc::clone(&do_redo);
        redo_btn.connect_clicked(move |_| do_redo());
    }

    // Export / Copy / Apply actions (include active Beautify frame!)
    let render_current = Rc::new({
        let ed = Rc::clone(&ed);
        move || {
            let st = ed.borrow();
            render_final_image_with_beautify(&st.base_img, &st.stack.items, st.beautify)
        }
    });

    let do_copy = Rc::new({
        let render_current = Rc::clone(&render_current);
        let toast_overlay = toast_overlay.clone();
        move || match render_current().and_then(|img| copy_rgba_to_clipboard(&img)) {
            Ok(()) => toast_overlay.add_toast(plain_toast(tr(
                "Bearbeitetes Bild kopiert",
                "Edited image copied",
            ))),
            Err(err) => {
                toast_overlay.add_toast(plain_toast(&format!("{}: {err}", tr("Fehler", "Error"))))
            }
        }
    });

    let do_save = Rc::new({
        let render_current = Rc::clone(&render_current);
        let state = state.clone();
        let window = window.clone();
        let toast_overlay = toast_overlay.clone();
        let ed = Rc::clone(&ed);
        move || {
            let (created_at, w, h) = {
                let st = ed.borrow();
                (
                    st.history_item
                        .as_ref()
                        .map(|i| i.created_at)
                        .unwrap_or_else(chrono::Utc::now),
                    st.base_img.width(),
                    st.base_img.height(),
                )
            };
            let (save_dir, suggested_name) = {
                let cfg = state.config.borrow();
                (
                    cfg.ensure_effective_save_dir(),
                    cfg.format_export_filename(created_at, w, h),
                )
            };
            let dialog = gtk4::FileDialog::builder()
                .title(tr("Screenshot exportieren", "Export Screenshot"))
                .initial_name(&suggested_name)
                .initial_folder(&gtk4::gio::File::for_path(save_dir))
                .build();
            let render_current = Rc::clone(&render_current);
            let export_fmt = state.config.borrow().export_format;
            let toast_overlay = toast_overlay.clone();
            dialog.save(Some(&window), gtk4::gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res
                    && let Some(dest) = file.path()
                {
                    match render_current()
                        .and_then(|img| crate::capture::export_rgba_image(&img, &dest, export_fmt))
                    {
                        Ok(()) => toast_overlay.add_toast(plain_toast(&format!(
                            "{}: {}",
                            tr("Exportiert", "Exported"),
                            dest.file_name().and_then(|n| n.to_str()).unwrap_or("Bild")
                        ))),
                        Err(err) => toast_overlay
                            .add_toast(plain_toast(&format!("{}: {err}", tr("Fehler", "Error")))),
                    }
                }
            });
        }
    });

    let do_apply = Rc::new({
        let ed = Rc::clone(&ed);
        let state = state.clone();
        let render_current = Rc::clone(&render_current);
        let toast_overlay = toast_overlay.clone();
        move || match render_current() {
            Ok(rendered) => {
                let existing_id = ed.borrow().history_item.as_ref().map(|i| i.id.clone());
                let max_hist = state.config.borrow().max_history;
                let res = match existing_id {
                    Some(id) => state.store.update_image(&id, &rendered),
                    None => state.store.add_image(&rendered, max_hist),
                };
                match res {
                    Ok(updated) => {
                        ed.borrow_mut().history_item = Some(updated);
                        let _ = copy_rgba_to_clipboard(&rendered);
                        toast_overlay.add_toast(plain_toast(tr(
                            "In Historie übernommen & in Zwischenablage kopiert",
                            "Saved to history & copied to clipboard",
                        )));
                    }
                    Err(err) => {
                        toast_overlay
                            .add_toast(plain_toast(&format!("{}: {err}", tr("Fehler", "Error"))));
                    }
                }
            }
            Err(err) => {
                toast_overlay.add_toast(plain_toast(&format!("{}: {err}", tr("Fehler", "Error"))))
            }
        }
    });

    {
        let do_copy = Rc::clone(&do_copy);
        copy_btn.connect_clicked(move |_| do_copy());
    }
    {
        let do_save = Rc::clone(&do_save);
        save_btn.connect_clicked(move |_| do_save());
    }
    {
        let do_apply = Rc::clone(&do_apply);
        apply_btn.connect_clicked(move |_| do_apply());
    }

    // Rotate 90° CW & Flip Horizontal
    {
        let ed = Rc::clone(&ed);
        let sync_canvas_size = sync_canvas_size.clone();
        let sync_history_btns = sync_history_btns.clone();
        let toast_overlay = toast_overlay.clone();
        rotate_btn.connect_clicked(move |_| {
            let mut st = ed.borrow_mut();
            st.push_undo_snapshot();
            let old_h = st.base_img.height() as f64;
            st.base_img = image::imageops::rotate90(&st.base_img);
            for ann in &mut st.stack.items {
                ann.rotate_90_cw(old_h);
            }
            st.selected_index = None;
            st.rebuild_blurred_surface();
            drop(st);
            sync_history_btns();
            sync_canvas_size();
            toast_overlay.add_toast(plain_toast(tr("Um 90° gedreht", "Rotated 90°")));
        });
    }
    {
        let ed = Rc::clone(&ed);
        let area = drawing_area.clone();
        let sync_history_btns = sync_history_btns.clone();
        let toast_overlay = toast_overlay.clone();
        flip_btn.connect_clicked(move |_| {
            let mut st = ed.borrow_mut();
            st.push_undo_snapshot();
            let width = st.base_img.width() as f64;
            st.base_img = image::imageops::flip_horizontal(&st.base_img);
            for ann in &mut st.stack.items {
                ann.flip_horizontal(width);
            }
            st.selected_index = None;
            st.rebuild_blurred_surface();
            drop(st);
            sync_history_btns();
            area.queue_draw();
            toast_overlay.add_toast(plain_toast(tr(
                "Horizontal gespiegelt",
                "Flipped horizontally",
            )));
        });
    }

    // Offline OCR via Tesseract
    {
        let render_current = Rc::clone(&render_current);
        let toast_overlay = toast_overlay.clone();
        let ocr_btn_ref = ocr_btn.clone();
        ocr_btn.connect_clicked(move |btn| {
            let img = match render_current() {
                Ok(img) => img,
                Err(err) => {
                    toast_overlay.add_toast(plain_toast(&format!(
                        "{}: {err}",
                        tr("OCR-Fehler", "OCR error")
                    )));
                    return;
                }
            };
            btn.set_sensitive(false);
            let rx = ocr::spawn_ocr_async(img);
            let toast_overlay = toast_overlay.clone();
            let btn_done = ocr_btn_ref.clone();
            glib::MainContext::default().spawn_local(async move {
                match rx.recv().await {
                    Ok(Ok(text)) => {
                        let chars = text.chars().count();
                        match copy_text_to_clipboard(&text) {
                            Ok(()) => toast_overlay.add_toast(plain_toast(
                                &tr(
                                    "Text erkannt & kopiert ({chars} Zeichen)",
                                    "Text recognized & copied ({chars} chars)",
                                )
                                .replace("{chars}", &chars.to_string()),
                            )),
                            Err(err) => toast_overlay.add_toast(plain_toast(&format!(
                                "{}: {err}",
                                tr("Fehler", "Error")
                            ))),
                        }
                    }
                    Ok(Err(err)) => {
                        toast_overlay.add_toast(plain_toast(&format!("{err}")));
                    }
                    Err(_) => {}
                }
                btn_done.set_sensitive(true);
            });
        });
    }

    // Keyboard shortcuts inside editor
    let key_ctrl = gtk4::EventControllerKey::new();
    let win_key = window.clone();
    let do_undo_k = Rc::clone(&do_undo);
    let do_redo_k = Rc::clone(&do_redo);
    let do_copy_k = Rc::clone(&do_copy);
    let do_save_k = Rc::clone(&do_save);
    let ed_key = Rc::clone(&ed);
    let area_key = drawing_area.clone();
    let pop_key = popover.clone();
    let sync_hist_key = sync_history_btns.clone();
    let state_key = state.clone();

    key_ctrl.connect_key_pressed(move |_, key, _, mods| {
        let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
        match key {
            gdk::Key::Escape => {
                win_key.close();
                glib::Propagation::Stop
            }
            gdk::Key::h | gdk::Key::H if ctrl => {
                crate::ui::carousel::open_carousel(state_key.clone());
                glib::Propagation::Stop
            }
            gdk::Key::Delete | gdk::Key::BackSpace if !pop_key.is_visible() => {
                let mut st = ed_key.borrow_mut();
                if let Some(idx) = st.selected_index.take()
                    && idx < st.stack.items.len()
                {
                    st.push_undo_snapshot();
                    st.stack.remove_at(idx);
                    st.rebuild_blurred_surface();
                    drop(st);
                    sync_hist_key();
                    area_key.queue_draw();
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            }
            gdk::Key::z | gdk::Key::Z if ctrl => {
                do_undo_k();
                glib::Propagation::Stop
            }
            gdk::Key::y | gdk::Key::Y if ctrl => {
                do_redo_k();
                glib::Propagation::Stop
            }
            gdk::Key::c | gdk::Key::C if ctrl => {
                do_copy_k();
                glib::Propagation::Stop
            }
            gdk::Key::s | gdk::Key::S if ctrl => {
                do_save_k();
                glib::Propagation::Stop
            }
            gdk::Key::v | gdk::Key::V if !ctrl => {
                select_btn.set_active(true);
                glib::Propagation::Stop
            }
            gdk::Key::_1 if !ctrl => {
                rect_btn.set_active(true);
                glib::Propagation::Stop
            }
            gdk::Key::_2 if !ctrl => {
                arrow_btn.set_active(true);
                glib::Propagation::Stop
            }
            gdk::Key::_3 if !ctrl => {
                ellipse_btn.set_active(true);
                glib::Propagation::Stop
            }
            gdk::Key::_4 if !ctrl => {
                marker_btn.set_active(true);
                glib::Propagation::Stop
            }
            gdk::Key::_5 if !ctrl => {
                pen_btn.set_active(true);
                glib::Propagation::Stop
            }
            gdk::Key::_6 if !ctrl => {
                step_btn.set_active(true);
                glib::Propagation::Stop
            }
            gdk::Key::_7 if !ctrl => {
                text_btn.set_active(true);
                glib::Propagation::Stop
            }
            gdk::Key::_8 if !ctrl => {
                bubble_btn.set_active(true);
                glib::Propagation::Stop
            }
            gdk::Key::_9 if !ctrl => {
                blur_btn.set_active(true);
                glib::Propagation::Stop
            }
            gdk::Key::_0 if !ctrl => {
                spotlight_btn.set_active(true);
                glib::Propagation::Stop
            }
            gdk::Key::c | gdk::Key::C if !ctrl => {
                crop_btn.set_active(true);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    window.add_controller(key_ctrl);

    if state.config.borrow().auto_copy_clipboard {
        let ed_clip = Rc::clone(&ed);
        let copied_once = Rc::new(Cell::new(false));
        let schedule_initial_copy = {
            let ed_c = Rc::clone(&ed_clip);
            let done = Rc::clone(&copied_once);
            move || {
                if done.get() {
                    return;
                }
                done.set(true);
                let ed_delayed = Rc::clone(&ed_c);
                glib::timeout_add_local_once(std::time::Duration::from_millis(80), move || {
                    let _ = copy_rgba_to_clipboard(&ed_delayed.borrow().base_img);
                });
            }
        };
        {
            let trigger = schedule_initial_copy.clone();
            window.connect_is_active_notify(move |win| {
                if win.is_active() {
                    trigger();
                }
            });
        }
        window.present();
        if window.is_active() {
            schedule_initial_copy();
        }
    } else {
        window.present();
    }
    let sidebar_enter = tool_scroller.clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(16), move || {
        sidebar_enter.remove_css_class("entering-left");
    });
}
