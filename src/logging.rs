//! Optional file logging and GLib/GTK warning routing (`--log` or `VELOSHOT_LOG=1`).

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use chrono::Local;
use gtk4::glib::{self, LogLevel};

use crate::config::APP_DIR_NAME;

static LOG_ENABLED: AtomicBool = AtomicBool::new(false);
static LOG_FILE: OnceLock<Mutex<Option<File>>> = OnceLock::new();

/// Returns the XDG state log file path (`~/.local/state/veloshot/app.log`).
pub fn log_file_path() -> PathBuf {
    let base = dirs::state_dir()
        .or_else(dirs::cache_dir)
        .unwrap_or_else(std::env::temp_dir);
    base.join(APP_DIR_NAME).join("app.log")
}

/// Enables or disables file logging at runtime.
pub fn set_enabled(enabled: bool) {
    let was = LOG_ENABLED.swap(enabled, Ordering::Relaxed);
    if enabled && !was {
        let path = log_file_path();
        eprintln!("Protokollierung aktiv: {}", path.display());
        write_line(&format_log_line(
            "App",
            "INFO",
            "Protokollierung gestartet",
        ));
    }
}

/// Initializes the GLib/GTK default log handler and optional file logger.
pub fn init(enabled: bool) {
    if enabled {
        set_enabled(true);
    }

    glib::log_set_default_handler(|domain, level, msg| {
        let is_vulkan_swapchain_noise =
            msg.contains("VK_SUBOPTIMAL_KHR") || msg.contains("VK_ERROR_OUT_OF_DATE_KHR");

        if LOG_ENABLED.load(Ordering::Relaxed) {
            let line = format_log_line(
                domain.unwrap_or("GLib"),
                level_label(level),
                msg,
            );
            write_line(&line);
        }

        if !is_vulkan_swapchain_noise || LOG_ENABLED.load(Ordering::Relaxed) {
            glib::log_default_handler(domain, level, Some(msg));
        }
    });
}

/// Logs an application warning to stderr and (if enabled) to `app.log`.
pub fn log_warn(context: &str, msg: &str) {
    eprintln!("{context}: {msg}");
    if LOG_ENABLED.load(Ordering::Relaxed) {
        write_line(&format_log_line(context, "WARN", msg));
    }
}

pub fn format_log_line(domain: &str, level: &str, msg: &str) -> String {
    let ts = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    format!("[{ts}] [{domain}] [{level}] {msg}")
}

fn level_label(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "ERROR",
        LogLevel::Critical => "CRITICAL",
        LogLevel::Warning => "WARN",
        LogLevel::Message => "MSG",
        LogLevel::Info => "INFO",
        LogLevel::Debug => "DEBUG",
    }
}

fn write_line(line: &str) {
    let mutex = LOG_FILE.get_or_init(|| Mutex::new(None));
    let Ok(mut guard) = mutex.lock() else {
        return;
    };
    if guard.is_none() {
        let path = log_file_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        *guard = OpenOptions::new().create(true).append(true).open(path).ok();
    }
    if let Some(file) = guard.as_mut() {
        let _ = writeln!(file, "{line}");
        let _ = file.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_path_and_line_formatting() {
        let p = log_file_path();
        assert!(p.ends_with("veloshot/app.log"));
        let line = format_log_line("Gtk", "WARN", "Testnachricht");
        assert!(line.contains("[Gtk] [WARN] Testnachricht"));
    }
}
