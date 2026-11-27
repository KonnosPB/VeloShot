//! Native Wayland clipboard integration via GDK4 textures.

use std::io::Cursor;

use anyhow::{Context, Result, anyhow};
use gtk4::{gdk, glib, prelude::*};
use image::{ImageFormat, RgbaImage};

/// Copies an `RgbaImage` as lossless PNG onto the default Wayland/GDK clipboard.
pub fn copy_rgba_to_clipboard(img: &RgbaImage) -> Result<()> {
    let mut png_bytes = Vec::new();
    img.write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
        .context("PNG-Kodierung für Zwischenablage fehlgeschlagen")?;

    let bytes = glib::Bytes::from_owned(png_bytes);
    let texture = gdk::Texture::from_bytes(&bytes)
        .context("Konnte GdkTexture aus PNG nicht erzeugen")?;

    let display = gdk::Display::default()
        .ok_or_else(|| anyhow!("Kein grafisches Display verfügbar"))?;
    display.clipboard().set_texture(&texture);
    Ok(())
}

/// Copies plain UTF-8 text (e.g. OCR output) onto the default Wayland/GDK clipboard.
pub fn copy_text_to_clipboard(text: &str) -> Result<()> {
    let display = gdk::Display::default()
        .ok_or_else(|| anyhow!("Kein grafisches Display verfügbar"))?;
    display.clipboard().set_text(text);
    Ok(())
}
