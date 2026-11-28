//! XDG autostart management for launching the VeloShot background daemon at login.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::APP_ID;
use crate::i18n::tr;
use crate::shortcuts::is_running_in_snap;

/// Returns the user's XDG autostart `.desktop` file path for VeloShot.
pub fn autostart_file_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("autostart").join(format!("{APP_ID}.desktop"))
}

/// Returns `true` if VeloShot autostart is currently enabled on disk.
pub fn is_autostart_enabled() -> bool {
    if is_running_in_snap() {
        return false;
    }
    is_enabled_at(&autostart_file_path())
}

fn is_enabled_at(path: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(path) else {
        return false;
    };
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("Hidden=true")
            || trimmed.eq_ignore_ascii_case("X-GNOME-Autostart-enabled=false")
        {
            return false;
        }
    }
    true
}

fn format_autostart_entry(exec_path: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=VeloShot\n\
         Comment=Fast Ubuntu Wayland screenshot tool\n\
         Exec=\"{exec_path}\" daemon\n\
         Icon={APP_ID}\n\
         Terminal=false\n\
         Categories=Utility;\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

fn set_enabled_at(path: &Path, exec_path: &str, enabled: bool) -> Result<()> {
    if enabled {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Konnte Autostart-Ordner nicht anlegen: {}",
                    parent.display()
                )
            })?;
        }
        fs::write(path, format_autostart_entry(exec_path)).with_context(|| {
            format!("Konnte Autostart-Datei nicht schreiben: {}", path.display())
        })?;
    } else if path.exists() {
        fs::remove_file(path).with_context(|| {
            format!("Konnte Autostart-Datei nicht entfernen: {}", path.display())
        })?;
    }
    Ok(())
}

/// Enables or disables launching `veloshot daemon` automatically on desktop login.
pub fn set_autostart_enabled(enabled: bool) -> Result<String> {
    if is_running_in_snap() {
        return Ok(tr(
            "Im Snap-Modus bitte VeloShot in den GNOME-Startprogrammen hinzufügen.",
            "In Snap mode, please add VeloShot in GNOME Startup Applications.",
        )
        .to_string());
    }

    let exe = env::current_exe()
        .context("Konnte Programmpfad nicht ermitteln")?
        .to_string_lossy()
        .into_owned();
    set_enabled_at(&autostart_file_path(), &exe, enabled)?;

    if enabled {
        Ok(tr("Autostart aktiviert", "Autostart enabled").to_string())
    } else {
        Ok(tr("Autostart deaktiviert", "Autostart disabled").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_desktop_entry_roundtrip_and_disable() {
        let dir = std::env::temp_dir().join(format!("veloshot-autostart-{}", uuid::Uuid::new_v4()));
        let file = dir.join(format!("{APP_ID}.desktop"));

        assert!(!is_enabled_at(&file));

        set_enabled_at(&file, "/usr/local/bin/veloshot", true).unwrap();
        assert!(is_enabled_at(&file));
        let body = fs::read_to_string(&file).unwrap();
        assert!(body.contains("Exec=\"/usr/local/bin/veloshot\" daemon"));
        assert!(body.contains("X-GNOME-Autostart-enabled=true"));

        // Explicit disable flag inside existing file is respected
        fs::write(
            &file,
            "[Desktop Entry]\nType=Application\nName=VeloShot\nX-GNOME-Autostart-enabled=false\n",
        )
        .unwrap();
        assert!(!is_enabled_at(&file));

        set_enabled_at(&file, "/usr/local/bin/veloshot", false).unwrap();
        assert!(!file.exists());
        assert!(!is_enabled_at(&file));

        let _ = fs::remove_dir_all(&dir);
    }
}
