//! Embedded 24x24 refined duotone UI SVG icons registered with GtkIconTheme.
//!
//! Styled in a clean Adwaita / Linear-inspired duotone palette: crisp `#e6ebf2`
//! vector linework paired with subtle translucent fills and a single muted accent.

use std::fs;
use std::path::PathBuf;

use gtk4::{IconTheme, gdk};

use crate::config::APP_DIR_NAME;

const HICOLOR_INDEX: &str = "\
[Icon Theme]
Name=Hicolor
Directories=scalable/actions,scalable/apps,symbolic/apps

[scalable/actions]
Size=24
Context=Actions
Type=Scalable
MinSize=16
MaxSize=512

[scalable/apps]
Size=128
Context=Applications
Type=Scalable
MinSize=16
MaxSize=512

[symbolic/apps]
Size=16
Context=Applications
Type=Scalable
MinSize=8
MaxSize=512
";

pub const APP_ICON_SVG: &str =
    include_str!("../../data/icons/hicolor/scalable/apps/io.github.konnospb.VeloShot.svg");
pub const APP_SYMBOLIC_SVG: &str =
    include_str!("../../data/icons/hicolor/symbolic/apps/io.github.konnospb.VeloShot-symbolic.svg");

pub const UI_ICONS: &[(&str, &str)] = &[
    // --- Editor Left Sidebar Tools (12) ---
    (
        "jas-select",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M5.5 3.5L18.5 11.8L12.8 13.7L15.4 19.1L13.3 20.2L10.7 14.8L6.3 18.4Z" fill="#6ea8fe" fill-opacity="0.22" stroke="#e6ebf2" stroke-width="1.75" stroke-linejoin="round"/>
  <circle cx="13.8" cy="12.6" r="1.5" fill="#6ea8fe"/>
</svg>"##,
    ),
    (
        "jas-rect",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <rect x="4" y="5" width="16" height="14" rx="2.5" fill="#6ea8fe" fill-opacity="0.18" stroke="#e6ebf2" stroke-width="1.75"/>
  <rect x="2.7" y="3.7" width="3" height="3" rx="0.8" fill="#6ea8fe"/>
  <rect x="18.3" y="17.3" width="3" height="3" rx="0.8" fill="#6ea8fe"/>
</svg>"##,
    ),
    (
        "jas-arrow",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M5.5 18.5L18 6" stroke="#e6ebf2" stroke-width="1.9" stroke-linecap="round"/>
  <path d="M11.5 5.5H18.5V12.5" fill="none" stroke="#e5a04b" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
  <circle cx="5.5" cy="18.5" r="1.7" fill="#6ea8fe"/>
</svg>"##,
    ),
    (
        "jas-ellipse",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <ellipse cx="12" cy="12" rx="8.8" ry="6.5" fill="#58c49a" fill-opacity="0.18" stroke="#e6ebf2" stroke-width="1.75"/>
  <path d="M15.8 7.5A6.5 4.6 0 0 1 18.8 10.6" fill="none" stroke="#58c49a" stroke-width="1.75" stroke-linecap="round"/>
</svg>"##,
    ),
    (
        "jas-marker",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M4 19.8H20" stroke="#e5a04b" stroke-width="2.4" stroke-linecap="round" stroke-opacity="0.85"/>
  <path d="M14.5 3.8L19.8 9.1L10 18.9H5.2L4.4 14.1Z" fill="#e5a04b" fill-opacity="0.22" stroke="#e6ebf2" stroke-width="1.7" stroke-linejoin="round"/>
  <path d="M12.4 6L17.6 11.2" stroke="#e5a04b" stroke-width="1.5"/>
</svg>"##,
    ),
    (
        "jas-pen",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M4 19.2C7.2 19.8 9.2 17.8 12.2 17.8" fill="none" stroke="#a78bfa" stroke-width="1.75" stroke-linecap="round"/>
  <path d="M16.2 3.8L20.2 7.8L9.6 18.4L4.8 19.5L5.9 14.7Z" fill="#a78bfa" fill-opacity="0.22" stroke="#e6ebf2" stroke-width="1.7" stroke-linejoin="round"/>
</svg>"##,
    ),
    (
        "jas-step",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <circle cx="12" cy="12" r="8.5" fill="#e06c75" fill-opacity="0.24" stroke="#e06c75" stroke-width="1.75"/>
  <path d="M12.8 7.8V16.2H11.1V10.1L9.4 11.1V9.5L11.5 7.8Z" fill="#e6ebf2"/>
</svg>"##,
    ),
    (
        "jas-text",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <rect x="3.5" y="4" width="17" height="16" rx="3" fill="#6ea8fe" fill-opacity="0.16" stroke="#6ea8fe" stroke-opacity="0.55" stroke-width="1.3"/>
  <path d="M7.2 7.5H16.8M12 7.5V17M9.6 17H14.4" stroke="#e6ebf2" stroke-width="1.9" stroke-linecap="round"/>
</svg>"##,
    ),
    (
        "jas-bubble",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M5 4.2H19A2.2 2.2 0 0 1 21.2 6.4V14A2.2 2.2 0 0 1 19 16.2H10.4L6.2 19.8V16.2H5A2.2 2.2 0 0 1 2.8 14V6.4A2.2 2.2 0 0 1 5 4.2Z" fill="#a78bfa" fill-opacity="0.20" stroke="#e6ebf2" stroke-width="1.75" stroke-linejoin="round"/>
  <circle cx="8.6" cy="10.2" r="1.1" fill="#a78bfa"/>
  <circle cx="12" cy="10.2" r="1.1" fill="#e6ebf2"/>
  <circle cx="15.4" cy="10.2" r="1.1" fill="#a78bfa"/>
</svg>"##,
    ),
    (
        "jas-blur",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <rect x="3.8" y="3.8" width="4.8" height="4.8" rx="1" fill="#6ea8fe" fill-opacity="0.75"/>
  <rect x="9.6" y="3.8" width="4.8" height="4.8" rx="1" fill="#e6ebf2" fill-opacity="0.24"/>
  <rect x="15.4" y="3.8" width="4.8" height="4.8" rx="1" fill="#6ea8fe" fill-opacity="0.50"/>
  <rect x="3.8" y="9.6" width="4.8" height="4.8" rx="1" fill="#e6ebf2" fill-opacity="0.24"/>
  <rect x="9.6" y="9.6" width="4.8" height="4.8" rx="1" fill="#e6ebf2" fill-opacity="0.88"/>
  <rect x="15.4" y="9.6" width="4.8" height="4.8" rx="1" fill="#e6ebf2" fill-opacity="0.24"/>
  <rect x="3.8" y="15.4" width="4.8" height="4.8" rx="1" fill="#6ea8fe" fill-opacity="0.50"/>
  <rect x="9.6" y="15.4" width="4.8" height="4.8" rx="1" fill="#e6ebf2" fill-opacity="0.24"/>
  <rect x="15.4" y="15.4" width="4.8" height="4.8" rx="1" fill="#6ea8fe" fill-opacity="0.75"/>
</svg>"##,
    ),
    (
        "jas-spotlight",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <rect x="3.5" y="3.5" width="17" height="17" rx="3" fill="#e6ebf2" fill-opacity="0.10" stroke="#e6ebf2" stroke-opacity="0.45" stroke-width="1.3"/>
  <rect x="7.2" y="7.2" width="9.6" height="9.6" rx="1.8" fill="#e5a04b" fill-opacity="0.22" stroke="#e5a04b" stroke-width="1.75"/>
  <circle cx="12" cy="12" r="1.5" fill="#e6ebf2"/>
</svg>"##,
    ),
    (
        "jas-crop",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M6.8 3.5V16.2A1 1 0 0 0 7.8 17.2H20.5" fill="none" stroke="#e6ebf2" stroke-width="2" stroke-linecap="round"/>
  <path d="M3.5 6.8H16.2A1 1 0 0 1 17.2 7.8V20.5" fill="none" stroke="#58c49a" stroke-width="2" stroke-linecap="round"/>
</svg>"##,
    ),
    // --- Editor HeaderBar & Subbar Icons (11) ---
    (
        "jas-undo",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M9 7L4.5 11.4L9 15.8" fill="none" stroke="#6ea8fe" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
  <path d="M5 11.4H14.2A4.6 4.6 0 0 1 18.8 16A4.6 4.6 0 0 1 14.2 20.6H11.5" fill="none" stroke="#e6ebf2" stroke-width="1.8" stroke-linecap="round"/>
</svg>"##,
    ),
    (
        "jas-redo",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M15 7L19.5 11.4L15 15.8" fill="none" stroke="#6ea8fe" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
  <path d="M19 11.4H9.8A4.6 4.6 0 0 0 5.2 16A4.6 4.6 0 0 0 9.8 20.6H12.5" fill="none" stroke="#e6ebf2" stroke-width="1.8" stroke-linecap="round"/>
</svg>"##,
    ),
    (
        "jas-history",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <rect x="2.5" y="6.5" width="4.6" height="11" rx="1.3" fill="#6ea8fe" fill-opacity="0.24" stroke="#6ea8fe" stroke-opacity="0.7" stroke-width="1.1"/>
  <rect x="16.9" y="6.5" width="4.6" height="11" rx="1.3" fill="#a78bfa" fill-opacity="0.24" stroke="#a78bfa" stroke-opacity="0.7" stroke-width="1.1"/>
  <rect x="7" y="4.2" width="10" height="15.6" rx="2" fill="#6ea8fe" fill-opacity="0.22" stroke="#e6ebf2" stroke-width="1.6"/>
  <circle cx="12" cy="12" r="1.7" fill="#6ea8fe"/>
</svg>"##,
    ),
    (
        "jas-ocr",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M4.2 8.2V4.8A.8.8 0 0 1 5 4H8.4M15.6 4H19A.8.8 0 0 1 19.8 4.8V8.2M19.8 15.8V19.2A.8.8 0 0 1 19 20H15.6M8.4 20H5A.8.8 0 0 1 4.2 19.2V15.8" fill="none" stroke="#e5a04b" stroke-width="1.8" stroke-linecap="round"/>
  <path d="M7.6 9H16.4M7.6 12H16.4M7.6 15H13" stroke="#e6ebf2" stroke-width="1.8" stroke-linecap="round"/>
</svg>"##,
    ),
    (
        "jas-copy",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <rect x="8.5" y="3.8" width="10.8" height="13.2" rx="2" fill="#6ea8fe" fill-opacity="0.18" stroke="#6ea8fe" stroke-width="1.5"/>
  <rect x="4.7" y="7" width="10.8" height="13.2" rx="2" fill="#232732" fill-opacity="0.9" stroke="#e6ebf2" stroke-width="1.6"/>
  <path d="M7.6 11.4H12.6M7.6 14.4H11" stroke="#e6ebf2" stroke-width="1.5" stroke-linecap="round"/>
</svg>"##,
    ),
    (
        "jas-save",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M12 4.2V14.5" stroke="#58c49a" stroke-width="2" stroke-linecap="round"/>
  <path d="M8 10.8L12 14.8L16 10.8" fill="none" stroke="#58c49a" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
  <path d="M4.8 16.5V18.3A1.5 1.5 0 0 0 6.3 19.8H17.7A1.5 1.5 0 0 0 19.2 18.3V16.5" fill="none" stroke="#e6ebf2" stroke-width="1.8" stroke-linecap="round"/>
</svg>"##,
    ),
    (
        "jas-stroke-width",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <rect x="3.8" y="5.2" width="16.4" height="1.8" rx="0.9" fill="#e6ebf2" fill-opacity="0.65"/>
  <rect x="3.8" y="10.2" width="16.4" height="2.8" rx="1.4" fill="#6ea8fe" fill-opacity="0.85"/>
  <rect x="3.8" y="16" width="16.4" height="4" rx="1.8" fill="#e6ebf2"/>
</svg>"##,
    ),
    (
        "jas-fill",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M9.6 3.8L18 12.2L12.6 17.6A1.9 1.9 0 0 1 9.9 17.6L5 12.7A1.9 1.9 0 0 1 5 10Z" fill="#6ea8fe" fill-opacity="0.22" stroke="#e6ebf2" stroke-width="1.65" stroke-linejoin="round"/>
  <path d="M6.4 12.2L11.2 17L16.1 12.2Z" fill="#6ea8fe" fill-opacity="0.75"/>
  <path d="M19 14C19 14 21.3 16.7 21.3 18.3A2.3 2.3 0 1 1 16.7 18.3C16.7 16.7 19 14 19 14Z" fill="#6ea8fe"/>
</svg>"##,
    ),
    (
        "jas-frame",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <rect x="3" y="3.8" width="18" height="16.4" rx="2.8" fill="#a78bfa" fill-opacity="0.26" stroke="#a78bfa" stroke-width="1.4"/>
  <rect x="6.2" y="7" width="11.6" height="10" rx="1.5" fill="#e6ebf2" fill-opacity="0.88"/>
</svg>"##,
    ),
    (
        "jas-rotate",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M19.2 11.5A7.2 7.2 0 1 0 17 17" fill="none" stroke="#e6ebf2" stroke-width="1.8" stroke-linecap="round"/>
  <path d="M19.2 6.8V11.8H14.2" fill="none" stroke="#6ea8fe" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/>
</svg>"##,
    ),
    (
        "jas-flip",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M12 4V20" stroke="#e6ebf2" stroke-opacity="0.7" stroke-width="1.6" stroke-linecap="round" stroke-dasharray="2.4 2.4"/>
  <path d="M9 7L4 12L9 17Z" fill="#6ea8fe" fill-opacity="0.30" stroke="#6ea8fe" stroke-width="1.5" stroke-linejoin="round"/>
  <path d="M15 7L20 12L15 17Z" fill="#e6ebf2" fill-opacity="0.22" stroke="#e6ebf2" stroke-width="1.5" stroke-linejoin="round"/>
</svg>"##,
    ),
    // --- Capture Mini-Bar Icons (6) ---
    (
        "jas-capture-region",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M4.2 8.2V4.8A.8.8 0 0 1 5 4H8.4M15.6 4H19A.8.8 0 0 1 19.8 4.8V8.2M19.8 15.8V19.2A.8.8 0 0 1 19 20H15.6M8.4 20H5A.8.8 0 0 1 4.2 19.2V15.8" fill="none" stroke="#e6ebf2" stroke-width="1.85" stroke-linecap="round"/>
  <path d="M12 8.5V15.5M8.5 12H15.5" stroke="#6ea8fe" stroke-width="1.9" stroke-linecap="round"/>
</svg>"##,
    ),
    (
        "jas-capture-monitor",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <rect x="3" y="4.2" width="18" height="12" rx="2" fill="#6ea8fe" fill-opacity="0.20" stroke="#e6ebf2" stroke-width="1.65"/>
  <path d="M12 16.2V19.4M8.2 19.6H15.8" stroke="#e6ebf2" stroke-width="1.75" stroke-linecap="round"/>
  <circle cx="12" cy="10.2" r="1.6" fill="#6ea8fe"/>
</svg>"##,
    ),
    (
        "jas-capture-all-monitors",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <rect x="2" y="5" width="9" height="10" rx="1.5" fill="#6ea8fe" fill-opacity="0.20" stroke="#e6ebf2" stroke-width="1.5"/>
  <rect x="13" y="5" width="9" height="10" rx="1.5" fill="#a78bfa" fill-opacity="0.20" stroke="#e6ebf2" stroke-width="1.5"/>
  <path d="M4.8 18.6H19.2" stroke="#6ea8fe" stroke-width="1.8" stroke-linecap="round"/>
</svg>"##,
    ),
    (
        "jas-capture-window",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <rect x="3.2" y="4.2" width="17.6" height="15.6" rx="2.2" fill="#6ea8fe" fill-opacity="0.16" stroke="#e6ebf2" stroke-width="1.65"/>
  <path d="M3.5 8.6H20.5" stroke="#6ea8fe" stroke-width="1.5"/>
  <circle cx="6.2" cy="6.4" r="0.9" fill="#e6ebf2"/>
  <circle cx="9" cy="6.4" r="0.9" fill="#6ea8fe"/>
</svg>"##,
    ),
    (
        "jas-timer",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M9.6 3H14.4M12 3V5.8" stroke="#e6ebf2" stroke-width="1.75" stroke-linecap="round"/>
  <circle cx="12" cy="13.5" r="7.2" fill="#6ea8fe" fill-opacity="0.18" stroke="#e6ebf2" stroke-width="1.7"/>
  <path d="M12 9.5V13.6L15 15.4" fill="none" stroke="#e5a04b" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/>
</svg>"##,
    ),
    (
        "jas-flip-vertical",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M12 4.8V19.2" stroke="#6ea8fe" stroke-width="1.8" stroke-linecap="round"/>
  <path d="M8.4 8.2L12 4.6L15.6 8.2" fill="none" stroke="#e6ebf2" stroke-width="1.85" stroke-linecap="round" stroke-linejoin="round"/>
  <path d="M8.4 15.8L12 19.4L15.6 15.8" fill="none" stroke="#e6ebf2" stroke-width="1.85" stroke-linecap="round" stroke-linejoin="round"/>
</svg>"##,
    ),
    // --- Carousel & Hub Icons (3) ---
    (
        "jas-edit",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M4.8 20H19.2" stroke="#6ea8fe" stroke-opacity="0.7" stroke-width="1.75" stroke-linecap="round"/>
  <path d="M15.6 4L19.8 8.2L9.8 18.2H5.2L5.2 13.6Z" fill="#6ea8fe" fill-opacity="0.22" stroke="#e6ebf2" stroke-width="1.65" stroke-linejoin="round"/>
  <path d="M13.6 6L17.8 10.2" stroke="#6ea8fe" stroke-width="1.5"/>
</svg>"##,
    ),
    (
        "jas-delete",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M9.2 4.5V3.6A.9.9 0 0 1 10.1 2.7H13.9A.9.9 0 0 1 14.8 3.6V4.5M4.8 6.2H19.2" fill="none" stroke="#e6ebf2" stroke-width="1.75" stroke-linecap="round"/>
  <path d="M6.6 6.2H17.4L16.5 19.2A1.7 1.7 0 0 1 14.8 20.8H9.2A1.7 1.7 0 0 1 7.5 19.2Z" fill="#e06c75" fill-opacity="0.24" stroke="#e06c75" stroke-width="1.65" stroke-linejoin="round"/>
  <path d="M10.2 10V16.6M13.8 10V16.6" stroke="#e6ebf2" stroke-width="1.5" stroke-linecap="round"/>
</svg>"##,
    ),
    (
        "jas-settings",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path d="M12 2.8L13.5 4.9L16 4.2L16.5 6.8L19 7.6L18.1 10L20.1 12L18.1 14L19 16.4L16.5 17.2L16 19.8L13.5 19.1L12 21.2L10.5 19.1L8 19.8L7.5 17.2L5 16.4L5.9 14L3.9 12L5.9 10L5 7.6L7.5 6.8L8 4.2L10.5 4.9Z" fill="#6ea8fe" fill-opacity="0.20" stroke="#e6ebf2" stroke-width="1.55" stroke-linejoin="round"/>
  <circle cx="12" cy="12" r="2.8" fill="#232732" stroke="#6ea8fe" stroke-width="1.6"/>
</svg>"##,
    ),
    (
        "jas-apply",
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <circle cx="12" cy="12" r="8.6" fill="#ffffff" fill-opacity="0.18" stroke="#ffffff" stroke-opacity="0.82" stroke-width="1.6"/>
  <path d="M8.3 12.2L10.9 14.8L15.9 9.4" fill="none" stroke="#ffffff" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
</svg>"##,
    ),
];

/// Creates a crisp `gtk4::Image` sized to `pixel_size` for a registered icon.
pub fn icon_image(name: &str, pixel_size: i32) -> gtk4::Image {
    gtk4::Image::builder()
        .icon_name(name)
        .pixel_size(pixel_size)
        .build()
}

/// Writes the embedded icons into the XDG cache icon theme and registers it with GTK4.
pub fn register_icons() {
    let base_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(APP_DIR_NAME)
        .join("icons");
    let actions_dir = base_dir.join("hicolor").join("scalable").join("actions");
    let apps_dir = base_dir.join("hicolor").join("scalable").join("apps");
    let sym_apps_dir = base_dir.join("hicolor").join("symbolic").join("apps");

    if fs::create_dir_all(&actions_dir).is_ok()
        && fs::create_dir_all(&apps_dir).is_ok()
        && fs::create_dir_all(&sym_apps_dir).is_ok()
    {
        let index_path = base_dir.join("hicolor").join("index.theme");
        if fs::read_to_string(&index_path).ok().as_deref() != Some(HICOLOR_INDEX) {
            let _ = fs::write(&index_path, HICOLOR_INDEX);
        }
        for (name, svg) in UI_ICONS {
            let path = actions_dir.join(format!("{name}.svg"));
            if fs::read_to_string(&path).ok().as_deref() != Some(*svg) {
                let _ = fs::write(&path, svg);
            }
        }
        let app_icon_path = apps_dir.join(format!("{}.svg", crate::config::APP_ID));
        if fs::read_to_string(&app_icon_path).ok().as_deref() != Some(APP_ICON_SVG) {
            let _ = fs::write(&app_icon_path, APP_ICON_SVG);
        }
        let app_sym_path = sym_apps_dir.join(format!("{}-symbolic.svg", crate::config::APP_ID));
        if fs::read_to_string(&app_sym_path).ok().as_deref() != Some(APP_SYMBOLIC_SVG) {
            let _ = fs::write(&app_sym_path, APP_SYMBOLIC_SVG);
        }
    }

    if let Some(display) = gdk::Display::default() {
        IconTheme::for_display(&display).add_search_path(&base_dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_ui_icons_are_valid_full_color_24x24_svgs() {
        assert_eq!(UI_ICONS.len(), 33);
        for (name, svg) in UI_ICONS {
            assert!(
                !name.ends_with("-symbolic"),
                "{name} must NOT end with -symbolic so GTK4 keeps full colors"
            );
            assert!(svg.contains("<svg"), "{name} missing <svg");
            assert!(
                svg.contains("viewBox=\"0 0 24 24\""),
                "{name} must use 24x24 viewBox"
            );
            assert!(svg.contains('#'), "{name} must contain color hex values");
            assert!(svg.contains("</svg>"), "{name} missing </svg>");
        }
    }

    #[test]
    fn app_icons_are_valid_svgs() {
        assert!(APP_ICON_SVG.contains("viewBox=\"0 0 128 128\""));
        assert!(APP_ICON_SVG.contains("</svg>"));
        assert!(APP_SYMBOLIC_SVG.contains("viewBox=\"0 0 16 16\""));
        assert!(APP_SYMBOLIC_SVG.contains("</svg>"));
    }
}
