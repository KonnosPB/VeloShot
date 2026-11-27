//! Multi-desktop keyboard shortcut registration and `Print`-key redirect helper.
//!
//! Automatically detects `GNOME`, `KDE Plasma`, `XFCE`, and `Sway` from session
//! environment variables (`XDG_CURRENT_DESKTOP`, `DESKTOP_SESSION`, `SWAYSOCK`)
//! and applies shortcuts using each desktop's native mechanism.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, anyhow};

use crate::i18n::tr;

const GNOME_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const GNOME_KEY: &str = "custom-keybindings";
const GNOME_SHELL_KEYBINDINGS: &str = "org.gnome.shell.keybindings";
const PATH_CAPTURE: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/veloshot-capture/";
const PATH_HISTORY: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/veloshot-history/";

/// Detected Linux desktop environment / compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEnv {
    Gnome,
    Kde,
    Xfce,
    Sway,
    Other,
}

impl DesktopEnv {
    pub fn display_name(self) -> &'static str {
        match self {
            DesktopEnv::Gnome => "GNOME",
            DesktopEnv::Kde => "KDE Plasma",
            DesktopEnv::Xfce => "XFCE",
            DesktopEnv::Sway => "Sway",
            DesktopEnv::Other => "Linux Desktop",
        }
    }

    pub fn supports_print_redirect(self) -> bool {
        matches!(self, DesktopEnv::Gnome | DesktopEnv::Xfce | DesktopEnv::Kde)
    }
}

/// Detects the active desktop environment from the current session environment.
pub fn detect_desktop_env() -> DesktopEnv {
    let xdg = env::var("XDG_CURRENT_DESKTOP").ok();
    let sess = env::var("DESKTOP_SESSION").ok();
    let has_swaysock = env::var_os("SWAYSOCK")
        .is_some_and(|v| !v.is_empty());
    detect_desktop_from_vars(xdg.as_deref(), sess.as_deref(), has_swaysock)
}

/// Pure environment-variable classifier for desktop environments (unit-testable).
pub fn detect_desktop_from_vars(
    xdg_current_desktop: Option<&str>,
    desktop_session: Option<&str>,
    has_swaysock: bool,
) -> DesktopEnv {
    if has_swaysock {
        return DesktopEnv::Sway;
    }

    let mut tokens = Vec::new();
    for val in [xdg_current_desktop, desktop_session].into_iter().flatten() {
        for part in val.split([':', ';', ',']) {
            let trimmed = part.trim().to_ascii_lowercase();
            if !trimmed.is_empty() {
                tokens.push(trimmed);
            }
        }
    }

    if tokens.iter().any(|t| t.contains("sway")) {
        DesktopEnv::Sway
    } else if tokens.iter().any(|t| t.contains("kde") || t.contains("plasma")) {
        DesktopEnv::Kde
    } else if tokens.iter().any(|t| t.contains("xfce") || t.contains("xubuntu")) {
        DesktopEnv::Xfce
    } else if tokens
        .iter()
        .any(|t| t.contains("gnome") || matches!(t.as_str(), "ubuntu" | "ubuntu-wayland" | "ubuntu-xorg") || t.contains("unity"))
    {
        DesktopEnv::Gnome
    } else {
        DesktopEnv::Other
    }
}

/// Returns `true` when running inside a strict Snap sandbox.
pub fn is_running_in_snap() -> bool {
    env::var_os("SNAP").is_some()
}

fn find_in_path(candidates: &[&'static str]) -> Option<&'static str> {
    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        for &bin in candidates {
            if dir.join(bin).is_file() {
                return Some(bin);
            }
        }
    }
    None
}

fn current_exe_str() -> Result<String> {
    Ok(env::current_exe()
        .context("Konnte Programmpfad nicht ermitteln")?
        .to_string_lossy()
        .into_owned())
}

/// Splits a GTK accelerator string (e.g. `"<Super><Shift>h"`) into lowercase modifier tags and key name.
fn split_gtk_accel(accel: &str) -> (Vec<String>, String) {
    let mut rest = accel.trim();
    let mut mods = Vec::new();
    while let Some(after_lt) = rest.strip_prefix('<') {
        if let Some(end) = after_lt.find('>') {
            let tag = after_lt[..end].trim().to_ascii_lowercase();
            if !tag.is_empty() {
                mods.push(tag);
            }
            rest = &after_lt[end + 1..];
        } else {
            break;
        }
    }
    (mods, rest.trim().to_string())
}

/// Converts a GTK accelerator (e.g. `<Ctrl><Alt>p`) to XFCE format (`<Primary><Alt>p`).
pub fn gtk_accel_to_xfce(accel: &str) -> String {
    let (mods, key) = split_gtk_accel(accel);
    let mut out = String::new();
    for m in mods {
        match m.as_str() {
            "ctrl" | "control" | "primary" => out.push_str("<Primary>"),
            "super" => out.push_str("<Super>"),
            "shift" => out.push_str("<Shift>"),
            "alt" => out.push_str("<Alt>"),
            "meta" => out.push_str("<Meta>"),
            other => {
                out.push('<');
                out.push_str(other);
                out.push('>');
            }
        }
    }
    out.push_str(&key);
    out
}

/// Converts a GTK accelerator (e.g. `<Super><Shift>h`) to KDE format (`Meta+Shift+H`).
pub fn gtk_accel_to_kde(accel: &str) -> String {
    let (mods, key) = split_gtk_accel(accel);
    let mut parts: Vec<String> = mods
        .into_iter()
        .map(|m| match m.as_str() {
            "super" | "meta" => "Meta".to_string(),
            "ctrl" | "control" | "primary" => "Ctrl".to_string(),
            "shift" => "Shift".to_string(),
            "alt" => "Alt".to_string(),
            other => other.to_string(),
        })
        .collect();
    let kde_key = if key.chars().count() == 1 {
        key.to_ascii_uppercase()
    } else {
        key
    };
    if !kde_key.is_empty() {
        parts.push(kde_key);
    }
    parts.join("+")
}

/// Converts a GTK accelerator (e.g. `<Super><Shift>h`) to Sway `bindsym` format (`$mod+Shift+h`).
pub fn gtk_accel_to_sway(accel: &str) -> String {
    let (mods, key) = split_gtk_accel(accel);
    let mut parts: Vec<String> = mods
        .into_iter()
        .map(|m| match m.as_str() {
            "super" | "meta" => "$mod".to_string(),
            "ctrl" | "control" | "primary" => "Ctrl".to_string(),
            "shift" => "Shift".to_string(),
            "alt" => "Mod1".to_string(),
            other => other.to_string(),
        })
        .collect();
    if !key.is_empty() {
        parts.push(key);
    }
    parts.join("+")
}

/// Formats a drop-in Sway config snippet for VeloShot bindings.
pub fn format_sway_snippet(exe: &str, capture_binding: &str, history_binding: &str) -> String {
    let sway_cap = gtk_accel_to_sway(capture_binding);
    let sway_hist = gtk_accel_to_sway(history_binding);
    format!(
        "# Managed by VeloShot\nbindsym {sway_cap} exec {exe} capture\nbindsym {sway_hist} exec {exe} history\n"
    )
}

/// Registers custom keyboard shortcuts using the active desktop environment's backend.
pub fn apply_desktop_shortcuts(capture_binding: &str, history_binding: &str) -> Result<String> {
    let env = detect_desktop_env();
    if is_running_in_snap() {
        return Ok(snap_shortcut_notice(env));
    }

    match env {
        DesktopEnv::Gnome => apply_gnome_shortcuts(capture_binding, history_binding),
        DesktopEnv::Xfce => apply_xfce_shortcuts(capture_binding, history_binding),
        DesktopEnv::Kde => apply_kde_shortcuts(capture_binding, history_binding),
        DesktopEnv::Sway => apply_sway_shortcuts(capture_binding, history_binding),
        DesktopEnv::Other => Ok(format!(
            "{}: {} (veloshot capture / veloshot history)",
            env.display_name(),
            tr(
                "Tastenkombinationen bitte in den System-Einstellungen hinterlegen",
                "Please assign shortcuts in your system keyboard settings",
            )
        )),
    }
}

/// Returns `true` when the desktop's built-in `Print` key is currently redirected to VeloShot.
pub fn is_print_redirect_active() -> bool {
    if is_running_in_snap() {
        return false;
    }
    match detect_desktop_env() {
        DesktopEnv::Gnome => is_gnome_print_redirect_active(),
        DesktopEnv::Xfce => is_xfce_print_redirect_active(),
        DesktopEnv::Kde => is_kde_print_redirect_active(),
        DesktopEnv::Sway | DesktopEnv::Other => false,
    }
}

/// Unbinds the desktop's built-in `Print` screenshot shortcut and assigns it to VeloShot.
pub fn redirect_print_shortcut(capture_binding: &str, history_binding: &str) -> Result<String> {
    let env = detect_desktop_env();
    if is_running_in_snap() {
        return Ok(snap_redirect_notice(env));
    }

    match env {
        DesktopEnv::Gnome => redirect_gnome_print_shortcut(capture_binding, history_binding),
        DesktopEnv::Xfce => redirect_xfce_print_shortcut(capture_binding, history_binding),
        DesktopEnv::Kde => redirect_kde_print_shortcut(capture_binding, history_binding),
        DesktopEnv::Sway | DesktopEnv::Other => {
            apply_desktop_shortcuts(capture_binding, history_binding)
        }
    }
}

/// Restores the desktop's built-in `Print` screenshot shortcut and removes VeloShot's override.
pub fn restore_default_print_shortcut() -> Result<String> {
    let env = detect_desktop_env();
    if is_running_in_snap() {
        return Ok(tr(
            "Im Snap-Modus bitte in den System-Einstellungen → Tastatur zurücksetzen.",
            "In Snap mode, please reset in your system Keyboard Settings.",
        )
        .to_string());
    }

    match env {
        DesktopEnv::Gnome => restore_gnome_default_print_shortcut(),
        DesktopEnv::Xfce => restore_xfce_default_print_shortcut(),
        DesktopEnv::Kde => restore_kde_default_print_shortcut(),
        DesktopEnv::Sway | DesktopEnv::Other => Ok(tr(
            "Standard-Screenshot-Kürzel (Druck-Taste) wiederhergestellt",
            "Default screenshot shortcut (Print) restored",
        )
        .to_string()),
    }
}

fn snap_shortcut_notice(env: DesktopEnv) -> String {
    if env == DesktopEnv::Gnome {
        tr(
            "Im Snap-Paket bitte die Tastenkombination in den GNOME-Einstellungen → Tastatur → Tastenkombinationen manuell anlegen (Befehl: „veloshot capture“ bzw. „veloshot history“).",
            "Inside a Snap package, please add the shortcut in GNOME Settings → Keyboard → Keyboard Shortcuts (command: \"veloshot capture\" or \"veloshot history\").",
        )
        .to_string()
    } else {
        format!(
            "{}: veloshot capture / veloshot history",
            env.display_name()
        )
    }
}

fn snap_redirect_notice(env: DesktopEnv) -> String {
    if env == DesktopEnv::Gnome {
        tr(
            "Im Snap-Modus bitte in den GNOME-Einstellungen → Tastatur den Standard-Screenshot deaktivieren und „veloshot capture“ auf Druck legen.",
            "In Snap mode, please unbind the default GNOME screenshot shortcut in Settings → Keyboard and bind \"veloshot capture\" to Print.",
        )
        .to_string()
    } else {
        format!(
            "{}: Print → veloshot capture",
            env.display_name()
        )
    }
}

fn saved_summary(label: &str, capture_binding: &str, history_binding: &str) -> String {
    format!(
        "{label}: {} ({capture_binding}), {} ({history_binding})",
        tr("Aufnahme", "Capture"),
        tr("Karussell", "Carousel")
    )
}

// ---------------------------------------------------------------------------
// GNOME (`gsettings`)
// ---------------------------------------------------------------------------

/// Registers or updates GNOME custom keybindings for capture and carousel history.
pub fn apply_gnome_shortcuts(capture_binding: &str, history_binding: &str) -> Result<String> {
    if is_running_in_snap() {
        return Ok(snap_shortcut_notice(DesktopEnv::Gnome));
    }

    let exe = current_exe_str()?;
    let mut paths = read_gsettings_array(GNOME_SCHEMA, GNOME_KEY)?;
    for required in [PATH_CAPTURE, PATH_HISTORY] {
        if !paths.iter().any(|p| p == required) {
            paths.push(required.to_string());
        }
    }

    let formatted_array = format_gsettings_str_array(&paths);
    run_gsettings(&["set", GNOME_SCHEMA, GNOME_KEY, &formatted_array])?;

    set_custom_binding(
        PATH_CAPTURE,
        "VeloShot: Aufnahme",
        &format!("{exe} capture"),
        capture_binding,
    )?;
    set_custom_binding(
        PATH_HISTORY,
        "VeloShot: Karussell",
        &format!("{exe} history"),
        history_binding,
    )?;

    Ok(saved_summary(
        tr("GNOME-Tastenkürzel gespeichert", "GNOME shortcuts saved"),
        capture_binding,
        history_binding,
    ))
}

/// Unbinds GNOME's built-in Print screenshot UI and binds the shortcut to VeloShot.
pub fn redirect_gnome_print_shortcut(
    capture_binding: &str,
    history_binding: &str,
) -> Result<String> {
    if is_running_in_snap() {
        return Ok(snap_redirect_notice(DesktopEnv::Gnome));
    }

    run_gsettings(&["set", GNOME_SHELL_KEYBINDINGS, "show-screenshot-ui", "@as []"])?;
    apply_gnome_shortcuts(capture_binding, history_binding)?;

    Ok(format!(
        "{} ({capture_binding})",
        tr(
            "GNOME-Standard-Screenshot deaktiviert & Taste auf VeloShot umgeleitet",
            "GNOME default screenshot disabled & Print key redirected to VeloShot"
        )
    ))
}

/// Returns `true` when GNOME's built-in Print screenshot UI is unbound and our capture shortcut is registered.
pub fn is_gnome_print_redirect_active() -> bool {
    if is_running_in_snap() {
        return false;
    }
    let Ok(shell_bindings) = read_gsettings_array(GNOME_SHELL_KEYBINDINGS, "show-screenshot-ui")
    else {
        return false;
    };
    if !shell_bindings.is_empty() {
        return false;
    }
    let Ok(custom_paths) = read_gsettings_array(GNOME_SCHEMA, GNOME_KEY) else {
        return false;
    };
    custom_paths.iter().any(|p| p == PATH_CAPTURE)
}

/// Removes VeloShot's custom-keybinding paths while preserving unrelated entries.
pub fn remove_app_keybinding_paths(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter(|p| p.as_str() != PATH_CAPTURE && p.as_str() != PATH_HISTORY)
        .cloned()
        .collect()
}

/// Restores GNOME's built-in `Print` screenshot shortcut (`show-screenshot-ui`) and removes our custom keybindings.
pub fn restore_gnome_default_print_shortcut() -> Result<String> {
    if is_running_in_snap() {
        return Ok(snap_redirect_notice(DesktopEnv::Gnome));
    }

    let existing = read_gsettings_array(GNOME_SCHEMA, GNOME_KEY)?;
    let remaining = remove_app_keybinding_paths(&existing);
    let formatted_array = format_gsettings_str_array(&remaining);
    run_gsettings(&["set", GNOME_SCHEMA, GNOME_KEY, &formatted_array])?;

    for path in [PATH_CAPTURE, PATH_HISTORY] {
        let rel_schema = format!("{GNOME_SCHEMA}.custom-keybinding:{path}");
        for key in ["name", "command", "binding"] {
            let _ = run_gsettings(&["reset", &rel_schema, key]);
        }
    }

    run_gsettings(&["reset", GNOME_SHELL_KEYBINDINGS, "show-screenshot-ui"])?;
    Ok(tr(
        "GNOME-Standard-Screenshot (Druck-Taste) wiederhergestellt",
        "GNOME default screenshot shortcut (Print) restored",
    )
    .to_string())
}

// ---------------------------------------------------------------------------
// XFCE (`xfconf-query`)
// ---------------------------------------------------------------------------

fn run_xfconf(args: &[&str]) -> Result<String> {
    let bin = find_in_path(&["xfconf-query"])
        .ok_or_else(|| anyhow!("xfconf-query wurde nicht gefunden"))?;
    let out = Command::new(bin)
        .args(args)
        .output()
        .with_context(|| format!("Konnte xfconf-query {:?} nicht ausführen", args))?;
    if !out.status.success() {
        return Err(anyhow!(
            "xfconf-query fehlgeschlagen: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn apply_xfce_shortcuts(capture_binding: &str, history_binding: &str) -> Result<String> {
    let exe = current_exe_str()?;
    let cap_prop = format!("/commands/custom/{}", gtk_accel_to_xfce(capture_binding));
    let hist_prop = format!("/commands/custom/{}", gtk_accel_to_xfce(history_binding));

    run_xfconf(&[
        "-c",
        "xfce4-keyboard-shortcuts",
        "-p",
        &cap_prop,
        "-n",
        "-t",
        "string",
        "-s",
        &format!("{exe} capture"),
    ])?;
    run_xfconf(&[
        "-c",
        "xfce4-keyboard-shortcuts",
        "-p",
        &hist_prop,
        "-n",
        "-t",
        "string",
        "-s",
        &format!("{exe} history"),
    ])?;

    Ok(saved_summary(
        tr("Desktop-Tastenkürzel gespeichert", "Desktop shortcuts saved"),
        capture_binding,
        history_binding,
    ))
}

fn is_xfce_print_redirect_active() -> bool {
    run_xfconf(&[
        "-c",
        "xfce4-keyboard-shortcuts",
        "-p",
        "/commands/custom/Print",
    ])
    .is_ok_and(|val| val.contains("veloshot"))
}

fn redirect_xfce_print_shortcut(capture_binding: &str, history_binding: &str) -> Result<String> {
    let exe = current_exe_str()?;
    run_xfconf(&[
        "-c",
        "xfce4-keyboard-shortcuts",
        "-p",
        "/commands/custom/Print",
        "-n",
        "-t",
        "string",
        "-s",
        &format!("{exe} capture"),
    ])?;
    apply_xfce_shortcuts(capture_binding, history_binding)?;
    Ok(format!("XFCE: Print → {exe} capture"))
}

fn restore_xfce_default_print_shortcut() -> Result<String> {
    let _ = run_xfconf(&[
        "-c",
        "xfce4-keyboard-shortcuts",
        "-p",
        "/commands/custom/Print",
        "-r",
    ]);
    Ok(tr(
        "Standard-Screenshot-Kürzel (Druck-Taste) wiederhergestellt",
        "Default screenshot shortcut (Print) restored",
    )
    .to_string())
}

// ---------------------------------------------------------------------------
// KDE Plasma (`kwriteconfig6`/`kwriteconfig5` + helper .desktop launchers)
// ---------------------------------------------------------------------------

fn write_kde_helper_desktop(file_name: &str, name: &str, exec_cmd: &str) -> Result<()> {
    let apps_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from(".local/share"))
        .join("applications");
    fs::create_dir_all(&apps_dir)?;
    let content = format!(
        "[Desktop Entry]\nType=Application\nName={name}\nExec={exec_cmd}\nIcon=io.github.konnospb.VeloShot\nNoDisplay=true\n"
    );
    fs::write(apps_dir.join(file_name), content)?;
    Ok(())
}

fn run_kde_write(group: &str, key: &str, value: &str) -> Result<()> {
    let bin = find_in_path(&["kwriteconfig6", "kwriteconfig5"])
        .ok_or_else(|| anyhow!("kwriteconfig6/kwriteconfig5 wurde nicht gefunden"))?;
    let out = Command::new(bin)
        .args([
            "--file",
            "kglobalshortcutsrc",
            "--group",
            "services",
            "--group",
            group,
            "--key",
            key,
            value,
        ])
        .output()
        .with_context(|| format!("Konnte {bin} nicht ausführen"))?;
    if !out.status.success() {
        return Err(anyhow!(
            "{bin} fehlgeschlagen: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

fn read_kde_key(group: &str, key: &str) -> Option<String> {
    let bin = find_in_path(&["kreadconfig6", "kreadconfig5"])?;
    let out = Command::new(bin)
        .args([
            "--file",
            "kglobalshortcutsrc",
            "--group",
            "services",
            "--group",
            group,
            "--key",
            key,
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn reload_kde_global_accel() {
    if let Some(dbus) = find_in_path(&["dbus-send"]) {
        let _ = Command::new(dbus)
            .args([
                "--session",
                "--type=method_call",
                "--dest=org.kde.kglobalaccel",
                "/kglobalaccel",
                "org.kde.KGlobalAccel.reloadConfig",
            ])
            .output();
    }
}

fn apply_kde_shortcuts(capture_binding: &str, history_binding: &str) -> Result<String> {
    let exe = current_exe_str()?;
    write_kde_helper_desktop(
        "veloshot-capture.desktop",
        "VeloShot Capture",
        &format!("{exe} capture"),
    )?;
    write_kde_helper_desktop(
        "veloshot-history.desktop",
        "VeloShot History",
        &format!("{exe} history"),
    )?;

    let kde_cap = gtk_accel_to_kde(capture_binding);
    let kde_hist = gtk_accel_to_kde(history_binding);

    run_kde_write(
        "veloshot-capture.desktop",
        "_launch",
        &format!("{kde_cap},none,VeloShot: Capture"),
    )?;
    run_kde_write(
        "veloshot-history.desktop",
        "_launch",
        &format!("{kde_hist},none,VeloShot: Carousel"),
    )?;
    reload_kde_global_accel();

    Ok(saved_summary(
        tr("Desktop-Tastenkürzel gespeichert", "Desktop shortcuts saved"),
        &kde_cap,
        &kde_hist,
    ))
}

fn is_kde_print_redirect_active() -> bool {
    read_kde_key("veloshot-capture.desktop", "_launch")
        .is_some_and(|val| val.split(',').next().is_some_and(|k| k == "Print"))
}

fn redirect_kde_print_shortcut(capture_binding: &str, history_binding: &str) -> Result<String> {
    let _ = run_kde_write(
        "org.kde.spectacle.desktop",
        "_launch",
        "none,Print,Spectacle",
    );
    apply_kde_shortcuts(capture_binding, history_binding)?;
    Ok(format!("KDE Plasma: Print → VeloShot ({capture_binding})"))
}

fn restore_kde_default_print_shortcut() -> Result<String> {
    let _ = run_kde_write(
        "veloshot-capture.desktop",
        "_launch",
        "none,none,VeloShot: Capture",
    );
    let _ = run_kde_write(
        "org.kde.spectacle.desktop",
        "_launch",
        "Print,Print,Spectacle",
    );
    reload_kde_global_accel();
    Ok(tr(
        "Standard-Screenshot-Kürzel (Druck-Taste) wiederhergestellt",
        "Default screenshot shortcut (Print) restored",
    )
    .to_string())
}

// ---------------------------------------------------------------------------
// Sway (`~/.config/sway/config.d/90-veloshot.conf` + `swaymsg reload`)
// ---------------------------------------------------------------------------

fn apply_sway_shortcuts(capture_binding: &str, history_binding: &str) -> Result<String> {
    let exe = current_exe_str()?;
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("sway")
        .join("config.d");
    fs::create_dir_all(&dir)?;
    let conf_path = dir.join("90-veloshot.conf");
    let snippet = format_sway_snippet(&exe, capture_binding, history_binding);
    fs::write(&conf_path, snippet)?;

    if let Some(swaymsg) = find_in_path(&["swaymsg"]) {
        let _ = Command::new(swaymsg).arg("reload").output();
    }

    Ok(format!(
        "{} ({})",
        tr("Desktop-Tastenkürzel gespeichert", "Desktop shortcuts saved"),
        conf_path.display()
    ))
}

fn read_gsettings_array(schema: &str, key: &str) -> Result<Vec<String>> {
    let out = Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .with_context(|| format!("Konnte gsettings get {schema} {key} nicht ausführen"))?;
    if !out.status.success() {
        return Err(anyhow!(
            "gsettings get {schema} {key} fehlgeschlagen: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(parse_gsettings_str_array(&String::from_utf8_lossy(
        &out.stdout,
    )))
}

fn set_custom_binding(path: &str, name: &str, command: &str, binding: &str) -> Result<()> {
    let rel_schema = format!("{GNOME_SCHEMA}.custom-keybinding:{path}");
    run_gsettings(&["set", &rel_schema, "name", name])?;
    run_gsettings(&["set", &rel_schema, "command", command])?;
    run_gsettings(&["set", &rel_schema, "binding", binding])?;
    Ok(())
}

fn run_gsettings(args: &[&str]) -> Result<()> {
    let out = Command::new("gsettings")
        .args(args)
        .output()
        .with_context(|| format!("Konnte gsettings {:?} nicht ausführen", args))?;
    if !out.status.success() {
        return Err(anyhow!(
            "gsettings {:?} fehlgeschlagen: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

pub fn parse_gsettings_str_array(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed == "@as []" || trimmed == "[]" {
        return Vec::new();
    }
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(trimmed);
    inner
        .split(',')
        .map(|part| part.trim().trim_matches('\'').trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn format_gsettings_str_array(items: &[String]) -> String {
    if items.is_empty() {
        return "@as []".to_string();
    }
    let quoted: Vec<String> = items
        .iter()
        .map(|s| format!("'{}'", s.replace('\'', "\\'")))
        .collect();
    format!("[{}]", quoted.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gsettings_array_parse_and_format() {
        assert!(parse_gsettings_str_array("@as []").is_empty());
        assert!(parse_gsettings_str_array("[]").is_empty());

        let parsed = parse_gsettings_str_array("['/org/foo/', '/org/bar/']");
        assert_eq!(parsed, vec!["/org/foo/", "/org/bar/"]);
        assert_eq!(
            format_gsettings_str_array(&parsed),
            "['/org/foo/', '/org/bar/']"
        );
    }

    #[test]
    fn remove_app_keybinding_paths_preserves_other_entries() {
        let input = vec![
            "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/".to_string(),
            PATH_CAPTURE.to_string(),
            PATH_HISTORY.to_string(),
            "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom1/".to_string(),
        ];
        assert_eq!(
            remove_app_keybinding_paths(&input),
            vec![
                "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/",
                "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom1/",
            ]
        );
        assert!(
            remove_app_keybinding_paths(&[PATH_CAPTURE.to_string(), PATH_HISTORY.to_string()])
                .is_empty()
        );
    }

    #[test]
    fn detect_desktop_variants() {
        assert_eq!(
            detect_desktop_from_vars(Some("ubuntu:GNOME"), Some("ubuntu"), false),
            DesktopEnv::Gnome
        );
        assert_eq!(
            detect_desktop_from_vars(Some("KDE"), Some("plasma"), false),
            DesktopEnv::Kde
        );
        assert_eq!(
            detect_desktop_from_vars(Some("XFCE"), Some("xubuntu"), false),
            DesktopEnv::Xfce
        );
        assert_eq!(
            detect_desktop_from_vars(Some("GNOME"), None, true),
            DesktopEnv::Sway
        );
        assert_eq!(
            detect_desktop_from_vars(Some("sway"), None, false),
            DesktopEnv::Sway
        );
        assert_eq!(
            detect_desktop_from_vars(Some("LXQt"), Some("Lubuntu"), false),
            DesktopEnv::Other
        );
        assert_eq!(detect_desktop_from_vars(None, None, false), DesktopEnv::Other);
    }

    #[test]
    fn accelerator_conversions_for_xfce_kde_and_sway() {
        assert_eq!(gtk_accel_to_xfce("Print"), "Print");
        assert_eq!(gtk_accel_to_xfce("<Ctrl><Alt>p"), "<Primary><Alt>p");
        assert_eq!(gtk_accel_to_xfce("<Super><Shift>h"), "<Super><Shift>h");

        assert_eq!(gtk_accel_to_kde("Print"), "Print");
        assert_eq!(gtk_accel_to_kde("<Super><Shift>h"), "Meta+Shift+H");
        assert_eq!(gtk_accel_to_kde("<Ctrl><Alt>p"), "Ctrl+Alt+P");

        assert_eq!(gtk_accel_to_sway("Print"), "Print");
        assert_eq!(gtk_accel_to_sway("<Super><Shift>h"), "$mod+Shift+h");
        assert_eq!(gtk_accel_to_sway("<Ctrl><Alt>p"), "Ctrl+Mod1+p");

        let snippet = format_sway_snippet("/usr/bin/veloshot", "Print", "<Super><Shift>h");
        assert!(snippet.contains("bindsym Print exec /usr/bin/veloshot capture"));
        assert!(snippet.contains("bindsym $mod+Shift+h exec /usr/bin/veloshot history"));
    }
}
