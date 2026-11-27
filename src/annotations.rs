//! Annotation model, pixelation/blur, beautify frames, Cairo rendering, and lossless PNG export.

use anyhow::{Result, anyhow};
use gtk4::cairo;
use image::{Rgba, RgbaImage};

use crate::i18n::tr;

/// RGBA color in normalized 0.0..=1.0 range for Cairo drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Color {
    pub const fn rgba(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }

    pub const RED: Self = Self::rgba(0.92, 0.22, 0.22, 1.0);
    pub const ORANGE: Self = Self::rgba(0.96, 0.52, 0.14, 1.0);
    pub const YELLOW: Self = Self::rgba(0.98, 0.80, 0.14, 1.0);
    pub const GREEN: Self = Self::rgba(0.18, 0.76, 0.42, 1.0);
    pub const BLUE: Self = Self::rgba(0.21, 0.52, 0.90, 1.0);
    pub const PURPLE: Self = Self::rgba(0.57, 0.25, 0.67, 1.0);
    pub const GRAY: Self = Self::rgba(0.52, 0.54, 0.58, 1.0);
    pub const WHITE: Self = Self::rgba(1.0, 1.0, 1.0, 1.0);
    pub const BLACK: Self = Self::rgba(0.08, 0.08, 0.10, 1.0);
    pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);

    pub const PALETTE: [(&'static str, Self); 9] = [
        ("Rot", Self::RED),
        ("Orange", Self::ORANGE),
        ("Gelb", Self::YELLOW),
        ("Grün", Self::GREEN),
        ("Blau", Self::BLUE),
        ("Violett", Self::PURPLE),
        ("Grau", Self::GRAY),
        ("Weiß", Self::WHITE),
        ("Schwarz", Self::BLACK),
    ];

    pub fn palette_label(idx: usize) -> &'static str {
        match idx {
            0 => tr("Rot", "Red"),
            1 => tr("Orange", "Orange"),
            2 => tr("Gelb", "Yellow"),
            3 => tr("Grün", "Green"),
            4 => tr("Blau", "Blue"),
            5 => tr("Violett", "Purple"),
            6 => tr("Grau", "Gray"),
            7 => tr("Weiß", "White"),
            _ => tr("Schwarz", "Black"),
        }
    }

    pub const FILL_PRESETS: [(&'static str, Self); 10] = [
        ("Transparent", Self::TRANSPARENT),
        ("Weiß", Self::WHITE),
        ("Gelb", Self::YELLOW),
        ("Dunkel", Self::rgba(0.11, 0.12, 0.16, 0.90)),
        ("Blau", Self::BLUE),
        ("Violett", Self::PURPLE),
        ("Grau", Self::GRAY),
        ("Rot", Self::RED),
        ("Grün", Self::GREEN),
        ("Orange", Self::ORANGE),
    ];

    pub fn preset_label(idx: usize) -> &'static str {
        match idx {
            0 => tr("Transparent", "Transparent"),
            1 => tr("Weiß", "White"),
            2 => tr("Gelb", "Yellow"),
            3 => tr("Dunkel", "Dark"),
            4 => tr("Blau", "Blue"),
            5 => tr("Violett", "Purple"),
            6 => tr("Grau", "Gray"),
            7 => tr("Rot", "Red"),
            8 => tr("Grün", "Green"),
            _ => tr("Orange", "Orange"),
        }
    }

    pub fn contrast_text_color(self) -> Self {
        if self.a < 0.05 {
            return Self::WHITE;
        }
        let luma = 0.299 * self.r + 0.587 * self.g + 0.114 * self.b;
        if luma > 0.62 {
            Self::BLACK
        } else {
            Self::WHITE
        }
    }

    pub fn preset_index(self) -> u32 {
        Self::FILL_PRESETS
            .iter()
            .position(|(_, c)| {
                (c.a - self.a).abs() < 0.05
                    && (c.a < 0.05
                        || ((c.r - self.r).abs() < 0.05
                            && (c.g - self.g).abs() < 0.05
                            && (c.b - self.b).abs() < 0.05))
            })
            .unwrap_or(0) as u32
    }
}

/// Optional gradient frame + rounded corners + drop shadow around the final screenshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BeautifyStyle {
    #[default]
    None,
    GnomeBlue,
    Sunset,
    SlateDark,
    Mint,
}

impl BeautifyStyle {
    pub const ALL: [BeautifyStyle; 5] = [
        BeautifyStyle::None,
        BeautifyStyle::GnomeBlue,
        BeautifyStyle::Sunset,
        BeautifyStyle::SlateDark,
        BeautifyStyle::Mint,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BeautifyStyle::None => tr("Kein Rahmen", "No Frame"),
            BeautifyStyle::GnomeBlue => tr("GNOME Blau", "GNOME Blue"),
            BeautifyStyle::Sunset => tr("Sonnenuntergang", "Sunset"),
            BeautifyStyle::SlateDark => tr("Dunkel-Schiefer", "Dark Slate"),
            BeautifyStyle::Mint => "Mint",
        }
    }

    pub fn from_index(idx: u32) -> Self {
        Self::ALL.get(idx as usize).copied().unwrap_or_default()
    }

    pub fn padding(self) -> f64 {
        match self {
            BeautifyStyle::None => 0.0,
            _ => 36.0,
        }
    }

    pub fn gradient_stops(self) -> ((f64, f64, f64), (f64, f64, f64)) {
        match self {
            BeautifyStyle::None => ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0)),
            BeautifyStyle::GnomeBlue => ((0.18, 0.45, 0.88), (0.42, 0.22, 0.78)),
            BeautifyStyle::Sunset => ((0.94, 0.36, 0.28), (0.96, 0.68, 0.22)),
            BeautifyStyle::SlateDark => ((0.13, 0.16, 0.22), (0.24, 0.29, 0.38)),
            BeautifyStyle::Mint => ((0.12, 0.68, 0.56), (0.20, 0.50, 0.82)),
        }
    }
}

/// Active tool in the annotation editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnnotationTool {
    Select,
    #[default]
    Rectangle,
    Arrow,
    Ellipse,
    Highlighter,
    Freehand,
    StepBadge,
    Text,
    SpeechBubble,
    Blur,
    Spotlight,
    Crop,
}

/// Single vector or pixel annotation in image pixel coordinates.
#[derive(Debug, Clone, PartialEq)]
pub enum Annotation {
    Rectangle {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: Color,
        fill: Color,
        line_width: f64,
    },
    Arrow {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: Color,
        line_width: f64,
    },
    Ellipse {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: Color,
        fill: Color,
        line_width: f64,
    },
    Highlighter {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: Color,
        width: f64,
    },
    Freehand {
        points: Vec<(f64, f64)>,
        color: Color,
        line_width: f64,
    },
    StepBadge {
        x: f64,
        y: f64,
        number: u32,
        color: Color,
        radius: f64,
    },
    Text {
        x: f64,
        y: f64,
        text: String,
        color: Color,
        fill: Color,
        font_size: f64,
    },
    SpeechBubble {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        tail_x: f64,
        tail_y: f64,
        text: String,
        fill: Color,
        border: Color,
        text_color: Color,
        font_size: f64,
    },
    Blur {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        block_size: u32,
    },
    Spotlight {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    },
}

impl Annotation {
    /// Shifts coordinates by `(dx, dy)` (used after cropping the base image).
    pub fn translate(&mut self, dx: f64, dy: f64) {
        match self {
            Annotation::Rectangle { x, y, .. }
            | Annotation::Ellipse { x, y, .. }
            | Annotation::StepBadge { x, y, .. }
            | Annotation::Text { x, y, .. }
            | Annotation::Blur { x, y, .. }
            | Annotation::Spotlight { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Annotation::Arrow { x1, y1, x2, y2, .. }
            | Annotation::Highlighter { x1, y1, x2, y2, .. } => {
                *x1 += dx;
                *y1 += dy;
                *x2 += dx;
                *y2 += dy;
            }
            Annotation::Freehand { points, .. } => {
                for (px, py) in points {
                    *px += dx;
                    *py += dy;
                }
            }
            Annotation::SpeechBubble {
                x,
                y,
                tail_x,
                tail_y,
                ..
            } => {
                *x += dx;
                *y += dy;
                *tail_x += dx;
                *tail_y += dy;
            }
        }
    }

    /// Returns axis-aligned `(x, y, w, h)` bounding box in image pixel coordinates.
    pub fn bounding_box(&self) -> (f64, f64, f64, f64) {
        match self {
            Annotation::Rectangle { x, y, w, h, .. }
            | Annotation::Ellipse { x, y, w, h, .. }
            | Annotation::SpeechBubble { x, y, w, h, .. }
            | Annotation::Blur { x, y, w, h, .. }
            | Annotation::Spotlight { x, y, w, h } => normalize_rect(*x, *y, *w, *h),
            Annotation::Arrow {
                x1, y1, x2, y2, ..
            } => {
                let x = x1.min(*x2);
                let y = y1.min(*y2);
                (x, y, (x2 - x1).abs().max(12.0), (y2 - y1).abs().max(12.0))
            }
            Annotation::Highlighter {
                x1,
                y1,
                x2,
                y2,
                width,
                ..
            } => {
                let pad = (*width * 0.5).max(8.0);
                let x = x1.min(*x2) - pad;
                let y = y1.min(*y2) - pad;
                let w = (x2 - x1).abs() + pad * 2.0;
                let h = (y2 - y1).abs() + pad * 2.0;
                (x, y, w, h)
            }
            Annotation::Freehand {
                points,
                line_width,
                ..
            } => {
                if points.is_empty() {
                    return (0.0, 0.0, 0.0, 0.0);
                }
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                for &(px, py) in points {
                    min_x = min_x.min(px);
                    min_y = min_y.min(py);
                    max_x = max_x.max(px);
                    max_y = max_y.max(py);
                }
                let pad = (*line_width).max(6.0);
                (
                    min_x - pad,
                    min_y - pad,
                    (max_x - min_x) + pad * 2.0,
                    (max_y - min_y) + pad * 2.0,
                )
            }
            Annotation::StepBadge { x, y, radius, .. } => {
                let r = radius.max(12.0);
                (x - r, y - r, r * 2.0, r * 2.0)
            }
            Annotation::Text {
                x,
                y,
                text,
                font_size,
                ..
            } => {
                let lines: Vec<&str> = text.lines().collect();
                let line_count = lines.len().max(1) as f64;
                let max_chars = lines
                    .iter()
                    .map(|l| l.chars().count())
                    .max()
                    .unwrap_or(1)
                    .max(1) as f64;
                let w = (max_chars * font_size * 0.62).max(24.0);
                let h = line_count * font_size * 1.25;
                (*x, *y, w, h)
            }
        }
    }

    /// Returns `true` if `(px, py)` hits this annotation (with 6px selection margin, or 12px on bubble tail).
    pub fn hit_test(&self, px: f64, py: f64) -> bool {
        if let Annotation::SpeechBubble { tail_x, tail_y, .. } = self
            && (px - tail_x).hypot(py - tail_y) <= 12.0
        {
            return true;
        }
        let (x, y, w, h) = self.bounding_box();
        let pad = 6.0;
        px >= x - pad && px <= x + w + pad && py >= y - pad && py <= y + h + pad
    }

    /// Returns the draggable tail tip coordinates if this annotation is a `SpeechBubble`.
    pub fn speech_bubble_tail(&self) -> Option<(f64, f64)> {
        match self {
            Annotation::SpeechBubble { tail_x, tail_y, .. } => Some((*tail_x, *tail_y)),
            _ => None,
        }
    }

    /// Updates the primary color of the selected annotation; returns `false` if not colorable.
    pub fn set_color(&mut self, new_color: Color) -> bool {
        match self {
            Annotation::Rectangle { color, .. }
            | Annotation::Arrow { color, .. }
            | Annotation::Ellipse { color, .. }
            | Annotation::Highlighter { color, .. }
            | Annotation::Freehand { color, .. }
            | Annotation::StepBadge { color, .. }
            | Annotation::Text { color, .. } => {
                *color = new_color;
                true
            }
            Annotation::SpeechBubble { border, .. } => {
                *border = new_color;
                true
            }
            Annotation::Blur { .. } | Annotation::Spotlight { .. } => false,
        }
    }

    /// Updates stroke width / font size / badge radius of the selected annotation.
    pub fn set_size(&mut self, size: f64) -> bool {
        match self {
            Annotation::Rectangle { line_width, .. }
            | Annotation::Ellipse { line_width, .. }
            | Annotation::Freehand { line_width, .. } => {
                *line_width = size.clamp(1.0, 24.0);
                true
            }
            Annotation::Arrow { line_width, .. } => {
                *line_width = size.clamp(2.0, 24.0);
                true
            }
            Annotation::Highlighter { width, .. } => {
                *width = size.max(12.0);
                true
            }
            Annotation::StepBadge { radius, .. } => {
                *radius = size.clamp(12.0, 32.0);
                true
            }
            Annotation::Text { font_size, .. }
            | Annotation::SpeechBubble { font_size, .. } => {
                *font_size = size.max(10.0);
                true
            }
            Annotation::Blur { .. } | Annotation::Spotlight { .. } => false,
        }
    }

    /// Returns the editable size (stroke / font / radius) if applicable.
    pub fn current_size(&self) -> Option<f64> {
        match self {
            Annotation::Rectangle { line_width, .. }
            | Annotation::Arrow { line_width, .. }
            | Annotation::Ellipse { line_width, .. }
            | Annotation::Freehand { line_width, .. } => Some(*line_width),
            Annotation::Highlighter { width, .. } => Some(*width),
            Annotation::StepBadge { radius, .. } => Some(*radius),
            Annotation::Text { font_size, .. }
            | Annotation::SpeechBubble { font_size, .. } => Some(*font_size),
            Annotation::Blur { .. } | Annotation::Spotlight { .. } => None,
        }
    }

    /// Updates pixelation block size if this annotation is a `Blur`.
    pub fn set_blur_block_size(&mut self, new_block_size: u32) -> bool {
        if let Annotation::Blur { block_size, .. } = self {
            *block_size = new_block_size.clamp(4, 64);
            true
        } else {
            false
        }
    }

    /// Returns pixelation block size if this annotation is a `Blur`.
    pub fn current_blur_block_size(&self) -> Option<u32> {
        match self {
            Annotation::Blur { block_size, .. } => Some(*block_size),
            _ => None,
        }
    }

    /// Updates background fill color on `Rectangle`, `Ellipse`, `Text`, or `SpeechBubble`.
    pub fn set_fill(&mut self, new_fill: Color) -> bool {
        match self {
            Annotation::Rectangle { fill, .. }
            | Annotation::Ellipse { fill, .. }
            | Annotation::Text { fill, .. } => {
                *fill = new_fill;
                true
            }
            Annotation::SpeechBubble {
                fill, text_color, ..
            } => {
                *fill = new_fill;
                *text_color = new_fill.contrast_text_color();
                true
            }
            _ => false,
        }
    }

    /// Returns the background fill color if this annotation supports background fills.
    pub fn current_fill(&self) -> Option<Color> {
        match self {
            Annotation::Rectangle { fill, .. }
            | Annotation::Ellipse { fill, .. }
            | Annotation::Text { fill, .. }
            | Annotation::SpeechBubble { fill, .. } => Some(*fill),
            _ => None,
        }
    }

    /// Returns the editable text of a `Text` or `SpeechBubble` annotation.
    pub fn current_text(&self) -> Option<&str> {
        match self {
            Annotation::Text { text, .. } | Annotation::SpeechBubble { text, .. } => {
                Some(text.as_str())
            }
            _ => None,
        }
    }

    /// Updates the text of a `Text` or `SpeechBubble` annotation; returns `false` otherwise.
    pub fn set_text(&mut self, new_text: String) -> bool {
        match self {
            Annotation::Text { text, .. } | Annotation::SpeechBubble { text, .. } => {
                *text = new_text;
                true
            }
            _ => false,
        }
    }

    /// Rotates coordinates 90° clockwise given the previous image height `old_h`.
    pub fn rotate_90_cw(&mut self, old_h: f64) {
        match self {
            Annotation::Rectangle { x, y, w, h, .. }
            | Annotation::Ellipse { x, y, w, h, .. }
            | Annotation::Blur { x, y, w, h, .. }
            | Annotation::Spotlight { x, y, w, h } => {
                let (nx, ny, nw, nh) = normalize_rect(*x, *y, *w, *h);
                *x = old_h - (ny + nh);
                *y = nx;
                *w = nh;
                *h = nw;
            }
            Annotation::Arrow { x1, y1, x2, y2, .. }
            | Annotation::Highlighter { x1, y1, x2, y2, .. } => {
                let (ox1, oy1, ox2, oy2) = (*x1, *y1, *x2, *y2);
                *x1 = old_h - oy1;
                *y1 = ox1;
                *x2 = old_h - oy2;
                *y2 = ox2;
            }
            Annotation::Freehand { points, .. } => {
                for (px, py) in points {
                    let (ox, oy) = (*px, *py);
                    *px = old_h - oy;
                    *py = ox;
                }
            }
            Annotation::StepBadge { x, y, .. } | Annotation::Text { x, y, .. } => {
                let (ox, oy) = (*x, *y);
                *x = old_h - oy;
                *y = ox;
            }
            Annotation::SpeechBubble {
                x,
                y,
                w,
                h,
                tail_x,
                tail_y,
                ..
            } => {
                let (nx, ny, nw, nh) = normalize_rect(*x, *y, *w, *h);
                let (otx, oty) = (*tail_x, *tail_y);
                *x = old_h - (ny + nh);
                *y = nx;
                *w = nh;
                *h = nw;
                *tail_x = old_h - oty;
                *tail_y = otx;
            }
        }
    }

    /// Mirrors coordinates horizontally across an image of width `width`.
    pub fn flip_horizontal(&mut self, width: f64) {
        match self {
            Annotation::Rectangle { x, w, .. }
            | Annotation::Ellipse { x, w, .. }
            | Annotation::Blur { x, w, .. }
            | Annotation::Spotlight { x, w, .. } => {
                *x = width - (*x + *w);
            }
            Annotation::Arrow { x1, x2, .. } | Annotation::Highlighter { x1, x2, .. } => {
                *x1 = width - *x1;
                *x2 = width - *x2;
            }
            Annotation::Freehand { points, .. } => {
                for (px, _) in points {
                    *px = width - *px;
                }
            }
            Annotation::StepBadge { x, .. } | Annotation::Text { x, .. } => {
                *x = width - *x;
            }
            Annotation::SpeechBubble { x, w, tail_x, .. } => {
                *x = width - (*x + *w);
                *tail_x = width - *tail_x;
            }
        }
    }
}

/// Undo/redo stack for annotations.
#[derive(Debug, Default, Clone)]
pub struct AnnotationStack {
    pub items: Vec<Annotation>,
    #[cfg(test)]
    redo_stack: Vec<Annotation>,
}

impl AnnotationStack {
    #[cfg(test)]
    pub fn push(&mut self, ann: Annotation) {
        self.items.push(ann);
        self.redo_stack.clear();
    }

    #[cfg(test)]
    pub fn undo(&mut self) -> bool {
        if let Some(item) = self.items.pop() {
            self.redo_stack.push(item);
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    pub fn redo(&mut self) -> bool {
        if let Some(item) = self.redo_stack.pop() {
            self.items.push(item);
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    pub fn can_undo(&self) -> bool {
        !self.items.is_empty()
    }

    #[cfg(test)]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Returns the next sequential number for `StepBadge`.
    pub fn next_step_number(&self) -> u32 {
        self.items
            .iter()
            .filter_map(|a| match a {
                Annotation::StepBadge { number, .. } => Some(*number),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Returns the index of the topmost annotation hitting `(px, py)`.
    pub fn hit_test_topmost(&self, px: f64, py: f64) -> Option<usize> {
        self.items.iter().rposition(|ann| ann.hit_test(px, py))
    }

    /// Removes the annotation at `index`.
    pub fn remove_at(&mut self, index: usize) -> Option<Annotation> {
        if index < self.items.len() {
            Some(self.items.remove(index))
        } else {
            None
        }
    }
}

/// Draws a crisp dashed selection frame with corner handles around `ann`.
pub fn draw_selection_box(cr: &cairo::Context, ann: &Annotation) -> Result<()> {
    let (x, y, w, h) = ann.bounding_box();
    let pad = 4.0;
    let rx = x - pad;
    let ry = y - pad;
    let rw = w + pad * 2.0;
    let rh = h + pad * 2.0;

    cr.save()?;
    cr.set_source_rgba(0.21, 0.52, 0.90, 0.95);
    cr.set_line_width(1.6);
    cr.set_dash(&[5.0, 4.0], 0.0);
    cr.rectangle(rx, ry, rw, rh);
    cr.stroke()?;

    cr.set_dash(&[], 0.0);
    let handle = 6.0;
    for (hx, hy) in [
        (rx, ry),
        (rx + rw, ry),
        (rx, ry + rh),
        (rx + rw, ry + rh),
    ] {
        cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        cr.rectangle(hx - handle * 0.5, hy - handle * 0.5, handle, handle);
        cr.fill_preserve()?;
        cr.set_source_rgba(0.21, 0.52, 0.90, 1.0);
        cr.set_line_width(1.4);
        cr.stroke()?;
    }

    if let Some((tx, ty)) = ann.speech_bubble_tail() {
        let cx = x + w * 0.5;
        let cy = y + h * 0.5;
        cr.set_dash(&[3.0, 3.0], 0.0);
        cr.set_source_rgba(0.96, 0.52, 0.14, 0.75);
        cr.set_line_width(1.2);
        cr.move_to(cx, cy);
        cr.line_to(tx, ty);
        cr.stroke()?;

        cr.set_dash(&[], 0.0);
        cr.arc(tx, ty, 6.5, 0.0, std::f64::consts::TAU);
        cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        cr.fill_preserve()?;
        cr.set_source_rgba(0.96, 0.45, 0.08, 1.0);
        cr.set_line_width(2.0);
        cr.stroke()?;
    }
    cr.restore()?;
    Ok(())
}

/// Normalizes potentially negative width/height drag coordinates into `(x, y, w, h)`.
pub fn normalize_rect(x: f64, y: f64, w: f64, h: f64) -> (f64, f64, f64, f64) {
    let nx = if w < 0.0 { x + w } else { x };
    let ny = if h < 0.0 { y + h } else { y };
    (nx, ny, w.abs(), h.abs())
}

/// Converts an `RgbaImage` (straight RGBA) to a Cairo `ARgb32` surface (premultiplied BGRA on LE).
pub fn rgba_to_cairo_surface(img: &RgbaImage) -> Result<cairo::ImageSurface> {
    let w = i32::try_from(img.width()).map_err(|_| anyhow!("Bild zu breit"))?;
    let h = i32::try_from(img.height()).map_err(|_| anyhow!("Bild zu hoch"))?;
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)
        .map_err(|e| anyhow!("Cairo-Surface fehlgeschlagen: {e}"))?;
    let stride = surface.stride() as usize;
    {
        let mut data = surface
            .data()
            .map_err(|e| anyhow!("Cairo-Daten gesperrt: {e}"))?;
        for y in 0..img.height() as usize {
            let row_offset = y * stride;
            for x in 0..img.width() as usize {
                let p = img.get_pixel(x as u32, y as u32).0;
                let a = p[3] as u32;
                let r = ((p[0] as u32 * a + 127) / 255) as u8;
                let g = ((p[1] as u32 * a + 127) / 255) as u8;
                let b = ((p[2] as u32 * a + 127) / 255) as u8;
                let idx = row_offset + x * 4;
                data[idx] = b;
                data[idx + 1] = g;
                data[idx + 2] = r;
                data[idx + 3] = p[3];
            }
        }
    }
    surface.mark_dirty();
    Ok(surface)
}

/// Converts a Cairo `ARgb32` surface back to an `RgbaImage` (un-premultiplied RGBA).
pub fn cairo_surface_to_rgba(surface: &mut cairo::ImageSurface) -> Result<RgbaImage> {
    surface.flush();
    let w = surface.width() as u32;
    let h = surface.height() as u32;
    let stride = surface.stride() as usize;
    let data = surface
        .data()
        .map_err(|e| anyhow!("Cairo-Daten gesperrt: {e}"))?;
    let mut out = RgbaImage::new(w, h);
    for y in 0..h as usize {
        let row_offset = y * stride;
        for x in 0..w as usize {
            let idx = row_offset + x * 4;
            let b = data[idx];
            let g = data[idx + 1];
            let r = data[idx + 2];
            let a = data[idx + 3];
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
            out.put_pixel(x as u32, y as u32, Rgba([ur, ug, ub, a]));
        }
    }
    Ok(out)
}

/// Applies irreversible block pixelation ("Verwischen") to a rectangular region.
pub fn apply_pixelate_region(img: &mut RgbaImage, x: f64, y: f64, w: f64, h: f64, block_size: u32) {
    let (nx, ny, nw, nh) = normalize_rect(x, y, w, h);
    let bs = block_size.max(4);
    let img_w = img.width();
    let img_h = img.height();
    if img_w == 0 || img_h == 0 || nw < 1.0 || nh < 1.0 {
        return;
    }

    let x0 = (nx.floor().max(0.0) as u32).min(img_w);
    let y0 = (ny.floor().max(0.0) as u32).min(img_h);
    let x1 = (nx + nw).ceil().max(0.0).min(img_w as f64) as u32;
    let y1 = (ny + nh).ceil().max(0.0).min(img_h as f64) as u32;

    let mut by = y0;
    while by < y1 {
        let bh = (by + bs).min(y1);
        let mut bx = x0;
        while bx < x1 {
            let bw = (bx + bs).min(x1);
            let mut sum_r: u64 = 0;
            let mut sum_g: u64 = 0;
            let mut sum_b: u64 = 0;
            let mut sum_a: u64 = 0;
            let mut count: u64 = 0;

            for py in by..bh {
                for px in bx..bw {
                    let p = img.get_pixel(px, py).0;
                    sum_r += p[0] as u64;
                    sum_g += p[1] as u64;
                    sum_b += p[2] as u64;
                    sum_a += p[3] as u64;
                    count += 1;
                }
            }

            if count > 0 {
                let avg = Rgba([
                    (sum_r / count) as u8,
                    (sum_g / count) as u8,
                    (sum_b / count) as u8,
                    (sum_a / count) as u8,
                ]);
                for py in by..bh {
                    for px in bx..bw {
                        img.put_pixel(px, py, avg);
                    }
                }
            }
            bx = bw;
        }
        by = bh;
    }
}

/// Builds a pixelated copy of `base_img` with all `Blur` annotations applied.
pub fn apply_all_blurs(
    base_img: &RgbaImage,
    annotations: &[Annotation],
    preview: Option<&Annotation>,
) -> RgbaImage {
    let mut working = base_img.clone();
    for ann in annotations.iter().chain(preview.into_iter()) {
        if let Annotation::Blur {
            x,
            y,
            w,
            h,
            block_size,
        } = ann
        {
            apply_pixelate_region(&mut working, *x, *y, *w, *h, *block_size);
        }
    }
    working
}

/// Draws vector annotations via Cairo for live preview or final image export.
pub fn draw_vector_annotations(
    cr: &cairo::Context,
    annotations: &[Annotation],
    preview: Option<&Annotation>,
    canvas_w: f64,
    canvas_h: f64,
) -> Result<()> {
    for ann in annotations {
        draw_single_annotation(cr, ann, false, canvas_w, canvas_h)?;
    }
    if let Some(p) = preview {
        draw_single_annotation(cr, p, true, canvas_w, canvas_h)?;
    }
    Ok(())
}

/// Draws a beautified gradient backdrop and drop shadow behind a screenshot surface.
pub fn draw_beautify_backdrop_and_clip(
    cr: &cairo::Context,
    style: BeautifyStyle,
    img_w: f64,
    img_h: f64,
) -> Result<()> {
    let pad = style.padding();
    if pad <= 0.0 {
        return Ok(());
    }
    let total_w = img_w + pad * 2.0;
    let total_h = img_h + pad * 2.0;
    let (c1, c2) = style.gradient_stops();

    let grad = cairo::LinearGradient::new(0.0, 0.0, total_w, total_h);
    grad.add_color_stop_rgba(0.0, c1.0, c1.1, c1.2, 1.0);
    grad.add_color_stop_rgba(1.0, c2.0, c2.1, c2.2, 1.0);
    cr.set_source(&grad)?;
    cr.rectangle(0.0, 0.0, total_w, total_h);
    cr.fill()?;

    // Soft multi-layer rounded drop shadow beneath the screenshot
    for step in (1..=6).rev() {
        let spread = step as f64 * 2.5;
        let alpha = 0.055 * (7 - step) as f64 / 6.0;
        draw_rounded_rect(
            cr,
            pad - spread * 0.45,
            pad + spread * 0.65,
            img_w + spread * 0.9,
            img_h + spread * 0.9,
            14.0 + spread * 0.35,
        );
        cr.set_source_rgba(0.0, 0.0, 0.0, alpha);
        cr.fill()?;
    }

    Ok(())
}

/// Renders the base image plus all annotations (without beautify frame) onto an `RgbaImage`.
#[cfg(test)]
pub fn render_final_image(base_img: &RgbaImage, annotations: &[Annotation]) -> Result<RgbaImage> {
    render_final_image_with_beautify(base_img, annotations, BeautifyStyle::None)
}

/// Renders base image + annotations + optional beautify gradient frame & rounded shadow.
pub fn render_final_image_with_beautify(
    base_img: &RgbaImage,
    annotations: &[Annotation],
    beautify: BeautifyStyle,
) -> Result<RgbaImage> {
    let blurred = apply_all_blurs(base_img, annotations, None);
    let base_surface = rgba_to_cairo_surface(&blurred)?;
    let img_w = blurred.width() as f64;
    let img_h = blurred.height() as f64;
    let pad = beautify.padding();
    let out_w = (img_w + pad * 2.0).round() as i32;
    let out_h = (img_h + pad * 2.0).round() as i32;

    let mut out_surface = cairo::ImageSurface::create(cairo::Format::ARgb32, out_w, out_h)
        .map_err(|e| anyhow!("Ausgabe-Surface fehlgeschlagen: {e}"))?;

    {
        let cr = cairo::Context::new(&out_surface)
            .map_err(|e| anyhow!("Cairo-Kontext fehlgeschlagen: {e}"))?;

        if pad > 0.0 {
            draw_beautify_backdrop_and_clip(&cr, beautify, img_w, img_h)?;
            cr.save()?;
            draw_rounded_rect(&cr, pad, pad, img_w, img_h, 14.0);
            cr.clip();
            cr.translate(pad, pad);
            cr.set_source_surface(&base_surface, 0.0, 0.0)?;
            cr.paint()?;
            draw_vector_annotations(&cr, annotations, None, img_w, img_h)?;
            cr.restore()?;
        } else {
            cr.set_source_surface(&base_surface, 0.0, 0.0)?;
            cr.paint()?;
            draw_vector_annotations(&cr, annotations, None, img_w, img_h)?;
        }
    }

    cairo_surface_to_rgba(&mut out_surface)
}

fn draw_single_annotation(
    cr: &cairo::Context,
    ann: &Annotation,
    is_preview: bool,
    canvas_w: f64,
    canvas_h: f64,
) -> Result<()> {
    match ann {
        Annotation::Rectangle {
            x,
            y,
            w,
            h,
            color,
            fill,
            line_width,
        } => {
            let (nx, ny, nw, nh) = normalize_rect(*x, *y, *w, *h);
            if nw < 1.0 || nh < 1.0 {
                return Ok(());
            }
            cr.save()?;
            draw_rounded_rect(cr, nx, ny, nw, nh, 4.0);
            if fill.a >= 0.05 {
                cr.set_source_rgba(fill.r, fill.g, fill.b, fill.a);
                cr.fill_preserve()?;
            }
            cr.set_source_rgba(color.r, color.g, color.b, color.a);
            cr.set_line_width(*line_width);
            cr.set_line_join(cairo::LineJoin::Round);
            cr.stroke()?;
            cr.restore()?;
        }
        Annotation::Arrow {
            x1,
            y1,
            x2,
            y2,
            color,
            line_width,
        } => {
            let dx = x2 - x1;
            let dy = y2 - y1;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 4.0 {
                return Ok(());
            }
            let angle = dy.atan2(dx);
            let head_len = (*line_width * 3.6).clamp(14.0, 34.0).min(len * 0.65);
            let head_angle = 0.48;
            let shaft_end_x = x2 - head_len * 0.72 * angle.cos();
            let shaft_end_y = y2 - head_len * 0.72 * angle.sin();
            let p1x = x2 - head_len * (angle - head_angle).cos();
            let p1y = y2 - head_len * (angle - head_angle).sin();
            let p2x = x2 - head_len * (angle + head_angle).cos();
            let p2y = y2 - head_len * (angle + head_angle).sin();

            cr.save()?;
            // Subtle shadow
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.35);
            cr.set_line_width(*line_width);
            cr.set_line_cap(cairo::LineCap::Round);
            cr.move_to(x1 + 1.5, y1 + 1.5);
            cr.line_to(shaft_end_x + 1.5, shaft_end_y + 1.5);
            cr.stroke()?;
            cr.move_to(x2 + 1.5, y2 + 1.5);
            cr.line_to(p1x + 1.5, p1y + 1.5);
            cr.line_to(p2x + 1.5, p2y + 1.5);
            cr.close_path();
            cr.fill()?;

            // Colored shaft + head
            cr.set_source_rgba(color.r, color.g, color.b, color.a);
            cr.move_to(*x1, *y1);
            cr.line_to(shaft_end_x, shaft_end_y);
            cr.stroke()?;
            cr.move_to(*x2, *y2);
            cr.line_to(p1x, p1y);
            cr.line_to(p2x, p2y);
            cr.close_path();
            cr.fill()?;
            cr.restore()?;
        }
        Annotation::Ellipse {
            x,
            y,
            w,
            h,
            color,
            fill,
            line_width,
        } => {
            let (nx, ny, nw, nh) = normalize_rect(*x, *y, *w, *h);
            if nw < 2.0 || nh < 2.0 {
                return Ok(());
            }
            cr.save()?;
            cr.translate(nx + nw * 0.5, ny + nh * 0.5);
            cr.scale(nw * 0.5, nh * 0.5);
            cr.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
            cr.restore()?;
            cr.save()?;
            if fill.a >= 0.05 {
                cr.set_source_rgba(fill.r, fill.g, fill.b, fill.a);
                cr.fill_preserve()?;
            }
            cr.set_source_rgba(color.r, color.g, color.b, color.a);
            cr.set_line_width(*line_width);
            cr.stroke()?;
            cr.restore()?;
        }
        Annotation::Highlighter {
            x1,
            y1,
            x2,
            y2,
            color,
            width,
        } => {
            cr.save()?;
            cr.set_source_rgba(color.r, color.g, color.b, 0.36);
            cr.set_line_width((*width).max(12.0));
            cr.set_line_cap(cairo::LineCap::Round);
            cr.move_to(*x1, *y1);
            cr.line_to(*x2, *y2);
            cr.stroke()?;
            cr.restore()?;
        }
        Annotation::Freehand {
            points,
            color,
            line_width,
        } => {
            if points.len() < 2 {
                return Ok(());
            }
            cr.save()?;
            cr.set_source_rgba(color.r, color.g, color.b, color.a);
            cr.set_line_width(*line_width);
            cr.set_line_cap(cairo::LineCap::Round);
            cr.set_line_join(cairo::LineJoin::Round);
            cr.move_to(points[0].0, points[0].1);
            for &(px, py) in &points[1..] {
                cr.line_to(px, py);
            }
            cr.stroke()?;
            cr.restore()?;
        }
        Annotation::StepBadge {
            x,
            y,
            number,
            color,
            radius,
        } => {
            let r = radius.max(12.0);
            cr.save()?;
            // Drop shadow
            cr.arc(x + 1.5, y + 2.5, r, 0.0, std::f64::consts::TAU);
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.35);
            cr.fill()?;

            // Colored badge circle + crisp white border
            cr.arc(*x, *y, r, 0.0, std::f64::consts::TAU);
            cr.set_source_rgba(color.r, color.g, color.b, 1.0);
            cr.fill_preserve()?;
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.95);
            cr.set_line_width(2.4);
            cr.stroke()?;

            // Centered number with automatic light/dark contrast
            let luma = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
            if luma > 0.68 {
                cr.set_source_rgba(0.08, 0.08, 0.10, 1.0);
            } else {
                cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
            }
            let text = number.to_string();
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size(r * 1.15);
            let ext = cr.text_extents(&text)?;
            cr.move_to(
                x - (ext.width() * 0.5 + ext.x_bearing()),
                y - (ext.height() * 0.5 + ext.y_bearing()),
            );
            cr.show_text(&text)?;
            cr.restore()?;
        }
        Annotation::Text {
            x,
            y,
            text,
            color,
            fill,
            font_size,
        } => {
            if text.is_empty() {
                return Ok(());
            }
            cr.save()?;
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size(*font_size);
            let line_height = *font_size * 1.25;
            let lines: Vec<&str> = text.lines().collect();
            if fill.a >= 0.05 {
                let mut max_w = 12.0_f64;
                for line in &lines {
                    let ext = cr.text_extents(line)?;
                    max_w = max_w.max(ext.x_advance().max(ext.width()));
                }
                let total_h = (lines.len().max(1) as f64) * line_height;
                draw_rounded_rect(cr, *x - 8.0, *y + 2.0, max_w + 16.0, total_h + 6.0, 6.0);
                cr.set_source_rgba(fill.r, fill.g, fill.b, fill.a);
                cr.fill()?;
            }
            for (idx, line) in lines.iter().enumerate() {
                let ly = *y + *font_size + idx as f64 * line_height;
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.55);
                cr.move_to(*x + 1.5, ly + 1.5);
                cr.show_text(line)?;
                cr.set_source_rgba(color.r, color.g, color.b, color.a);
                cr.move_to(*x, ly);
                cr.show_text(line)?;
            }
            cr.restore()?;
        }
        Annotation::SpeechBubble {
            x,
            y,
            w,
            h,
            tail_x,
            tail_y,
            text,
            fill,
            border,
            text_color,
            font_size,
        } => {
            let (nx, ny, nw, nh) = normalize_rect(*x, *y, *w, *h);
            let bw = nw.max(48.0);
            let bh = nh.max(32.0);
            cr.save()?;

            if fill.a >= 0.05 {
                draw_bubble_path(cr, nx + 2.0, ny + 3.0, bw, bh, *tail_x + 2.0, *tail_y + 3.0);
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.25);
                cr.fill()?;

                draw_bubble_path(cr, nx, ny, bw, bh, *tail_x, *tail_y);
                cr.set_source_rgba(fill.r, fill.g, fill.b, fill.a);
                cr.fill_preserve()?;
            } else {
                // Transparent bubble: subtle dark outer halo so border stays crisp over light UI
                draw_bubble_path(cr, nx, ny, bw, bh, *tail_x, *tail_y);
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.35);
                cr.set_line_width(4.2);
                cr.stroke_preserve()?;
            }
            cr.set_source_rgba(border.r, border.g, border.b, border.a);
            cr.set_line_width(2.5);
            cr.stroke()?;

            if !text.is_empty() {
                cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
                cr.set_font_size(*font_size);
                let pad = 10.0;
                let max_text_w = (bw - pad * 2.0).max(20.0);
                let lines = wrap_text(cr, text, max_text_w)?;
                let line_h = *font_size * 1.25;
                let total_h = lines.len() as f64 * line_h;
                let start_y = ny + ((bh - total_h) / 2.0).max(pad) + *font_size * 0.85;
                let draw_shadow = fill.a < 0.05 || (text_color.r > 0.7 && text_color.g > 0.7);
                for (idx, line) in lines.iter().enumerate() {
                    let ly = start_y + idx as f64 * line_h;
                    if ly > ny + bh - 4.0 {
                        break;
                    }
                    if draw_shadow {
                        cr.set_source_rgba(0.0, 0.0, 0.0, 0.65);
                        cr.move_to(nx + pad + 1.2, ly + 1.2);
                        cr.show_text(line)?;
                    }
                    cr.set_source_rgba(text_color.r, text_color.g, text_color.b, text_color.a);
                    cr.move_to(nx + pad, ly);
                    cr.show_text(line)?;
                }
            }
            cr.restore()?;
        }
        Annotation::Blur { x, y, w, h, .. } => {
            if is_preview {
                let (nx, ny, nw, nh) = normalize_rect(*x, *y, *w, *h);
                if nw >= 1.0 && nh >= 1.0 {
                    cr.save()?;
                    cr.set_source_rgba(0.21, 0.52, 0.90, 0.9);
                    cr.set_line_width(2.0);
                    cr.set_dash(&[6.0, 4.0], 0.0);
                    cr.rectangle(nx, ny, nw, nh);
                    cr.stroke()?;
                    cr.restore()?;
                }
            }
        }
        Annotation::Spotlight { x, y, w, h } => {
            let (nx, ny, nw, nh) = normalize_rect(*x, *y, *w, *h);
            if nw < 2.0 || nh < 2.0 {
                return Ok(());
            }
            cr.save()?;
            cr.set_fill_rule(cairo::FillRule::EvenOdd);
            cr.rectangle(0.0, 0.0, canvas_w, canvas_h);
            draw_rounded_rect(cr, nx, ny, nw, nh, 6.0);
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.52);
            cr.fill()?;

            cr.set_source_rgba(1.0, 1.0, 1.0, 0.85);
            cr.set_line_width(2.0);
            draw_rounded_rect(cr, nx, ny, nw, nh, 6.0);
            cr.stroke()?;
            cr.restore()?;
        }
    }
    Ok(())
}

pub fn draw_rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    let deg = std::f64::consts::PI / 180.0;
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -90.0 * deg, 0.0 * deg);
    cr.arc(x + w - r, y + h - r, r, 0.0 * deg, 90.0 * deg);
    cr.arc(x + r, y + h - r, r, 90.0 * deg, 180.0 * deg);
    cr.arc(x + r, y + r, r, 180.0 * deg, 270.0 * deg);
    cr.close_path();
}

fn draw_bubble_path(
    cr: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    tail_x: f64,
    tail_y: f64,
) {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum TailSide {
        None,
        Top,
        Right,
        Bottom,
        Left,
    }

    let r = 12.0_f64.min(w / 3.0).min(h / 3.0);
    let deg = std::f64::consts::PI / 180.0;
    let outside = tail_x < x || tail_x > x + w || tail_y < y || tail_y > y + h;
    let side = if !outside {
        TailSide::None
    } else {
        let cx = x + w * 0.5;
        let cy = y + h * 0.5;
        let dx = tail_x - cx;
        let dy = tail_y - cy;
        let nx = dx / (w * 0.5).max(1.0);
        let ny = dy / (h * 0.5).max(1.0);
        if ny.abs() >= nx.abs() {
            if dy >= 0.0 {
                TailSide::Bottom
            } else {
                TailSide::Top
            }
        } else if dx >= 0.0 {
            TailSide::Right
        } else {
            TailSide::Left
        }
    };

    let h_span = 18.0_f64.min((w - r * 2.0).max(4.0));
    let h_left = (tail_x - h_span * 0.5).clamp(x + r, (x + w - r - h_span).max(x + r));
    let h_right = h_left + h_span;

    let v_span = 16.0_f64.min((h - r * 2.0).max(4.0));
    let v_top = (tail_y - v_span * 0.5).clamp(y + r, (y + h - r - v_span).max(y + r));
    let v_bottom = v_top + v_span;

    cr.new_sub_path();
    cr.move_to(x + r, y);
    if side == TailSide::Top {
        cr.line_to(h_left, y);
        cr.line_to(tail_x, tail_y);
        cr.line_to(h_right, y);
    }
    cr.arc(x + w - r, y + r, r, -90.0 * deg, 0.0 * deg);
    if side == TailSide::Right {
        cr.line_to(x + w, v_top);
        cr.line_to(tail_x, tail_y);
        cr.line_to(x + w, v_bottom);
    }
    cr.arc(x + w - r, y + h - r, r, 0.0 * deg, 90.0 * deg);
    if side == TailSide::Bottom {
        cr.line_to(h_right, y + h);
        cr.line_to(tail_x, tail_y);
        cr.line_to(h_left, y + h);
    }
    cr.arc(x + r, y + h - r, r, 90.0 * deg, 180.0 * deg);
    if side == TailSide::Left {
        cr.line_to(x, v_bottom);
        cr.line_to(tail_x, tail_y);
        cr.line_to(x, v_top);
    }
    cr.arc(x + r, y + r, r, 180.0 * deg, 270.0 * deg);
    cr.close_path();
}

fn wrap_text(cr: &cairo::Context, text: &str, max_width: f64) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            let ext = cr.text_extents(&candidate)?;
            if ext.width() > max_width && !current.is_empty() {
                lines.push(current);
                current = word.to_string();
            } else {
                current = candidate;
            }
        }
        if !current.is_empty() || paragraph.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cairo_surface_roundtrip_preserves_pixels() {
        let mut img = RgbaImage::new(4, 4);
        img.put_pixel(0, 0, Rgba([220, 40, 20, 255]));
        img.put_pixel(1, 1, Rgba([10, 180, 90, 255]));
        img.put_pixel(2, 2, Rgba([50, 100, 200, 128]));

        let mut surface = rgba_to_cairo_surface(&img).unwrap();
        let back = cairo_surface_to_rgba(&mut surface).unwrap();

        assert_eq!(back.get_pixel(0, 0), img.get_pixel(0, 0));
        assert_eq!(back.get_pixel(1, 1), img.get_pixel(1, 1));
        let p2 = back.get_pixel(2, 2).0;
        assert!((p2[0] as i32 - 50).abs() <= 2);
        assert!((p2[1] as i32 - 100).abs() <= 2);
        assert!((p2[2] as i32 - 200).abs() <= 2);
        assert_eq!(p2[3], 128);
    }

    #[test]
    fn pixelate_and_render_annotations_modify_output() {
        let mut base = RgbaImage::from_pixel(160, 120, Rgba([240, 240, 240, 255]));
        for y in 10..20 {
            for x in 10..20 {
                base.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }

        let anns = vec![
            Annotation::Blur {
                x: 10.0,
                y: 10.0,
                w: 32.0,
                h: 32.0,
                block_size: 16,
            },
            Annotation::Rectangle {
                x: 50.0,
                y: 20.0,
                w: 40.0,
                h: 30.0,
                color: Color::RED,
                fill: Color::TRANSPARENT,
                line_width: 4.0,
            },
            Annotation::Arrow {
                x1: 20.0,
                y1: 90.0,
                x2: 110.0,
                y2: 90.0,
                color: Color::BLUE,
                line_width: 4.0,
            },
            Annotation::Ellipse {
                x: 100.0,
                y: 15.0,
                w: 40.0,
                h: 24.0,
                color: Color::GREEN,
                fill: Color::TRANSPARENT,
                line_width: 3.0,
            },
            Annotation::Highlighter {
                x1: 15.0,
                y1: 105.0,
                x2: 120.0,
                y2: 105.0,
                color: Color::YELLOW,
                width: 16.0,
            },
            Annotation::Freehand {
                points: vec![(12.0, 50.0), (28.0, 58.0), (44.0, 52.0)],
                color: Color::ORANGE,
                line_width: 3.0,
            },
            Annotation::StepBadge {
                x: 135.0,
                y: 65.0,
                number: 1,
                color: Color::RED,
                radius: 14.0,
            },
            Annotation::SpeechBubble {
                x: 20.0,
                y: 50.0,
                w: 80.0,
                h: 36.0,
                tail_x: 35.0,
                tail_y: 95.0,
                text: "Test Blase".to_string(),
                fill: Color::YELLOW,
                border: Color::BLACK,
                text_color: Color::BLACK,
                font_size: 13.0,
            },
            Annotation::Text {
                x: 12.0,
                y: 8.0,
                text: "Hinweis".to_string(),
                color: Color::BLUE,
                fill: Color::TRANSPARENT,
                font_size: 14.0,
            },
        ];

        let rendered = render_final_image(&base, &anns).unwrap();
        assert_eq!(rendered.dimensions(), (160, 120));
        let p = rendered.get_pixel(12, 12).0;
        assert!(p[1] > 50 && p[2] > 50);
        let border_px = rendered.get_pixel(70, 20).0;
        assert!(border_px[0] > 180 && border_px[1] < 100);
    }

    #[test]
    fn spotlight_and_beautify_produce_expected_dimensions() {
        let base = RgbaImage::from_pixel(100, 80, Rgba([255, 255, 255, 255]));
        let anns = vec![Annotation::Spotlight {
            x: 25.0,
            y: 20.0,
            w: 40.0,
            h: 30.0,
        }];
        let rendered =
            render_final_image_with_beautify(&base, &anns, BeautifyStyle::GnomeBlue).unwrap();
        let pad = BeautifyStyle::GnomeBlue.padding() as u32 * 2;
        assert_eq!(rendered.dimensions(), (100 + pad, 80 + pad));
        // Outside spotlight inside screenshot should be darkened
        let dim_px = rendered.get_pixel(40, 40).0;
        assert!(dim_px[0] < 180 && dim_px[1] < 180);
        // Inside spotlight center should stay bright white
        let bright_px = rendered.get_pixel(36 + 45, 36 + 35).0;
        assert_eq!(bright_px, [255, 255, 255, 255]);
    }

    #[test]
    fn undo_redo_and_step_counter() {
        let mut stack = AnnotationStack::default();
        assert_eq!(stack.next_step_number(), 1);
        stack.push(Annotation::StepBadge {
            x: 10.0,
            y: 10.0,
            number: 1,
            color: Color::RED,
            radius: 14.0,
        });
        assert_eq!(stack.next_step_number(), 2);
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
        assert!(stack.undo());
        assert!(stack.can_redo());
        assert_eq!(stack.next_step_number(), 1);
        assert!(stack.redo());
        assert!(!stack.can_redo());
        assert_eq!(stack.next_step_number(), 2);
    }

    #[test]
    fn hit_test_translate_rotate_and_flip() {
        let mut stack = AnnotationStack::default();
        stack.push(Annotation::Rectangle {
            x: 10.0,
            y: 20.0,
            w: 40.0,
            h: 30.0,
            color: Color::RED,
            fill: Color::TRANSPARENT,
            line_width: 3.0,
        });
        stack.push(Annotation::StepBadge {
            x: 25.0,
            y: 35.0,
            number: 1,
            color: Color::BLUE,
            radius: 14.0,
        });

        // StepBadge sits on top of Rectangle at (25, 35)
        assert_eq!(stack.hit_test_topmost(25.0, 35.0), Some(1));
        assert_eq!(stack.hit_test_topmost(48.0, 48.0), Some(0));
        assert_eq!(stack.hit_test_topmost(150.0, 150.0), None);

        let mut rect = stack.items[0].clone();
        rect.translate(5.0, -5.0);
        assert_eq!(rect.bounding_box(), (15.0, 15.0, 40.0, 30.0));

        // Rotate 90° CW on 100x80 image -> old_h = 80
        rect.rotate_90_cw(80.0);
        assert_eq!(rect.bounding_box(), (35.0, 15.0, 30.0, 40.0));

        // Flip horizontally on width = 80
        rect.flip_horizontal(80.0);
        assert_eq!(rect.bounding_box(), (15.0, 15.0, 30.0, 40.0));

        assert!(stack.remove_at(1).is_some());
        assert_eq!(stack.items.len(), 1);
    }

    #[test]
    fn live_property_updates_and_four_sided_bubble_tail() {
        assert_eq!(Color::PALETTE.len(), 9);
        assert!(Color::PALETTE.iter().any(|(_, c)| *c == Color::PURPLE));
        assert!(Color::PALETTE.iter().any(|(_, c)| *c == Color::GRAY));
        assert_eq!(Color::FILL_PRESETS.len(), 10);
        assert!(Color::FILL_PRESETS.iter().any(|(_, c)| *c == Color::PURPLE));
        assert!(Color::FILL_PRESETS.iter().any(|(_, c)| *c == Color::GRAY));

        let mut text = Annotation::Text {
            x: 10.0,
            y: 10.0,
            text: "Hallo".into(),
            color: Color::RED,
            fill: Color::TRANSPARENT,
            font_size: 16.0,
        };
        assert!(text.set_color(Color::GREEN));
        assert!(text.set_size(28.0));
        assert_eq!(text.current_size(), Some(28.0));
        assert!(text.set_fill(Color::PURPLE));
        assert_eq!(text.current_fill(), Some(Color::PURPLE));
        assert_eq!(text.current_text(), Some("Hallo"));
        assert!(text.set_text("Neu".into()));
        assert_eq!(text.current_text(), Some("Neu"));

        let mut rect = Annotation::Rectangle {
            x: 10.0,
            y: 10.0,
            w: 60.0,
            h: 40.0,
            color: Color::BLACK,
            fill: Color::PURPLE,
            line_width: 2.0,
        };
        assert_eq!(rect.current_fill(), Some(Color::PURPLE));
        let rect_base = RgbaImage::from_pixel(100, 80, Rgba([255, 255, 255, 255]));
        let rect_rendered = render_final_image(&rect_base, &[rect.clone()]).unwrap();
        let interior = rect_rendered.get_pixel(40, 30).0;
        assert!(interior[0] > 100 && interior[2] > 140 && interior[1] < 100);
        assert!(rect.set_fill(Color::GRAY));
        assert_eq!(rect.current_fill(), Some(Color::GRAY));

        let mut blur = Annotation::Blur {
            x: 0.0,
            y: 0.0,
            w: 20.0,
            h: 20.0,
            block_size: 12,
        };
        assert!(!blur.set_color(Color::BLUE));
        assert!(!blur.set_fill(Color::YELLOW));
        assert!(blur.set_blur_block_size(24));
        assert_eq!(blur.current_blur_block_size(), Some(24));

        // Speech bubble with default transparent fill keeps background intact
        let base = RgbaImage::from_pixel(180, 140, Rgba([220, 220, 220, 255]));
        let mut bubble = Annotation::SpeechBubble {
            x: 50.0,
            y: 50.0,
            w: 80.0,
            h: 40.0,
            tail_x: 160.0,
            tail_y: 70.0,
            text: String::new(),
            fill: Color::TRANSPARENT,
            border: Color::BLUE,
            text_color: Color::WHITE,
            font_size: 14.0,
        };
        assert!(bubble.hit_test(158.0, 70.0));
        assert_eq!(bubble.current_fill(), Some(Color::TRANSPARENT));
        let rendered = render_final_image(&base, &[bubble.clone()]).unwrap();
        assert_eq!(rendered.dimensions(), (180, 140));
        assert_eq!(rendered.get_pixel(90, 70).0, [220, 220, 220, 255]);

        assert!(bubble.set_fill(Color::YELLOW));
        assert_eq!(bubble.current_fill(), Some(Color::YELLOW));
        assert_eq!(bubble.current_text(), Some(""));
        assert!(bubble.set_text("Blase Neu".into()));
        assert_eq!(bubble.current_text(), Some("Blase Neu"));
        assert!(!blur.set_text("Nope".into()));
    }
}
