//! Wayland screenshot capture via xdg-desktop-portal (`ashpd`) and region cropping.

use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, anyhow};
use ashpd::desktop::screenshot::Screenshot;
use image::codecs::jpeg::JpegEncoder;
use image::{ImageFormat, RgbImage, RgbaImage};
use tokio::runtime::Runtime;

use crate::config::ExportFormat;

pub(crate) fn tokio_runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Tokio-Runtime konnte nicht gestartet werden")
    })
}

fn should_cleanup_portal_file(
    path: &Path,
    picture_dir: Option<&Path>,
    temp_dir: &Path,
    started_at: SystemTime,
) -> bool {
    if path.starts_with(temp_dir) {
        return true;
    }
    let Some(pic_dir) = picture_dir else {
        return false;
    };
    if path.parent() != Some(pic_dir) {
        return false;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if !name.starts_with("Screenshot from ") || !name.ends_with(".png") {
        return false;
    }
    let threshold = started_at
        .checked_sub(Duration::from_secs(2))
        .unwrap_or(started_at);
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .is_ok_and(|mtime| mtime >= threshold)
}

async fn load_portal_response(
    response: ashpd::desktop::screenshot::Screenshot,
    started_at: SystemTime,
) -> Result<RgbaImage> {
    let path = response.uri().to_file_path().map_err(|_| {
        anyhow!(
            "Ungültige Datei-URI vom Screenshot-Portal: {}",
            response.uri()
        )
    })?;

    let dyn_img = image::open(&path)
        .with_context(|| format!("Konnte Portal-Screenshot nicht öffnen: {}", path.display()))?;
    if should_cleanup_portal_file(
        &path,
        dirs::picture_dir().as_deref(),
        &std::env::temp_dir(),
        started_at,
    ) {
        let _ = fs::remove_file(&path);
    }
    Ok(dyn_img.to_rgba8())
}

/// Structured error returned by non-interactive fullscreen portal capture.
#[derive(Debug)]
pub enum CaptureError {
    /// The user explicitly cancelled the capture request.
    Cancelled,
    /// The portal rejected the non-interactive screenshot (commonly because the app
    /// has not yet been granted permission and currently has no focused window).
    PermissionRequired(String),
    /// Any other portal or image I/O failure.
    Other(anyhow::Error),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureError::Cancelled => write!(f, "Aufnahme abgebrochen"),
            CaptureError::PermissionRequired(msg) => {
                write!(f, "Screenshot-Berechtigung erforderlich: {msg}")
            }
            CaptureError::Other(err) => write!(f, "{err}"),
        }
    }
}

/// Captures a fullscreen screenshot silently using `xdg-desktop-portal`.
///
/// Never falls back to `interactive(true)` (which would launch the desktop's own
/// screenshot UI); instead returns [`CaptureError::PermissionRequired`] so the UI
/// can briefly focus a VeloShot window and let GNOME show the permission dialog.
pub async fn capture_fullscreen_async() -> std::result::Result<RgbaImage, CaptureError> {
    let started_at = SystemTime::now();
    let req = Screenshot::request()
        .interactive(false)
        .modal(true)
        .send()
        .await
        .map_err(|err| match err {
            ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled) => {
                CaptureError::Cancelled
            }
            other => CaptureError::PermissionRequired(other.to_string()),
        })?;

    let response = req.response().map_err(|err| match err {
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled) => CaptureError::Cancelled,
        other => CaptureError::PermissionRequired(other.to_string()),
    })?;

    load_portal_response(response, started_at)
        .await
        .map_err(CaptureError::Other)
}

/// Captures a window or screen using the interactive GNOME portal dialog.
pub async fn capture_interactive_async() -> Result<RgbaImage> {
    let started_at = SystemTime::now();
    let response = Screenshot::request()
        .interactive(true)
        .modal(true)
        .send()
        .await
        .context("Interaktiver Portal-Screenshot fehlgeschlagen")?
        .response()
        .map_err(|err| {
            if matches!(
                err,
                ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)
            ) {
                anyhow!("Aufnahme abgebrochen")
            } else {
                anyhow!("Keine Screenshot-Antwort vom Portal erhalten: {err}")
            }
        })?;

    load_portal_response(response, started_at).await
}

/// Spawns fullscreen capture on the background Tokio runtime and delivers the result on a channel.
pub fn spawn_capture_async() -> async_channel::Receiver<std::result::Result<RgbaImage, CaptureError>>
{
    let (tx, rx) = async_channel::bounded(1);
    tokio_runtime().spawn(async move {
        let res = capture_fullscreen_async().await;
        let _ = tx.send(res).await;
    });
    rx
}

/// Spawns interactive portal capture (used for Wayland window picking).
pub fn spawn_interactive_capture_async() -> async_channel::Receiver<Result<RgbaImage>> {
    let (tx, rx) = async_channel::bounded(1);
    tokio_runtime().spawn(async move {
        let res = capture_interactive_async().await;
        let _ = tx.send(res).await;
    });
    rx
}

/// Waits `delay_secs` seconds and then captures the fullscreen desktop.
pub fn spawn_delayed_capture_async(
    delay_secs: u64,
) -> async_channel::Receiver<std::result::Result<RgbaImage, CaptureError>> {
    let (tx, rx) = async_channel::bounded(1);
    tokio_runtime().spawn(async move {
        if delay_secs > 0 {
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
        }
        let res = capture_fullscreen_async().await;
        let _ = tx.send(res).await;
    });
    rx
}

/// Crops an `RgbaImage` to `(x, y, w, h)` with strict bounds validation.
pub fn crop_region(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> Result<RgbaImage> {
    if w == 0 || h == 0 {
        return Err(anyhow!("Auswahlbereich darf nicht 0x0 sein"));
    }
    let img_w = img.width();
    let img_h = img.height();
    if x >= img_w || y >= img_h {
        return Err(anyhow!("Auswahl liegt außerhalb des Bildes"));
    }
    let clamped_w = w.min(img_w - x);
    let clamped_h = h.min(img_h - y);
    if clamped_w == 0 || clamped_h == 0 {
        return Err(anyhow!("Auswahlbereich ist leer"));
    }
    Ok(image::imageops::crop_imm(img, x, y, clamped_w, clamped_h).to_image())
}

/// Exports an `RgbaImage` as PNG or JPEG based on file extension, falling back to `default_format`.
///
/// Alpha pixels are composited onto opaque white when encoding as JPEG so semi-transparent
/// regions and rounded Beautify frames render cleanly without encoding errors.
pub fn export_rgba_image(img: &RgbaImage, dest: &Path, default_format: ExportFormat) -> Result<()> {
    let ext = dest
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    let target_format = match ext.as_deref() {
        Some("jpg") | Some("jpeg") => ExportFormat::Jpg,
        Some("png") => ExportFormat::Png,
        _ => default_format,
    };

    match target_format {
        ExportFormat::Png => {
            img.save_with_format(dest, ImageFormat::Png)
                .with_context(|| format!("PNG-Export fehlgeschlagen: {}", dest.display()))?;
        }
        ExportFormat::Jpg => {
            let mut rgb = RgbImage::new(img.width(), img.height());
            for (src, dst) in img.pixels().zip(rgb.pixels_mut()) {
                let a = src[3] as u32;
                let inv_a = 255 - a;
                dst[0] = ((src[0] as u32 * a + 255 * inv_a + 127) / 255) as u8;
                dst[1] = ((src[1] as u32 * a + 255 * inv_a + 127) / 255) as u8;
                dst[2] = ((src[2] as u32 * a + 255 * inv_a + 127) / 255) as u8;
            }
            let file = File::create(dest).with_context(|| {
                format!(
                    "JPEG-Datei konnte nicht erstellt werden: {}",
                    dest.display()
                )
            })?;
            let mut writer = BufWriter::new(file);
            let encoder = JpegEncoder::new_with_quality(&mut writer, 92);
            rgb.write_with_encoder(encoder)
                .with_context(|| format!("JPEG-Export fehlgeschlagen: {}", dest.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn crop_region_extracts_and_clamps_correctly() {
        let mut img = RgbaImage::from_pixel(100, 80, Rgba([10, 20, 30, 255]));
        img.put_pixel(15, 25, Rgba([255, 128, 0, 255]));

        let cropped = crop_region(&img, 10, 20, 30, 40).unwrap();
        assert_eq!(cropped.dimensions(), (30, 40));
        assert_eq!(cropped.get_pixel(5, 5), &Rgba([255, 128, 0, 255]));

        // Clamps when extending past right/bottom edge
        let clamped = crop_region(&img, 90, 70, 50, 50).unwrap();
        assert_eq!(clamped.dimensions(), (10, 10));

        assert!(crop_region(&img, 0, 0, 0, 10).is_err());
        assert!(crop_region(&img, 100, 0, 10, 10).is_err());
    }

    #[test]
    fn export_rgba_image_png_and_jpeg_flattens_alpha() {
        let dir = std::env::temp_dir().join(format!("veloshot-export-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();

        let mut img = RgbaImage::from_pixel(16, 16, Rgba([0, 0, 0, 0]));
        img.put_pixel(4, 4, Rgba([20, 120, 220, 255]));

        let png_path = dir.join("out.png");
        export_rgba_image(&img, &png_path, ExportFormat::Jpg).unwrap();
        assert_eq!(
            image::guess_format(&fs::read(&png_path).unwrap()).unwrap(),
            ImageFormat::Png
        );

        let jpg_path = dir.join("out.jpg");
        export_rgba_image(&img, &jpg_path, ExportFormat::Png).unwrap();
        let jpg_bytes = fs::read(&jpg_path).unwrap();
        assert_eq!(image::guess_format(&jpg_bytes).unwrap(), ImageFormat::Jpeg);
        let decoded_jpg = image::open(&jpg_path).unwrap().to_rgb8();
        // Transparent background pixel should composite onto white (>= 250)
        let bg = decoded_jpg.get_pixel(0, 0);
        assert!(bg[0] >= 250 && bg[1] >= 250 && bg[2] >= 250);

        // Extension-less path falls back to configured default_format
        let fallback_jpg = dir.join("no_ext");
        export_rgba_image(&img, &fallback_jpg, ExportFormat::Jpg).unwrap();
        assert_eq!(
            image::guess_format(&fs::read(&fallback_jpg).unwrap()).unwrap(),
            ImageFormat::Jpeg
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn portal_cleanup_only_deletes_fresh_portal_files() {
        let root = std::env::temp_dir().join(format!("veloshot-portal-{}", uuid::Uuid::new_v4()));
        let fake_temp = root.join("tmp");
        let fake_pics = root.join("Pictures");
        let nested_shots = fake_pics.join("Screenshots");
        fs::create_dir_all(&fake_temp).unwrap();
        fs::create_dir_all(&nested_shots).unwrap();

        let temp_file = fake_temp.join("portal.png");
        let fresh_portal = fake_pics.join("Screenshot from 2026-11-27 22-55-00.png");
        let nested_file = nested_shots.join("user-note.png");
        let user_file = fake_pics.join("Vacation Screenshot.png");

        for f in [&temp_file, &fresh_portal, &nested_file, &user_file] {
            fs::write(f, b"png").unwrap();
        }

        let now = SystemTime::now();
        assert!(should_cleanup_portal_file(
            &temp_file,
            Some(&fake_pics),
            &fake_temp,
            now
        ));
        assert!(should_cleanup_portal_file(
            &fresh_portal,
            Some(&fake_pics),
            &fake_temp,
            now
        ));
        // Pre-existing portal screenshot older than capture start must NOT be removed
        let later_capture = now + Duration::from_secs(60);
        assert!(!should_cleanup_portal_file(
            &fresh_portal,
            Some(&fake_pics),
            &fake_temp,
            later_capture
        ));
        // Arbitrary file inside a folder named "Screenshots" must NOT be removed
        assert!(!should_cleanup_portal_file(
            &nested_file,
            Some(&fake_pics),
            &fake_temp,
            now
        ));
        // Arbitrary user file containing "Screenshot" in Pictures must NOT be removed
        assert!(!should_cleanup_portal_file(
            &user_file,
            Some(&fake_pics),
            &fake_temp,
            now
        ));

        let _ = fs::remove_dir_all(&root);
    }
}
