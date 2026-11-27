//! Offline OCR text extraction via optional system `tesseract` CLI.

use std::fs;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow};
use image::{ImageFormat, RgbaImage};
use uuid::Uuid;

use crate::i18n::tr;

pub const INSTALL_COMMAND: &str =
    "sudo apt update && sudo apt install -y tesseract-ocr tesseract-ocr-deu tesseract-ocr-eng";

/// Returns `true` when the `tesseract` CLI is installed and runnable.
pub fn is_tesseract_available() -> bool {
    Command::new("tesseract")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Installs `tesseract-ocr`, `tesseract-ocr-deu`, and `tesseract-ocr-eng` via `pkexec` on Ubuntu.
pub fn spawn_tesseract_install_async() -> async_channel::Receiver<Result<String>> {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let res = run_tesseract_install();
        let _ = tx.send_blocking(res);
    });
    rx
}

fn run_tesseract_install() -> Result<String> {
    if std::env::var_os("SNAP").is_some() {
        return Err(anyhow!(tr(
            "Im Snap-Modus bitte über das Terminal installieren: sudo apt install tesseract-ocr tesseract-ocr-deu",
            "In Snap mode please install from a terminal: sudo apt install tesseract-ocr tesseract-ocr-deu",
        )));
    }

    let output = Command::new("pkexec")
        .args([
            "sh",
            "-c",
            "apt-get update && apt-get install -y tesseract-ocr tesseract-ocr-deu tesseract-ocr-eng",
        ])
        .output()
        .map_err(|err| {
            anyhow!(
                "{}: {err}",
                tr(
                    "Konnte System-Installer (pkexec) nicht starten",
                    "Could not start system installer (pkexec)"
                )
            )
        })?;

    if output.status.success() {
        Ok(tr(
            "Tesseract OCR installiert — das OCR-Symbol ist ab sofort im Editor verfügbar",
            "Tesseract OCR installed — the OCR icon is now available in the editor",
        )
        .to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            Err(anyhow!(tr(
                "Installation abgebrochen oder ohne Berechtigung beendet",
                "Installation cancelled or exited without authorization"
            )))
        } else {
            Err(anyhow!(stderr))
        }
    }
}

/// Extracts text from `img` using the system `tesseract` binary (`deu+eng`, fallback default).
pub fn extract_text_from_image(img: &RgbaImage) -> Result<String> {
    let tmp_path = std::env::temp_dir().join(format!("jas-ocr-{}.png", Uuid::new_v4()));
    img.save_with_format(&tmp_path, ImageFormat::Png)
        .with_context(|| {
            format!(
                "{}: {}",
                tr(
                    "Konnte temporäres OCR-Bild nicht schreiben",
                    "Could not write temporary OCR image"
                ),
                tmp_path.display()
            )
        })?;

    let path_str = tmp_path.to_string_lossy().into_owned();
    let run = |lang_args: &[&str]| {
        let mut cmd = Command::new("tesseract");
        cmd.arg(&path_str).arg("stdout");
        cmd.args(lang_args);
        cmd.args(["--psm", "6"]);
        cmd.output()
    };

    let output = match run(&["-l", "deu+eng"]) {
        Ok(out) if out.status.success() => Ok(out),
        Ok(_) => run(&[]),
        Err(err) => Err(err),
    };

    let _ = fs::remove_file(&tmp_path);

    let out = output.map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            anyhow!(tr(
                "„tesseract“ ist nicht installiert (sudo apt install tesseract-ocr)",
                "\"tesseract\" is not installed (sudo apt install tesseract-ocr)",
            ))
        } else {
            anyhow!(
                "{}: {err}",
                tr("OCR-Ausführung fehlgeschlagen", "OCR execution failed")
            )
        }
    })?;

    if !out.status.success() {
        return Err(anyhow!(
            "{}: {}",
            tr("Tesseract-Fehler", "Tesseract error"),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        return Err(anyhow!(tr(
            "Kein Text im Bild erkannt",
            "No text recognized in image"
        )));
    }
    Ok(text)
}

/// Runs OCR in a background thread and sends the extracted text over a channel.
pub fn spawn_ocr_async(img: RgbaImage) -> async_channel::Receiver<Result<String>> {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let res = extract_text_from_image(&img);
        let _ = tx.send_blocking(res);
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_command_and_availability_probe() {
        assert!(INSTALL_COMMAND.contains("tesseract-ocr"));
        assert!(INSTALL_COMMAND.contains("tesseract-ocr-deu"));
        assert!(INSTALL_COMMAND.contains("tesseract-ocr-eng"));
        let _ = is_tesseract_available();
    }
}
