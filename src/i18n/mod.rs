//! Runtime localization for 17 European languages:
//! English, German, Greek, French, Spanish, Dutch, Italian, Portuguese, Polish,
//! Swedish, Danish, Norwegian, Finnish, Czech, Hungarian, Romanian, and Ukrainian.
//!
//! `Language::Auto` detects the active system locale from `LC_ALL` / `LC_MESSAGES` / `LANG`
//! and falls back to English.

mod existing;
mod nordic;
mod west_central_east;

use std::env;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::config::Language;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedLang {
    En = 0,
    De = 1,
    El = 2,
    Fr = 3,
    Es = 4,
    Nl = 5,
    It = 6,
    Pt = 7,
    Pl = 8,
    Sv = 9,
    Da = 10,
    No = 11,
    Fi = 12,
    Cs = 13,
    Hu = 14,
    Ro = 15,
    Uk = 16,
}

static ACTIVE_LANG: AtomicU8 = AtomicU8::new(ResolvedLang::En as u8);

/// Resolves `Language` using `LC_ALL` / `LC_MESSAGES` / `LANG` when set to `Auto`.
pub fn resolve_language(lang: Language) -> ResolvedLang {
    match lang {
        Language::Auto => detect_system_language(),
        Language::En => ResolvedLang::En,
        Language::De => ResolvedLang::De,
        Language::El => ResolvedLang::El,
        Language::Fr => ResolvedLang::Fr,
        Language::Es => ResolvedLang::Es,
        Language::Nl => ResolvedLang::Nl,
        Language::It => ResolvedLang::It,
        Language::Pt => ResolvedLang::Pt,
        Language::Pl => ResolvedLang::Pl,
        Language::Sv => ResolvedLang::Sv,
        Language::Da => ResolvedLang::Da,
        Language::No => ResolvedLang::No,
        Language::Fi => ResolvedLang::Fi,
        Language::Cs => ResolvedLang::Cs,
        Language::Hu => ResolvedLang::Hu,
        Language::Ro => ResolvedLang::Ro,
        Language::Uk => ResolvedLang::Uk,
    }
}

/// Sets the active application language globally.
pub fn set_language(lang: Language) {
    ACTIVE_LANG.store(resolve_language(lang) as u8, Ordering::Relaxed);
}

fn current_lang() -> ResolvedLang {
    match ACTIVE_LANG.load(Ordering::Relaxed) {
        1 => ResolvedLang::De,
        2 => ResolvedLang::El,
        3 => ResolvedLang::Fr,
        4 => ResolvedLang::Es,
        5 => ResolvedLang::Nl,
        6 => ResolvedLang::It,
        7 => ResolvedLang::Pt,
        8 => ResolvedLang::Pl,
        9 => ResolvedLang::Sv,
        10 => ResolvedLang::Da,
        11 => ResolvedLang::No,
        12 => ResolvedLang::Fi,
        13 => ResolvedLang::Cs,
        14 => ResolvedLang::Hu,
        15 => ResolvedLang::Ro,
        16 => ResolvedLang::Uk,
        _ => ResolvedLang::En,
    }
}

/// Returns the localized string matching the active language.
pub fn tr(de: &'static str, en: &'static str) -> &'static str {
    match current_lang() {
        ResolvedLang::De => de,
        ResolvedLang::En => en,
        other => translate_en(other, en).unwrap_or(en),
    }
}

fn detect_system_language() -> ResolvedLang {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = env::var(var) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return detect_locale_code(trimmed);
            }
        }
    }
    ResolvedLang::En
}

fn detect_locale_code(locale: &str) -> ResolvedLang {
    let lower = locale.trim().to_ascii_lowercase();
    let code = lower
        .split(['.', '@', '_', '-'])
        .next()
        .unwrap_or_default();
    match code {
        "de" => ResolvedLang::De,
        "el" => ResolvedLang::El,
        "fr" => ResolvedLang::Fr,
        "es" => ResolvedLang::Es,
        "nl" => ResolvedLang::Nl,
        "it" => ResolvedLang::It,
        "pt" => ResolvedLang::Pt,
        "pl" => ResolvedLang::Pl,
        "sv" => ResolvedLang::Sv,
        "da" => ResolvedLang::Da,
        "no" | "nb" | "nn" => ResolvedLang::No,
        "fi" => ResolvedLang::Fi,
        "cs" => ResolvedLang::Cs,
        "hu" => ResolvedLang::Hu,
        "ro" => ResolvedLang::Ro,
        "uk" => ResolvedLang::Uk,
        _ => ResolvedLang::En,
    }
}

fn translate_en(lang: ResolvedLang, en: &'static str) -> Option<&'static str> {
    match lang {
        ResolvedLang::El
        | ResolvedLang::Fr
        | ResolvedLang::Es
        | ResolvedLang::Nl
        | ResolvedLang::It => existing::translate(lang, en),
        ResolvedLang::Sv | ResolvedLang::Da | ResolvedLang::No | ResolvedLang::Fi => {
            nordic::translate(lang, en)
        }
        ResolvedLang::Pt
        | ResolvedLang::Pl
        | ResolvedLang::Cs
        | ResolvedLang::Hu
        | ResolvedLang::Ro
        | ResolvedLang::Uk => west_central_east::translate(lang, en),
        ResolvedLang::En | ResolvedLang::De => Some(en),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_TRANSLATION_KEYS: &[&str] = &[
        "Red",
        "Orange",
        "Yellow",
        "Green",
        "Blue",
        "Purple",
        "Gray",
        "White",
        "Black",
        "Transparent",
        "Dark",
        "No Frame",
        "GNOME Blue",
        "Sunset",
        "Dark Slate",
        "Aurora",
        "Rose",
        "Ocean",
        "Amber",
        "Pearl (Light)",
        "Automatic (System)",
        "Top",
        "Bottom",
        "Left Edge",
        "Right Edge",
        "Center",
        "Open Editor",
        "Open Carousel",
        "Clipboard Only",
        "PNG (Lossless)",
        "JPEG (Smaller file)",
        "Small",
        "Medium",
        "Large",
        "Export Format",
        "Default file format when saving screenshots",
        "Editor Icon Size",
        "Size of toolbar and tool icons in the editor",
        "VeloShot v{VERSION}\nFast Ubuntu Wayland screenshot tool with live loupe, carousel history, and annotation editor.\n\nUSAGE:\n    veloshot [OPTIONS] [COMMAND]\n\nCOMMANDS:\n    (no command)  Start region capture directly (loupe + mini-bar) and keep tray active\n    capture       Start region capture immediately\n    history       Open carousel history\n    window        Open main hub window\n    settings      Open preferences dialog\n    daemon        Run background tray service quietly\n    quit          Quit running background instance cleanly\n\nOPTIONS:\n    --log         Also write GLib/GTK warnings to log file (or VELOSHOT_LOG=1)\n    --help, -h    Show this help message\n    --version, -V Show version information",
        "Log file",
        "In Snap mode please install from a terminal: sudo apt install tesseract-ocr tesseract-ocr-deu",
        "Could not start system installer (pkexec)",
        "Tesseract OCR installed — the OCR icon is now available in the editor",
        "Installation cancelled or exited without authorization",
        "Could not write temporary OCR image",
        "\"tesseract\" is not installed (sudo apt install tesseract-ocr)",
        "OCR execution failed",
        "Tesseract error",
        "No text recognized in image",
        "Inside a Snap package, please add the shortcut in GNOME Settings → Keyboard → Keyboard Shortcuts (command: \"veloshot capture\" or \"veloshot history\").",
        "GNOME shortcuts saved",
        "Capture",
        "Carousel",
        "In Snap mode, please unbind the default GNOME screenshot shortcut in Settings → Keyboard and bind \"veloshot capture\" to Print.",
        "GNOME default screenshot disabled & Print key redirected to VeloShot",
        "In Snap mode, please reset in GNOME Settings → Keyboard.",
        "GNOME default screenshot shortcut (Print) restored",
        "Desktop shortcuts saved",
        "Default screenshot shortcut (Print) restored",
        "Enabled: uses this app · Disabled: restores desktop default",
        "Please assign shortcuts in your system keyboard settings",
        "In Snap mode, please reset in your system Keyboard Settings.",
        "Click: Capture region • Right-click: Carousel & Settings",
        "Capture Region",
        "Open Carousel History",
        "Open Main Window",
        "Preferences",
        "Quit",
        "Carousel History — VeloShot",
        "Close (Esc)",
        "Copy to clipboard (Enter / Ctrl+C)",
        "Open in editor (E)",
        "Save as… (Ctrl+S)",
        "Delete (Del)",
        "New capture (N)",
        "Preferences (,)",
        "Move dock to top / bottom",
        "History",
        "No screenshots in history",
        "No screenshots yet — click New or press Print",
        "Copied to clipboard",
        "Error",
        "Save Screenshot",
        "Saved",
        "Delete failed",
        "Undo (Ctrl+Z)",
        "Redo (Ctrl+Y)",
        "Open Carousel History (Ctrl+H)",
        "Apply — save to history & copy",
        "Copy to clipboard (Ctrl+C)",
        "Export… (Ctrl+S)",
        "Recognize & copy text via OCR (Tesseract)",
        "Select, move & delete objects with Del (V)",
        "Draw rectangle (1)",
        "Draw arrow (2)",
        "Draw circle / ellipse (3)",
        "Semi-transparent highlighter (4)",
        "Freehand drawing (5)",
        "Place numbered steps on click (6)",
        "Place text (7)",
        "Draw & label speech bubble (8)",
        "Pixelate / redact region (9)",
        "Dim surroundings & highlight region (0)",
        "Crop image (C)",
        "Stroke & font size",
        "Pixelation strength",
        "Background fill for shapes, text & speech bubbles (default: Transparent)",
        "Beautify frame (gradient backdrop, rounded corners & drop shadow)",
        "Rotate image 90° clockwise",
        "Flip image horizontally",
        "Zoom out",
        "Reset zoom to 100%",
        "Zoom in",
        "Enter text…",
        "Image cropped",
        "Edited image copied",
        "Export Screenshot",
        "Exported",
        "Saved to history & copied to clipboard",
        "Rotated 90°",
        "Flipped horizontally",
        "OCR error",
        "Text recognized & copied ({chars} chars)",
        "Fast Wayland screenshot tool with live loupe, carousel history, and annotation editor.",
        "{count} of max {max} screenshots in history  •  Carousel: {pos}  •  Shortcut: {cap}",
        "Drag | Enter = Fullscreen",
        "Display",
        "Click for instant capture",
        "Select Region — VeloShot",
        "Select region — drag with 6× live loupe (bar hides while dragging)",
        "Capture entire monitor (Enter)",
        "Capture all monitors together (Enter)",
        "Choose single window via GNOME dialog",
        "Re-capture in {secs} seconds (e.g. for open menus)",
        "Move toolbar to top / bottom",
        "Cancel (Esc)",
        "Preferences — VeloShot",
        "General",
        "Language",
        "Display Language",
        "Automatic (system locale) or choose a fixed language",
        "Language saved — applies to newly opened windows",
        "Carousel &amp; History",
        "Placement of the history carousel and maximum number of stored screenshots.",
        "Carousel Position",
        "Choose where the carousel docks on screen",
        "Maximum History Count",
        "Older screenshots are pruned automatically",
        "Capture &amp; Storage",
        "Post-capture workflow and default directory for exported screenshots.",
        "After Capture Action",
        "Auto-copy to clipboard",
        "Copies new screenshots as soon as selection finishes",
        "Default Save Folder",
        "Choose…",
        "Reset to default folder inside Pictures",
        "Choose Save Folder",
        "Default save folder updated",
        "Default save folder reset",
        "Filename Pattern",
        "Preview",
        "Preview &amp; Placeholders",
        "Reset default pattern",
        "Filename pattern reset",
        "Keyboard Shortcuts (GNOME / Wayland)",
        "Keyboard Shortcuts",
        "Apply in GNOME",
        "Apply in Desktop",
        "Writes the shortcuts directly into GNOME custom keybindings",
        "Writes the shortcuts directly into your desktop environment settings",
        "GTK accelerator format, e.g. \"Print\", \"<Super><Shift>s\", or \"<Ctrl><Alt>p\".",
        "Capture screenshot",
        "Open carousel history",
        "Redirect Print key to VeloShot",
        "Enabled: uses this app · Disabled: restores GNOME default",
        "Apply in GNOME",
        "Apply custom shortcuts only",
        "In Snap mode, configure via GNOME Settings → Keyboard",
        "Writes the shortcuts directly into GNOME custom keybindings",
        "Setup error",
        "Text Recognition (OCR)",
        "Uses optional system Tesseract for offline OCR in the editor.",
        "Installed — OCR icon is active in the editor",
        "Not installed — OCR icon is hidden in the editor",
        "Install",
        "Guide",
        "Show & copy install command",
        "Installing (password prompt)…",
        "For text recognition in the editor, VeloShot uses \"tesseract-ocr\" with German and English language data. Terminal command:",
        "Set up Tesseract OCR",
        "Close",
        "Copy Command",
        "Install command copied to clipboard",
    ];

    #[test]
    fn locale_detection_all_17_languages() {
        assert_eq!(detect_locale_code("de_DE.UTF-8"), ResolvedLang::De);
        assert_eq!(detect_locale_code("el_GR.UTF-8"), ResolvedLang::El);
        assert_eq!(detect_locale_code("fr_FR.UTF-8"), ResolvedLang::Fr);
        assert_eq!(detect_locale_code("es_ES.UTF-8"), ResolvedLang::Es);
        assert_eq!(detect_locale_code("nl_NL.UTF-8"), ResolvedLang::Nl);
        assert_eq!(detect_locale_code("it_IT.UTF-8"), ResolvedLang::It);
        assert_eq!(detect_locale_code("pt_PT.UTF-8"), ResolvedLang::Pt);
        assert_eq!(detect_locale_code("pt_BR.UTF-8"), ResolvedLang::Pt);
        assert_eq!(detect_locale_code("pl_PL.UTF-8"), ResolvedLang::Pl);
        assert_eq!(detect_locale_code("sv_SE.UTF-8"), ResolvedLang::Sv);
        assert_eq!(detect_locale_code("da_DK.UTF-8"), ResolvedLang::Da);
        assert_eq!(detect_locale_code("no_NO.UTF-8"), ResolvedLang::No);
        assert_eq!(detect_locale_code("nb_NO.UTF-8"), ResolvedLang::No);
        assert_eq!(detect_locale_code("nn_NO.UTF-8"), ResolvedLang::No);
        assert_eq!(detect_locale_code("fi_FI.UTF-8"), ResolvedLang::Fi);
        assert_eq!(detect_locale_code("cs_CZ.UTF-8"), ResolvedLang::Cs);
        assert_eq!(detect_locale_code("hu_HU.UTF-8"), ResolvedLang::Hu);
        assert_eq!(detect_locale_code("ro_RO.UTF-8"), ResolvedLang::Ro);
        assert_eq!(detect_locale_code("uk_UA.UTF-8"), ResolvedLang::Uk);
        assert_eq!(detect_locale_code("en_US.UTF-8"), ResolvedLang::En);
        assert_eq!(detect_locale_code("C"), ResolvedLang::En);
    }

    #[test]
    fn all_17_languages_complete_valid_markup_and_placeholders() {
        let non_en_de = [
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
        let placeholders = ["{VERSION}", "{count}", "{max}", "{pos}", "{cap}", "{secs}", "{chars}"];

        for lang in non_en_de {
            for &key in ALL_TRANSLATION_KEYS {
                let translated = translate_en(lang, key)
                    .unwrap_or_else(|| panic!("Missing translation for {lang:?}: {key:?}"));
                assert!(!translated.is_empty(), "Empty translation for {lang:?}: {key:?}");

                if key.contains("&amp;") {
                    assert!(
                        translated.contains("&amp;"),
                        "Expected &amp; in markup translation for {lang:?}: {translated:?}"
                    );
                    assert!(
                        !translated.replace("&amp;", "").contains('&'),
                        "Unescaped '&' in markup translation for {lang:?}: {translated:?}"
                    );
                }

                for ph in placeholders {
                    if key.contains(ph) {
                        assert!(
                            translated.contains(ph),
                            "Missing placeholder {ph} for {lang:?}: {translated:?}"
                        );
                    }
                }
            }
        }

        let expected_prefs = [
            (Language::De, "Einstellungen"),
            (Language::En, "Preferences"),
            (Language::El, "Προτιμήσεις"),
            (Language::Fr, "Préférences"),
            (Language::Es, "Preferencias"),
            (Language::Nl, "Voorkeuren"),
            (Language::It, "Preferenze"),
            (Language::Pt, "Preferências"),
            (Language::Pl, "Preferencje"),
            (Language::Sv, "Inställningar"),
            (Language::Da, "Indstillinger"),
            (Language::No, "Innstillinger"),
            (Language::Fi, "Asetukset"),
            (Language::Cs, "Předvolby"),
            (Language::Hu, "Beállítások"),
            (Language::Ro, "Preferințe"),
            (Language::Uk, "Налаштування"),
        ];
        for (lang, word) in expected_prefs {
            set_language(lang);
            assert_eq!(tr("Einstellungen", "Preferences"), word);
        }
    }
}
