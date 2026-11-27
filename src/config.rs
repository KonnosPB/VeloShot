//! Application configuration persisted as TOML in XDG config directory.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};

use crate::i18n::tr;

pub const APP_ID: &str = "io.github.konnospb.VeloShot";
pub const APP_DIR_NAME: &str = "veloshot";

/// UI language preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    Auto,
    En,
    De,
    El,
    Fr,
    Es,
    Nl,
    It,
    Pt,
    Pl,
    Sv,
    Da,
    No,
    Fi,
    Cs,
    Hu,
    Ro,
    Uk,
}

impl Language {
    pub const ALL: [Language; 18] = [
        Language::Auto,
        Language::En,
        Language::De,
        Language::El,
        Language::Fr,
        Language::Es,
        Language::Nl,
        Language::It,
        Language::Pt,
        Language::Pl,
        Language::Sv,
        Language::Da,
        Language::No,
        Language::Fi,
        Language::Cs,
        Language::Hu,
        Language::Ro,
        Language::Uk,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Language::Auto => tr("Automatisch (System)", "Automatic (System)"),
            Language::En => "English",
            Language::De => "Deutsch",
            Language::El => "Ελληνικά",
            Language::Fr => "Français",
            Language::Es => "Español",
            Language::Nl => "Nederlands",
            Language::It => "Italiano",
            Language::Pt => "Português",
            Language::Pl => "Polski",
            Language::Sv => "Svenska",
            Language::Da => "Dansk",
            Language::No => "Norsk",
            Language::Fi => "Suomi",
            Language::Cs => "Čeština",
            Language::Hu => "Magyar",
            Language::Ro => "Română",
            Language::Uk => "Українська",
        }
    }

    pub fn from_index(idx: u32) -> Self {
        Self::ALL.get(idx as usize).copied().unwrap_or_default()
    }

    pub fn to_index(self) -> u32 {
        Self::ALL.iter().position(|&l| l == self).unwrap_or(0) as u32
    }
}

/// Screen edge / region where the history carousel docks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CarouselPosition {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
    Center,
}

impl CarouselPosition {
    pub const ALL: [CarouselPosition; 3] = [
        CarouselPosition::Top,
        CarouselPosition::Bottom,
        CarouselPosition::Center,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CarouselPosition::Top => tr("Oben", "Top"),
            CarouselPosition::Bottom => tr("Unten", "Bottom"),
            CarouselPosition::Left => tr("Linker Rand", "Left Edge"),
            CarouselPosition::Right => tr("Rechter Rand", "Right Edge"),
            CarouselPosition::Center => tr("Zentral", "Center"),
        }
    }

    pub fn is_vertical(self) -> bool {
        matches!(self, CarouselPosition::Left | CarouselPosition::Right)
    }

    pub fn from_index(idx: u32) -> Self {
        Self::ALL.get(idx as usize).copied().unwrap_or_default()
    }

    pub fn to_index(self) -> u32 {
        Self::ALL
            .iter()
            .position(|&p| p == self)
            .unwrap_or(1) as u32
    }
}

/// Action to take immediately after completing a screenshot region selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AfterCaptureAction {
    #[default]
    OpenEditor,
    OpenCarousel,
    CopyOnly,
}

impl AfterCaptureAction {
    pub const ALL: [AfterCaptureAction; 3] = [
        AfterCaptureAction::OpenEditor,
        AfterCaptureAction::OpenCarousel,
        AfterCaptureAction::CopyOnly,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AfterCaptureAction::OpenEditor => tr("Editor öffnen", "Open Editor"),
            AfterCaptureAction::OpenCarousel => tr("Karussell öffnen", "Open Carousel"),
            AfterCaptureAction::CopyOnly => tr("Nur in Zwischenablage", "Clipboard Only"),
        }
    }

    pub fn from_index(idx: u32) -> Self {
        Self::ALL.get(idx as usize).copied().unwrap_or_default()
    }

    pub fn to_index(self) -> u32 {
        Self::ALL
            .iter()
            .position(|&a| a == self)
            .unwrap_or(0) as u32
    }
}

/// Default file format when exporting screenshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    #[default]
    Png,
    Jpg,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 2] = [ExportFormat::Png, ExportFormat::Jpg];

    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Png => tr("PNG (Verlustfrei)", "PNG (Lossless)"),
            ExportFormat::Jpg => tr("JPEG (Kleinere Datei)", "JPEG (Smaller file)"),
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Png => "png",
            ExportFormat::Jpg => "jpg",
        }
    }

    pub fn from_index(idx: u32) -> Self {
        Self::ALL.get(idx as usize).copied().unwrap_or_default()
    }

    pub fn to_index(self) -> u32 {
        Self::ALL.iter().position(|&f| f == self).unwrap_or(0) as u32
    }
}

/// Icon & button size preset in the annotation editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EditorIconSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl EditorIconSize {
    pub const ALL: [EditorIconSize; 3] = [
        EditorIconSize::Small,
        EditorIconSize::Medium,
        EditorIconSize::Large,
    ];

    pub fn label(self) -> &'static str {
        match self {
            EditorIconSize::Small => tr("Klein", "Small"),
            EditorIconSize::Medium => tr("Mittel", "Medium"),
            EditorIconSize::Large => tr("Groß", "Large"),
        }
    }

    pub fn header_px(self) -> i32 {
        match self {
            EditorIconSize::Small => 18,
            EditorIconSize::Medium => 22,
            EditorIconSize::Large => 28,
        }
    }

    pub fn tool_px(self) -> i32 {
        match self {
            EditorIconSize::Small => 20,
            EditorIconSize::Medium => 28,
            EditorIconSize::Large => 36,
        }
    }

    pub fn css_class(self) -> &'static str {
        match self {
            EditorIconSize::Small => "tool-size-small",
            EditorIconSize::Medium => "tool-size-medium",
            EditorIconSize::Large => "tool-size-large",
        }
    }

    pub fn from_index(idx: u32) -> Self {
        Self::ALL.get(idx as usize).copied().unwrap_or_default()
    }

    pub fn to_index(self) -> u32 {
        Self::ALL.iter().position(|&s| s == self).unwrap_or(1) as u32
    }
}

/// Persistent user preferences for VeloShot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub language: Language,
    pub carousel_position: CarouselPosition,
    pub max_history: usize,
    pub after_capture: AfterCaptureAction,
    pub auto_copy_clipboard: bool,
    pub capture_shortcut: String,
    pub history_shortcut: String,
    pub save_dir: Option<PathBuf>,
    #[serde(default = "default_filename_pattern")]
    pub filename_pattern: String,
    pub export_format: ExportFormat,
    pub editor_icon_size: EditorIconSize,
}

pub fn default_filename_pattern() -> String {
    "<year>-<month>-<day>_<hour>-<minute>-<second>_Screenshot".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: Language::Auto,
            carousel_position: CarouselPosition::Bottom,
            max_history: 25,
            after_capture: AfterCaptureAction::OpenEditor,
            auto_copy_clipboard: true,
            capture_shortcut: "Print".to_string(),
            history_shortcut: "<Super><Shift>h".to_string(),
            save_dir: None,
            filename_pattern: default_filename_pattern(),
            export_format: ExportFormat::Png,
            editor_icon_size: EditorIconSize::Medium,
        }
    }
}

impl AppConfig {
    /// Returns the default XDG config file path.
    pub fn default_path() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join(APP_DIR_NAME).join("config.toml")
    }

    /// Loads config from the default path, returning defaults if the file does not exist.
    pub fn load() -> Self {
        let path = Self::default_path();
        Self::load_from(&path).unwrap_or_default()
    }

    /// Loads config from an explicit path, clamping values into valid bounds.
    pub fn load_from(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("Konnte Konfiguration nicht lesen: {}", path.display()))?;
        let mut cfg: Self = toml::from_str(&raw)
            .with_context(|| format!("Ungültiges TOML in {}", path.display()))?;
        cfg.sanitize();
        Ok(cfg)
    }

    /// Saves config to the default XDG path.
    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::default_path())
    }

    /// Saves config atomically to an explicit path.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        let mut copy = self.clone();
        copy.sanitize();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let encoded = toml::to_string_pretty(&copy)?;
        fs::write(path, encoded)?;
        Ok(())
    }

    /// Ensures numeric and string fields stay within valid boundaries.
    pub fn sanitize(&mut self) {
        self.max_history = self.max_history.clamp(1, 200);
        if matches!(
            self.carousel_position,
            CarouselPosition::Left | CarouselPosition::Right
        ) {
            self.carousel_position = CarouselPosition::Bottom;
        }
        if self.capture_shortcut.trim().is_empty() {
            self.capture_shortcut = "Print".to_string();
        }
        if self.history_shortcut.trim().is_empty() {
            self.history_shortcut = "<Super><Shift>h".to_string();
        }
        if self.filename_pattern.trim().is_empty() {
            self.filename_pattern = default_filename_pattern();
        }
    }

    /// Expands `filename_pattern` placeholders for an exported screenshot file.
    pub fn format_export_filename(
        &self,
        timestamp: DateTime<Utc>,
        width: u32,
        height: u32,
    ) -> String {
        let local = timestamp.with_timezone(&Local);
        let template = if self.filename_pattern.trim().is_empty() {
            default_filename_pattern()
        } else {
            self.filename_pattern.clone()
        };
        let mut name = template
            .replace("<year>", &local.format("%Y").to_string())
            .replace("<month>", &local.format("%m").to_string())
            .replace("<day>", &local.format("%d").to_string())
            .replace("<hour>", &local.format("%H").to_string())
            .replace("<minute>", &local.format("%M").to_string())
            .replace("<second>", &local.format("%S").to_string())
            .replace("<width>", &width.to_string())
            .replace("<height>", &height.to_string())
            .replace(['/', '\\', '\0'], "-");
        name = name.trim().to_string();
        if name.is_empty() {
            name = "screenshot".to_string();
        }
        let lower = name.to_ascii_lowercase();
        if !lower.ends_with(".png") && !lower.ends_with(".jpg") && !lower.ends_with(".jpeg") {
            name.push('.');
            name.push_str(self.export_format.extension());
        }
        name
    }

    /// Default folder for saved/exported screenshots (`~/Bilder/VeloShot` or `~/Pictures/VeloShot`).
    pub fn default_save_dir() -> PathBuf {
        dirs::picture_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("VeloShot")
    }

    /// Returns the preferred directory for "Speichern unter" / "Save As" file dialogs.
    pub fn effective_save_dir(&self) -> PathBuf {
        if let Some(dir) = &self.save_dir
            && !dir.as_os_str().is_empty()
        {
            return dir.clone();
        }
        Self::default_save_dir()
    }

    /// Ensures the effective save directory exists on disk before opening a file dialog.
    pub fn ensure_effective_save_dir(&self) -> PathBuf {
        let dir = self.effective_save_dir();
        let _ = fs::create_dir_all(&dir);
        dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_sanitize_config() {
        let tmp = std::env::temp_dir().join(format!(
            "jas-config-test-{}.toml",
            uuid::Uuid::new_v4()
        ));

        let mut cfg = AppConfig {
            language: Language::En,
            carousel_position: CarouselPosition::Right,
            max_history: 999,
            after_capture: AfterCaptureAction::OpenCarousel,
            auto_copy_clipboard: false,
            capture_shortcut: "  ".to_string(),
            history_shortcut: "<Ctrl><Alt>h".to_string(),
            save_dir: Some(PathBuf::from("/tmp")),
            filename_pattern: "Snap_<year>-<month>-<day>".to_string(),
            export_format: ExportFormat::Jpg,
            editor_icon_size: EditorIconSize::Large,
        };

        cfg.save_to(&tmp).expect("save config");
        let loaded = AppConfig::load_from(&tmp).expect("load config");
        let _ = fs::remove_file(&tmp);

        cfg.sanitize();
        assert_eq!(loaded.language, Language::En);
        assert_eq!(loaded.carousel_position, CarouselPosition::Bottom);
        assert_eq!(loaded.max_history, 200);
        assert_eq!(loaded.after_capture, AfterCaptureAction::OpenCarousel);
        assert!(!loaded.auto_copy_clipboard);
        assert_eq!(loaded.capture_shortcut, "Print");
        assert_eq!(loaded.history_shortcut, "<Ctrl><Alt>h");
        assert_eq!(loaded.filename_pattern, "Snap_<year>-<month>-<day>");
        assert_eq!(loaded.export_format, ExportFormat::Jpg);
        assert_eq!(loaded.editor_icon_size, EditorIconSize::Large);
    }

    #[test]
    fn default_save_dir_and_indices() {
        let def = AppConfig::default();
        assert_eq!(def.language, Language::Auto);
        assert_eq!(def.export_format, ExportFormat::Png);
        assert_eq!(def.editor_icon_size, EditorIconSize::Medium);
        assert!(def.effective_save_dir().ends_with("VeloShot"));

        for lang in Language::ALL {
            assert_eq!(Language::from_index(lang.to_index()), lang);
        }
        for pos in CarouselPosition::ALL {
            assert_eq!(CarouselPosition::from_index(pos.to_index()), pos);
        }
        for action in AfterCaptureAction::ALL {
            assert_eq!(AfterCaptureAction::from_index(action.to_index()), action);
        }
        for fmt in ExportFormat::ALL {
            assert_eq!(ExportFormat::from_index(fmt.to_index()), fmt);
        }
        for size in EditorIconSize::ALL {
            assert_eq!(EditorIconSize::from_index(size.to_index()), size);
        }
        assert!(EditorIconSize::Small.tool_px() < EditorIconSize::Medium.tool_px());
        assert!(EditorIconSize::Medium.tool_px() < EditorIconSize::Large.tool_px());
    }

    #[test]
    fn filename_pattern_expansion_and_sanitization() {
        let ts = DateTime::parse_from_rfc3339("2026-11-26T15:04:05Z")
            .unwrap()
            .with_timezone(&Utc);
        let cfg = AppConfig {
            filename_pattern: "Shot/<year>-<month>-<day>_<width>x<height>".to_string(),
            ..AppConfig::default()
        };
        let out = cfg.format_export_filename(ts, 1920, 1080);
        assert!(!out.contains('/'));
        assert!(out.contains("2026-11-26_1920x1080"));
        assert!(out.ends_with(".png"));

        let jpg_cfg = AppConfig {
            filename_pattern: "Shot_<year>-<month>".to_string(),
            export_format: ExportFormat::Jpg,
            ..AppConfig::default()
        };
        let jpg_out = jpg_cfg.format_export_filename(ts, 1920, 1080);
        assert!(jpg_out.ends_with(".jpg"));

        // Explicit .png suffix inside pattern is preserved even if default is JPG
        let explicit_cfg = AppConfig {
            filename_pattern: "Fixed_<year>.png".to_string(),
            export_format: ExportFormat::Jpg,
            ..AppConfig::default()
        };
        assert!(explicit_cfg.format_export_filename(ts, 100, 100).ends_with(".png"));

        let empty_cfg = AppConfig {
            filename_pattern: "   ".to_string(),
            ..AppConfig::default()
        };
        let def_out = empty_cfg.format_export_filename(ts, 800, 600);
        assert!(def_out.starts_with("2026-11-26_"));
        assert!(def_out.ends_with("_Screenshot.png"));
    }

    #[test]
    fn legacy_config_toml_loads_with_new_defaults() {
        let tmp = std::env::temp_dir().join(format!(
            "veloshot-legacy-cfg-{}.toml",
            uuid::Uuid::new_v4()
        ));
        let raw = r#"
language = "de"
max_history = 15
auto_copy_clipboard = true
"#;
        fs::write(&tmp, raw).unwrap();
        let loaded = AppConfig::load_from(&tmp).expect("load legacy config");
        let _ = fs::remove_file(&tmp);

        assert_eq!(loaded.language, Language::De);
        assert_eq!(loaded.max_history, 15);
        assert_eq!(loaded.export_format, ExportFormat::Png);
        assert_eq!(loaded.editor_icon_size, EditorIconSize::Medium);
    }
}
