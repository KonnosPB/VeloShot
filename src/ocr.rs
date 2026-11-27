//! Offline OCR text extraction via optional system `tesseract` CLI.

use std::fs;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow};
use image::{ImageFormat, RgbaImage};
use uuid::Uuid;

use crate::i18n::{ResolvedLang, current_lang, tr};

/// Returns the `apt` package list for Tesseract OCR matching `lang` (always including English).
pub fn install_packages_for(lang: ResolvedLang) -> String {
    let code = lang.tesseract_code();
    if code == "eng" {
        "tesseract-ocr tesseract-ocr-eng".to_string()
    } else {
        format!("tesseract-ocr tesseract-ocr-{code} tesseract-ocr-eng")
    }
}

/// Returns the Ubuntu terminal install command tailored to `lang`.
pub fn install_command_for(lang: ResolvedLang) -> String {
    format!(
        "sudo apt update && sudo apt install -y {}",
        install_packages_for(lang)
    )
}

/// Returns the Ubuntu terminal install command tailored to the currently active UI language.
pub fn install_command() -> String {
    install_command_for(current_lang())
}

/// Returns the `-l` language selector passed to `tesseract` (active language + `eng`).
pub fn tesseract_lang_spec_for(lang: ResolvedLang) -> String {
    let code = lang.tesseract_code();
    if code == "eng" {
        "eng".to_string()
    } else {
        format!("{code}+eng")
    }
}

/// Returns `true` when the `tesseract` CLI is installed and runnable.
pub fn is_tesseract_available() -> bool {
    Command::new("tesseract")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Installs `tesseract-ocr` and language data for the active UI language via `pkexec` on Ubuntu.
pub fn spawn_tesseract_install_async() -> async_channel::Receiver<Result<String>> {
    let lang = current_lang();
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let res = run_tesseract_install(lang);
        let _ = tx.send_blocking(res);
    });
    rx
}

fn run_tesseract_install(lang: ResolvedLang) -> Result<String> {
    if std::env::var_os("SNAP").is_some() {
        return Err(anyhow!(
            "{} {}",
            tr(
                "Im Snap-Modus bitte über das Terminal installieren:",
                "In Snap mode please install from a terminal:",
            ),
            install_command_for(lang),
        ));
    }

    let apt_cmd = format!(
        "apt-get update && apt-get install -y {}",
        install_packages_for(lang)
    );
    let output = Command::new("pkexec")
        .args(["sh", "-c", &apt_cmd])
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

/// Extracts text from `img` using the system `tesseract` binary (active UI language + `eng`, fallback `eng` / default).
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

    let spec = tesseract_lang_spec_for(current_lang());
    let output = match run(&["-l", &spec]) {
        Ok(out) if out.status.success() => Ok(out),
        Ok(_) if spec != "eng" => match run(&["-l", "eng"]) {
            Ok(out) if out.status.success() => Ok(out),
            Ok(_) => run(&[]),
            Err(err) => Err(err),
        },
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
        assert_eq!(
            install_command_for(ResolvedLang::En),
            "sudo apt update && sudo apt install -y tesseract-ocr tesseract-ocr-eng"
        );
        assert_eq!(
            install_command_for(ResolvedLang::De),
            "sudo apt update && sudo apt install -y tesseract-ocr tesseract-ocr-deu tesseract-ocr-eng"
        );
        assert_eq!(
            install_command_for(ResolvedLang::El),
            "sudo apt update && sudo apt install -y tesseract-ocr tesseract-ocr-ell tesseract-ocr-eng"
        );
        assert_eq!(
            install_command_for(ResolvedLang::Fr),
            "sudo apt update && sudo apt install -y tesseract-ocr tesseract-ocr-fra tesseract-ocr-eng"
        );
        assert_eq!(
            install_command_for(ResolvedLang::Uk),
            "sudo apt update && sudo apt install -y tesseract-ocr tesseract-ocr-ukr tesseract-ocr-eng"
        );

        assert_eq!(tesseract_lang_spec_for(ResolvedLang::En), "eng");
        assert_eq!(tesseract_lang_spec_for(ResolvedLang::De), "deu+eng");
        assert_eq!(tesseract_lang_spec_for(ResolvedLang::El), "ell+eng");

        let all_langs = [
            ResolvedLang::En,
            ResolvedLang::De,
            ResolvedLang::El,
            ResolvedLang::Fr,
            ResolvedLang::Es,
            ResolvedLang::Nl,
            ResolvedLang::It,
            ResolvedLang::Pt,
            ResolvedLang::Pl,
            ResolvedLang::Sv,
            ResolvedLang::Da,
            ResolvedLang::No,
            ResolvedLang::Fi,
            ResolvedLang::Cs,
            ResolvedLang::Hu,
            ResolvedLang::Ro,
            ResolvedLang::Uk,
        ];
        for lang in all_langs {
            let code = lang.tesseract_code();
            assert_eq!(code.len(), 3);
            assert!(code.bytes().all(|b| b.is_ascii_lowercase()));
        }

        let _ = is_tesseract_available();
    }
}
