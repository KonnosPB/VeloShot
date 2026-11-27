//! VeloShot — Fast, fluid screenshot tool for Ubuntu Wayland (GNOME).

mod annotations;
mod capture;
mod clipboard;
mod config;
mod history;
mod i18n;
mod logging;
mod ocr;
mod shortcuts;
mod tray;
mod ui;

use std::cell::{Cell, RefCell};
use std::env;
use std::process::ExitCode;
use std::rc::Rc;

use adw::prelude::*;
use gtk4::{gio, glib};
use libadwaita as adw;

use crate::config::APP_ID;
use crate::tray::TrayCommand;
use crate::ui::{AppState, carousel, main_window, preferences, trigger_capture_flow};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    let log_path = logging::log_file_path();
    println!(
        "{}",
        i18n::tr(
            "VeloShot v{VERSION}
Schnelles Ubuntu-Wayland-Screenshot-Tool mit Lupe, Verlaufs-Karussell und Editor.

NUTZUNG:
    veloshot [OPTIONEN] [BEFEHL]

BEFEHLE:
    (ohne Befehl)  Bereichsaufnahme (Greenshot-Zeiger & Lupe) direkt starten + Tray aktivieren
    capture       Bereichsaufnahme (Greenshot-Stil mit Lupe) sofort starten
    history       Verlaufs-Karussell öffnen
    window        Hauptfenster öffnen
    settings      Einstellungen öffnen
    daemon        Nur Hintergrunddienst + Taskleisten-Symbol starten
    quit          Laufenden Hintergrunddienst sauber beenden

OPTIONEN:
    --log         Warnungen & GTK-Logs zusätzlich protokollieren (oder VELOSHOT_LOG=1)
    --help, -h    Diese Hilfe anzeigen
    --version, -V Versionsnummer anzeigen",
            "VeloShot v{VERSION}
Fast Ubuntu Wayland screenshot tool with live loupe, carousel history, and annotation editor.

USAGE:
    veloshot [OPTIONS] [COMMAND]

COMMANDS:
    (no command)  Start region capture directly (loupe + mini-bar) and keep tray active
    capture       Start region capture immediately
    history       Open carousel history
    window        Open main hub window
    settings      Open preferences dialog
    daemon        Run background tray service quietly
    quit          Quit running background instance cleanly

OPTIONS:
    --log         Also write GLib/GTK warnings to log file (or VELOSHOT_LOG=1)
    --help, -h    Show this help message
    --version, -V Show version information",
        )
        .replace("{VERSION}", VERSION)
    );
    println!(
        "    {}: {}",
        i18n::tr("Logdatei", "Log file"),
        log_path.display()
    );
}

fn main() -> ExitCode {
    i18n::set_language(config::AppConfig::load().language);
    let cli_args: Vec<String> = env::args().skip(1).collect();
    let log_env = matches!(
        env::var("VELOSHOT_LOG").ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes")
    );
    let log_enabled = log_env || cli_args.iter().any(|a| a == "--log");
    let first_cmd = cli_args.iter().find(|a| a.as_str() != "--log").map(|s| s.as_str());

    // Handle --help and --version headlessly before initializing GTK/display.
    match first_cmd {
        Some("--help" | "-h") => {
            print_help();
            return ExitCode::SUCCESS;
        }
        Some("--version" | "-V") => {
            println!("veloshot {VERSION}");
            return ExitCode::SUCCESS;
        }
        Some(cmd)
            if !matches!(
                cmd,
                "capture" | "history" | "window" | "settings" | "daemon" | "quit"
            ) =>
        {
            eprintln!("Unbekannter Befehl „{cmd}“. Starte mit --help für Hilfe.");
            return ExitCode::from(2);
        }
        _ => {}
    }

    // Prefer GL GSK renderer on Wayland unless overridden by the user: avoids GTK 4.22 Vulkan
    // swapchain rebuild warnings (VK_SUBOPTIMAL_KHR / VK_ERROR_OUT_OF_DATE_KHR) and stutter.
    if env::var_os("GSK_RENDERER").is_none() {
        // SAFETY: Executed at startup on the single main thread before any other threads or GTK init.
        unsafe {
            env::set_var("GSK_RENDERER", "gl");
        }
    }

    adw::init().expect("Libadwaita konnte nicht initialisiert werden");
    logging::init(log_enabled);

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    let shared_state: Rc<RefCell<Option<AppState>>> = Rc::new(RefCell::new(None));
    let daemon_hold: Rc<RefCell<Option<gio::ApplicationHoldGuard>>> = Rc::new(RefCell::new(None));
    let initialized = Rc::new(Cell::new(false));

    let state_startup = Rc::clone(&shared_state);
    app.connect_startup(move |app| {
        *state_startup.borrow_mut() = Some(AppState::new(app.clone()));
    });

    let state_cmd = Rc::clone(&shared_state);
    let hold_cmd = Rc::clone(&daemon_hold);
    let init_cmd = Rc::clone(&initialized);
    app.connect_command_line(move |app, cmdline| {
        let args = cmdline.arguments();
        if args.iter().skip(1).any(|a| a.to_str() == Some("--log")) {
            logging::set_enabled(true);
        }

        let state = state_cmd
            .borrow()
            .clone()
            .unwrap_or_else(|| AppState::new(app.clone()));

        if !init_cmd.get() {
            ui::install_css();
            init_cmd.set(true);

            // Keep single-instance app alive so tray menu & Wayland clipboard persist.
            let mut guard = hold_cmd.borrow_mut();
            if guard.is_none() {
                *guard = Some(app.hold());
            }
            drop(guard);

            // Listen to System Tray / Top-Bar indicator commands.
            let tray_rx = tray::spawn_tray();
            let state_tray = state.clone();
            let app_tray = app.clone();
            let hold_tray = Rc::clone(&hold_cmd);
            glib::MainContext::default().spawn_local(async move {
                while let Ok(cmd) = tray_rx.recv().await {
                    state_tray.reload_config();
                    match cmd {
                        TrayCommand::Capture => {
                            trigger_capture_flow(state_tray.clone(), None);
                        }
                        TrayCommand::History => {
                            carousel::open_carousel(state_tray.clone());
                        }
                        TrayCommand::MainWindow => {
                            if let Some(win) = app_tray.active_window() {
                                win.present();
                            } else {
                                main_window::open_main_window(state_tray.clone());
                            }
                        }
                        TrayCommand::Settings => {
                            let win = app_tray.active_window();
                            preferences::open_preferences(state_tray.clone(), win.as_ref());
                        }
                        TrayCommand::Quit => {
                            hold_tray.borrow_mut().take();
                            app_tray.quit();
                            break;
                        }
                    }
                }
            });
        }

        state.reload_config();

        let subcommand = args
            .iter()
            .skip(1)
            .filter_map(|s| s.to_str())
            .find(|&s| s != "--log");

        match subcommand {
            None | Some("capture") => {
                trigger_capture_flow(state, None);
            }
            Some("history") => {
                carousel::open_carousel(state);
            }
            Some("window") => {
                if let Some(win) = app.active_window() {
                    win.present();
                } else {
                    main_window::open_main_window(state);
                }
            }
            Some("settings") => {
                let win = app.active_window();
                preferences::open_preferences(state, win.as_ref());
            }
            Some("daemon") => {
                // Background tray service is already active and held open.
            }
            Some("quit") => {
                hold_cmd.borrow_mut().take();
                app.quit();
            }
            _ => {}
        }
        0
    });

    let code = app.run();
    ExitCode::from(code.value() as u8)
}

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_is_non_empty() {
        assert!(!VERSION.is_empty());
    }
}
