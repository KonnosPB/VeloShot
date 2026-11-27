//! GNOME Adwaita PreferencesDialog for language, carousel position, history limit,
//! default save directory, capture behavior, and GNOME Print-key shortcut redirection.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk4::{Adjustment, StringList, gio};
use libadwaita as adw;

use crate::clipboard::copy_text_to_clipboard;
use crate::config::{
    AfterCaptureAction, AppConfig, CarouselPosition, Language, default_filename_pattern,
};
use crate::i18n::tr;
use crate::{ocr, shortcuts};
use crate::ui::{AppState, plain_toast};

/// Opens the native Libadwaita preferences dialog attached to `parent`.
pub fn open_preferences(state: AppState, parent: Option<&gtk4::Window>) {
    let dialog = adw::PreferencesDialog::builder()
        .title(tr(
            "Einstellungen — VeloShot",
            "Preferences — VeloShot",
        ))
        .build();
    let dialog_rc = Rc::new(dialog);
    let active_page: Rc<RefCell<Option<adw::PreferencesPage>>> = Rc::new(RefCell::new(None));

    let page = build_preferences_page(state, Rc::clone(&dialog_rc), Rc::clone(&active_page));
    *active_page.borrow_mut() = Some(page.clone());
    dialog_rc.add(&page);
    dialog_rc.present(parent);
}

fn build_preferences_page(
    state: AppState,
    dialog_rc: Rc<adw::PreferencesDialog>,
    active_page: Rc<RefCell<Option<adw::PreferencesPage>>>,
) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title(tr("Allgemein", "General"))
        .icon_name("preferences-system-symbolic")
        .build();

    // --- Group 0: Sprache / Language ---
    let lang_group = adw::PreferencesGroup::builder()
        .title(tr("Sprache", "Language"))
        .build();

    let lang_labels: Vec<String> = Language::ALL
        .iter()
        .map(|&l| {
            let lbl = l.label();
            if l == Language::Auto && !lbl.starts_with("Auto") {
                format!("Auto — {lbl}")
            } else {
                lbl.to_string()
            }
        })
        .collect();
    let lang_label_refs: Vec<&str> = lang_labels.iter().map(String::as_str).collect();
    let lang_model = StringList::new(&lang_label_refs);
    let lang_title = {
        let local = tr("Anzeigesprache", "Display Language");
        if local.contains("Language") {
            local.to_string()
        } else {
            format!("{local} / Language")
        }
    };
    let lang_row = adw::ComboRow::builder()
        .title(lang_title)
        .subtitle(tr(
            "Automatisch (System) oder feste Sprache wählen",
            "Automatic (system locale) or choose a fixed language",
        ))
        .model(&lang_model)
        .selected(state.config.borrow().language.to_index())
        .build();
    lang_row.add_prefix(&gtk4::Image::from_icon_name(
        "preferences-desktop-locale-symbolic",
    ));

    let state_lang = state.clone();
    let dialog_lang = Rc::clone(&dialog_rc);
    let active_page_lang = Rc::clone(&active_page);
    lang_row.connect_selected_notify(move |row| {
        let lang = Language::from_index(row.selected());
        if lang == state_lang.config.borrow().language {
            return;
        }
        crate::i18n::set_language(lang);
        {
            let mut cfg = state_lang.config.borrow_mut();
            cfg.language = lang;
            let _ = cfg.save();
        }
        let state_cb = state_lang.clone();
        let dlg_cb = Rc::clone(&dialog_lang);
        let page_slot = Rc::clone(&active_page_lang);
        gtk4::glib::idle_add_local_once(move || {
            dlg_cb.set_title(tr("Einstellungen — VeloShot", "Preferences — VeloShot"));
            if let Some(old) = page_slot.borrow_mut().take() {
                dlg_cb.remove(&old);
            }
            let new_page = build_preferences_page(state_cb, Rc::clone(&dlg_cb), Rc::clone(&page_slot));
            *page_slot.borrow_mut() = Some(new_page.clone());
            dlg_cb.add(&new_page);
            dlg_cb.add_toast(plain_toast(tr(
                "Sprache gespeichert — gilt sofort für neu geöffnete Fenster",
                "Language saved — applies to newly opened windows",
            )));
        });
    });
    lang_group.add(&lang_row);

    let icon_size_labels: Vec<&str> = crate::config::EditorIconSize::ALL
        .iter()
        .map(|s| s.label())
        .collect();
    let icon_size_model = StringList::new(&icon_size_labels);
    let icon_size_row = adw::ComboRow::builder()
        .title(tr("Icon-Größe im Editor", "Editor Icon Size"))
        .subtitle(tr(
            "Größe der Werkzeug- und Symbolleisten-Icons im Editor",
            "Size of toolbar and tool icons in the editor",
        ))
        .model(&icon_size_model)
        .selected(state.config.borrow().editor_icon_size.to_index())
        .build();

    let state_icon = state.clone();
    icon_size_row.connect_selected_notify(move |row| {
        let size = crate::config::EditorIconSize::from_index(row.selected());
        let mut cfg = state_icon.config.borrow_mut();
        cfg.editor_icon_size = size;
        let _ = cfg.save();
    });
    lang_group.add(&icon_size_row);
    page.add(&lang_group);

    // --- Group 1: Karussell & Historie ---
    let carousel_group = adw::PreferencesGroup::builder()
        .title(tr("Karussell &amp; Historie", "Carousel &amp; History"))
        .description(tr(
            "Position des Verlaufs-Karussells und maximale Anzahl gespeicherter Bilder.",
            "Placement of the history carousel and maximum number of stored screenshots.",
        ))
        .build();

    let position_labels: Vec<&str> = CarouselPosition::ALL.iter().map(|p| p.label()).collect();
    let position_model = StringList::new(&position_labels);
    let position_row = adw::ComboRow::builder()
        .title(tr("Karussell-Position", "Carousel Position"))
        .subtitle(tr(
            "Wähle, wo das Karussell auf dem Bildschirm erscheint",
            "Choose where the carousel docks on screen",
        ))
        .model(&position_model)
        .selected(state.config.borrow().carousel_position.to_index())
        .build();

    let state_pos = state.clone();
    position_row.connect_selected_notify(move |row| {
        let pos = CarouselPosition::from_index(row.selected());
        let mut cfg = state_pos.config.borrow_mut();
        cfg.carousel_position = pos;
        let _ = cfg.save();
    });
    carousel_group.add(&position_row);

    let history_adj = Adjustment::new(
        state.config.borrow().max_history as f64,
        1.0,
        200.0,
        1.0,
        10.0,
        0.0,
    );
    let history_row = adw::SpinRow::builder()
        .title(tr("Maximale Bildhistorie", "Maximum History Count"))
        .subtitle(tr(
            "Ältere Screenshots werden automatisch bereinigt",
            "Older screenshots are pruned automatically",
        ))
        .adjustment(&history_adj)
        .digits(0)
        .build();

    let state_hist = state.clone();
    history_row.connect_value_notify(move |row| {
        let max_hist = row.value().round().clamp(1.0, 200.0) as usize;
        {
            let mut cfg = state_hist.config.borrow_mut();
            cfg.max_history = max_hist;
            let _ = cfg.save();
        }
        let _ = state_hist.store.enforce_limit(max_hist);
    });
    carousel_group.add(&history_row);
    page.add(&carousel_group);

    // --- Group 2: Aufnahme & Speicherort ---
    let capture_group = adw::PreferencesGroup::builder()
        .title(tr("Aufnahme &amp; Speicherort", "Capture &amp; Storage"))
        .description(tr(
            "Verhalten nach der Aufnahme sowie Standardordner zum Speichern.",
            "Post-capture workflow and default directory for exported screenshots.",
        ))
        .build();

    let action_labels: Vec<&str> = AfterCaptureAction::ALL.iter().map(|a| a.label()).collect();
    let action_model = StringList::new(&action_labels);
    let action_row = adw::ComboRow::builder()
        .title(tr("Aktion nach Aufnahme", "After Capture Action"))
        .model(&action_model)
        .selected(state.config.borrow().after_capture.to_index())
        .build();

    let state_act = state.clone();
    action_row.connect_selected_notify(move |row| {
        let act = AfterCaptureAction::from_index(row.selected());
        let mut cfg = state_act.config.borrow_mut();
        cfg.after_capture = act;
        let _ = cfg.save();
    });
    capture_group.add(&action_row);

    let clipboard_row = adw::SwitchRow::builder()
        .title(tr(
            "Automatisch in Zwischenablage kopieren",
            "Auto-copy to clipboard",
        ))
        .subtitle(tr(
            "Kopiert neue Screenshots sofort beim Loslassen",
            "Copies new screenshots as soon as selection finishes",
        ))
        .active(state.config.borrow().auto_copy_clipboard)
        .build();

    let state_clip = state.clone();
    clipboard_row.connect_active_notify(move |row| {
        let mut cfg = state_clip.config.borrow_mut();
        cfg.auto_copy_clipboard = row.is_active();
        let _ = cfg.save();
    });
    capture_group.add(&clipboard_row);

    // Default save folder row (defaults to ~/Bilder/VeloShot or ~/Pictures/VeloShot)
    let save_dir_text = gtk4::glib::markup_escape_text(
        &state
            .config
            .borrow()
            .effective_save_dir()
            .display()
            .to_string(),
    );
    let save_dir_row = adw::ActionRow::builder()
        .title(tr("Standard-Speicherordner", "Default Save Folder"))
        .subtitle(save_dir_text)
        .build();

    let choose_dir_btn = gtk4::Button::builder()
        .label(tr("Ordner wählen…", "Choose…"))
        .valign(gtk4::Align::Center)
        .build();
    let reset_dir_btn = gtk4::Button::builder()
        .icon_name("edit-undo-symbolic")
        .tooltip_text(tr(
            "Auf Standardordner (unterhalb von Bilder) zurücksetzen",
            "Reset to default folder inside Pictures",
        ))
        .valign(gtk4::Align::Center)
        .build();
    let dir_btn_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(6)
        .valign(gtk4::Align::Center)
        .build();
    dir_btn_box.append(&choose_dir_btn);
    dir_btn_box.append(&reset_dir_btn);
    save_dir_row.add_suffix(&dir_btn_box);

    {
        let state_dir = state.clone();
        let row_ref = save_dir_row.clone();
        let dlg_ref = Rc::clone(&dialog_rc);
        choose_dir_btn.connect_clicked(move |_| {
            let current = state_dir.config.borrow().ensure_effective_save_dir();
            let fd = gtk4::FileDialog::builder()
                .title(tr("Speicherordner wählen", "Choose Save Folder"))
                .initial_folder(&gio::File::for_path(current))
                .build();
            let parent_win = dlg_ref.root().and_downcast::<gtk4::Window>();
            let state_cb = state_dir.clone();
            let row_cb = row_ref.clone();
            let dlg_cb = Rc::clone(&dlg_ref);
            fd.select_folder(
                parent_win.as_ref(),
                gio::Cancellable::NONE,
                move |res| {
                    if let Ok(file) = res
                        && let Some(path) = file.path()
                    {
                        {
                            let mut cfg = state_cb.config.borrow_mut();
                            cfg.save_dir = Some(path.clone());
                            let _ = cfg.save();
                        }
                        row_cb.set_subtitle(&gtk4::glib::markup_escape_text(
                            &path.display().to_string(),
                        ));
                        dlg_cb.add_toast(plain_toast(tr(
                            "Standard-Speicherordner aktualisiert",
                            "Default save folder updated",
                        )));
                    }
                },
            );
        });
    }
    {
        let state_rst = state.clone();
        let row_ref = save_dir_row.clone();
        let dlg_ref = Rc::clone(&dialog_rc);
        reset_dir_btn.connect_clicked(move |_| {
            {
                let mut cfg = state_rst.config.borrow_mut();
                cfg.save_dir = None;
                let _ = cfg.save();
            }
            let def = AppConfig::default_save_dir();
            row_ref.set_subtitle(&gtk4::glib::markup_escape_text(&def.display().to_string()));
            dlg_ref.add_toast(plain_toast(tr(
                "Standard-Speicherordner zurückgesetzt",
                "Default save folder reset",
            )));
        });
    }
    capture_group.add(&save_dir_row);

    let format_labels: Vec<&str> = crate::config::ExportFormat::ALL
        .iter()
        .map(|f| f.label())
        .collect();
    let format_model = StringList::new(&format_labels);
    let format_row = adw::ComboRow::builder()
        .title(tr("Export-Format", "Export Format"))
        .subtitle(tr(
            "Standard-Dateiformat beim Speichern von Screenshots",
            "Default file format when saving screenshots",
        ))
        .model(&format_model)
        .selected(state.config.borrow().export_format.to_index())
        .build();
    capture_group.add(&format_row);

    let pattern_entry = adw::EntryRow::builder()
        .title(tr("Dateinamen-Muster", "Filename Pattern"))
        .text(&state.config.borrow().filename_pattern)
        .build();

    fn format_pattern_hint(cfg: &AppConfig) -> String {
        let example = cfg.format_export_filename(chrono::Utc::now(), 1920, 1080);
        gtk4::glib::markup_escape_text(&format!(
            "{}: {example}\n<year> <month> <day> <hour> <minute> <second> <width> <height>",
            tr("Vorschau", "Preview")
        ))
        .to_string()
    }

    let pattern_preview_row = adw::ActionRow::builder()
        .title(tr("Vorschau &amp; Platzhalter", "Preview &amp; Placeholders"))
        .subtitle(format_pattern_hint(&state.config.borrow()))
        .build();
    let reset_pattern_btn = gtk4::Button::builder()
        .icon_name("edit-undo-symbolic")
        .tooltip_text(tr("Standard-Muster wiederherstellen", "Reset default pattern"))
        .valign(gtk4::Align::Center)
        .build();
    pattern_preview_row.add_suffix(&reset_pattern_btn);

    {
        let state_fmt = state.clone();
        let preview_ref = pattern_preview_row.clone();
        format_row.connect_selected_notify(move |row| {
            let fmt = crate::config::ExportFormat::from_index(row.selected());
            let mut cfg = state_fmt.config.borrow_mut();
            cfg.export_format = fmt;
            let _ = cfg.save();
            preview_ref.set_subtitle(&format_pattern_hint(&cfg));
        });
    }
    {
        let state_pat = state.clone();
        let preview_ref = pattern_preview_row.clone();
        pattern_entry.connect_changed(move |entry| {
            let mut cfg = state_pat.config.borrow_mut();
            cfg.filename_pattern = entry.text().to_string();
            let _ = cfg.save();
            preview_ref.set_subtitle(&format_pattern_hint(&cfg));
        });
    }
    {
        let entry_ref = pattern_entry.clone();
        let dlg_ref = Rc::clone(&dialog_rc);
        reset_pattern_btn.connect_clicked(move |_| {
            entry_ref.set_text(&default_filename_pattern());
            dlg_ref.add_toast(plain_toast(tr(
                "Dateinamen-Muster zurückgesetzt",
                "Filename pattern reset",
            )));
        });
    }
    capture_group.add(&pattern_entry);
    capture_group.add(&pattern_preview_row);
    page.add(&capture_group);

    // --- Group 3: Tastenkombinationen ---
    let desktop = shortcuts::detect_desktop_env();
    let shortcut_group_title = gtk4::glib::markup_escape_text(&format!(
        "{} ({})",
        tr("Tastenkombinationen", "Keyboard Shortcuts"),
        desktop.display_name()
    ));
    let shortcut_group = adw::PreferencesGroup::builder()
        .title(shortcut_group_title)
        .description(gtk4::glib::markup_escape_text(tr(
            "GTK-Kürzelformat, z. B. „Print“, „<Super><Shift>s“ oder „<Ctrl><Alt>p“.",
            "GTK accelerator format, e.g. \"Print\", \"<Super><Shift>s\", or \"<Ctrl><Alt>p\".",
        )))
        .build();

    let capture_entry = adw::EntryRow::builder()
        .title(tr("Screenshot aufnehmen", "Capture screenshot"))
        .text(&state.config.borrow().capture_shortcut)
        .build();
    let state_cap_key = state.clone();
    capture_entry.connect_changed(move |entry| {
        let mut cfg = state_cap_key.config.borrow_mut();
        cfg.capture_shortcut = entry.text().to_string();
        let _ = cfg.save();
    });
    shortcut_group.add(&capture_entry);

    let history_entry = adw::EntryRow::builder()
        .title(tr("Karussell-Historie öffnen", "Open carousel history"))
        .text(&state.config.borrow().history_shortcut)
        .build();
    let state_hist_key = state.clone();
    history_entry.connect_changed(move |entry| {
        let mut cfg = state_hist_key.config.borrow_mut();
        cfg.history_shortcut = entry.text().to_string();
        let _ = cfg.save();
    });
    shortcut_group.add(&history_entry);

    // Comfort switch: redirect desktop's default Print screenshot shortcut to VeloShot
    if desktop.supports_print_redirect() {
        let redirect_row = adw::SwitchRow::builder()
            .title(tr(
                "Druck-Taste auf VeloShot umleiten",
                "Redirect Print key to VeloShot",
            ))
            .subtitle(tr(
                "Aktiv: nutzt diese App · Deaktiviert: stellt den Standard wieder her",
                "Enabled: uses this app · Disabled: restores desktop default",
            ))
            .active(shortcuts::is_print_redirect_active())
            .build();

        let updating_switch = Rc::new(Cell::new(false));
        let state_redir = state.clone();
        let dlg_redir = Rc::clone(&dialog_rc);
        let guard = Rc::clone(&updating_switch);
        redirect_row.connect_active_notify(move |row| {
            if guard.get() {
                return;
            }
            let want_active = row.is_active();
            let res = if want_active {
                let cfg = state_redir.config.borrow().clone();
                shortcuts::redirect_print_shortcut(
                    &cfg.capture_shortcut,
                    &cfg.history_shortcut,
                )
            } else {
                shortcuts::restore_default_print_shortcut()
            };
            match res {
                Ok(info) => dlg_redir.add_toast(plain_toast(&info)),
                Err(err) => {
                    guard.set(true);
                    row.set_active(!want_active);
                    guard.set(false);
                    dlg_redir.add_toast(plain_toast(&format!("{}: {err}", tr("Fehler", "Error"))));
                }
            }
        });
        shortcut_group.add(&redirect_row);
    }

    let apply_btn = gtk4::Button::builder()
        .label(tr("Im Desktop einrichten", "Apply in Desktop"))
        .valign(gtk4::Align::Center)
        .build();

    let apply_row = adw::ActionRow::builder()
        .title(tr("Nur eigene Kürzel übernehmen", "Apply custom shortcuts only"))
        .subtitle(if shortcuts::is_running_in_snap() {
            tr(
                "Im Snap-Modus über GNOME Einstellungen → Tastatur verknüpfen",
                "In Snap mode, configure via GNOME Settings → Keyboard",
            )
        } else {
            tr(
                "Schreibt die Tastenkürzel direkt in die Einstellungen deiner Desktop-Umgebung",
                "Writes the shortcuts directly into your desktop environment settings",
            )
        })
        .build();
    apply_row.add_suffix(&apply_btn);
    apply_row.set_activatable_widget(Some(&apply_btn));

    let state_apply = state.clone();
    let dialog_toast = Rc::clone(&dialog_rc);
    apply_btn.connect_clicked(move |_| {
        let cfg = state_apply.config.borrow().clone();
        let msg = match shortcuts::apply_desktop_shortcuts(
            &cfg.capture_shortcut,
            &cfg.history_shortcut,
        ) {
            Ok(info) => info,
            Err(err) => format!("{}: {err}", tr("Fehler beim Einrichten", "Setup error")),
        };
        dialog_toast.add_toast(plain_toast(&msg));
    });
    shortcut_group.add(&apply_row);
    page.add(&shortcut_group);

    // --- Group 4: Texterkennung (OCR / optional Tesseract) ---
    let ocr_group = adw::PreferencesGroup::builder()
        .title(tr("Texterkennung (OCR)", "Text Recognition (OCR)"))
        .description(tr(
            "Nutzt optionales System-Tesseract für Offline-OCR im Editor.",
            "Uses optional system Tesseract for offline OCR in the editor.",
        ))
        .build();

    let installed = ocr::is_tesseract_available();
    let ocr_row = adw::ActionRow::builder()
        .title("Tesseract OCR")
        .subtitle(if installed {
            tr(
                "Installiert — OCR-Symbol ist im Editor aktiv",
                "Installed — OCR icon is active in the editor",
            )
        } else {
            tr(
                "Nicht installiert — OCR-Symbol im Editor ausgeblendet",
                "Not installed — OCR icon is hidden in the editor",
            )
        })
        .build();

    let install_btn = gtk4::Button::builder()
        .label(tr("Installieren", "Install"))
        .valign(gtk4::Align::Center)
        .css_classes(["suggested-action"])
        .visible(!installed && !shortcuts::is_running_in_snap())
        .build();
    let guide_btn = gtk4::Button::builder()
        .label(tr("Anleitung", "Guide"))
        .tooltip_text(tr(
            "Installationsbefehl anzeigen & kopieren",
            "Show & copy install command",
        ))
        .valign(gtk4::Align::Center)
        .build();
    let ocr_btn_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(6)
        .valign(gtk4::Align::Center)
        .build();
    ocr_btn_box.append(&install_btn);
    ocr_btn_box.append(&guide_btn);
    ocr_row.add_suffix(&ocr_btn_box);

    {
        let row_ref = ocr_row.clone();
        let dlg_ref = Rc::clone(&dialog_rc);
        install_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            row_ref.set_subtitle(tr(
                "Installation läuft (Passwort-Dialog)…",
                "Installing (password prompt)…",
            ));
            let rx = ocr::spawn_tesseract_install_async();
            let row_done = row_ref.clone();
            let dlg_done = Rc::clone(&dlg_ref);
            let btn_done = btn.clone();
            gtk4::glib::MainContext::default().spawn_local(async move {
                match rx.recv().await {
                    Ok(Ok(msg)) => {
                        row_done.set_subtitle(tr(
                            "Installiert — OCR-Symbol ist im Editor aktiv",
                            "Installed — OCR icon is active in the editor",
                        ));
                        btn_done.set_visible(false);
                        dlg_done.add_toast(plain_toast(&msg));
                    }
                    Ok(Err(err)) => {
                        if ocr::is_tesseract_available() {
                            row_done.set_subtitle(tr(
                                "Installiert — OCR-Symbol ist im Editor aktiv",
                                "Installed — OCR icon is active in the editor",
                            ));
                            btn_done.set_visible(false);
                        } else {
                            row_done.set_subtitle(tr(
                                "Nicht installiert — OCR-Symbol im Editor ausgeblendet",
                                "Not installed — OCR icon is hidden in the editor",
                            ));
                            btn_done.set_sensitive(true);
                        }
                        dlg_done.add_toast(plain_toast(&format!("{}: {err}", tr("Fehler", "Error"))));
                    }
                    Err(_) => {
                        btn_done.set_sensitive(true);
                    }
                }
            });
        });
    }

    {
        let dlg_guide = Rc::clone(&dialog_rc);
        guide_btn.connect_clicked(move |_| {
            let body = format!(
                "{}\n\n{}",
                tr(
                    "Für die Texterkennung im Editor nutzt VeloShot das Paket „tesseract-ocr“ samt deutscher und englischer Sprachdaten. Terminal-Befehl:",
                    "For text recognition in the editor, VeloShot uses \"tesseract-ocr\" with German and English language data. Terminal command:"
                ),
                ocr::INSTALL_COMMAND
            );
            let alert = adw::AlertDialog::builder()
                .heading(tr("Tesseract OCR einrichten", "Set up Tesseract OCR"))
                .body(&body)
                .build();
            alert.add_response("close", tr("Schließen", "Close"));
            alert.add_response("copy", tr("Befehl kopieren", "Copy Command"));
            alert.set_response_appearance("copy", adw::ResponseAppearance::Suggested);
            alert.set_default_response(Some("copy"));
            alert.set_close_response("close");

            let dlg_toast = Rc::clone(&dlg_guide);
            alert.connect_response(None, move |_, resp| {
                if resp == "copy" {
                    let msg = match copy_text_to_clipboard(ocr::INSTALL_COMMAND) {
                        Ok(()) => tr(
                            "Installationsbefehl in Zwischenablage kopiert",
                            "Install command copied to clipboard",
                        )
                        .to_string(),
                        Err(err) => format!("{}: {err}", tr("Fehler", "Error")),
                    };
                    dlg_toast.add_toast(plain_toast(&msg));
                }
            });
            alert.present(Some(&*dlg_guide));
        });
    }

    ocr_group.add(&ocr_row);
    page.add(&ocr_group);

    page
}
