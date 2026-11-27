//! XDG-compliant screenshot history store with fast thumbnail caching.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Local, Utc};
use image::{ImageFormat, RgbaImage, imageops::FilterType};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::APP_DIR_NAME;

const THUMB_MAX_DIM: u32 = 320;

/// Metadata for a single stored screenshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: String,
    pub filename: String,
    pub created_at: DateTime<Utc>,
    pub width: u32,
    pub height: u32,
}

impl HistoryItem {
    /// Human-readable local timestamp for carousel captions.
    pub fn formatted_time(&self) -> String {
        self.created_at
            .with_timezone(&Local)
            .format("%d.%m.%Y %H:%M:%S")
            .to_string()
    }
}

/// Manages persisted screenshots, index metadata, and cached thumbnails.
#[derive(Debug, Clone)]
pub struct HistoryStore {
    data_dir: PathBuf,
    cache_dir: PathBuf,
}

impl HistoryStore {
    /// Creates a store using standard XDG data and cache directories.
    pub fn new_default() -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_DIR_NAME);
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_DIR_NAME);
        Self { data_dir, cache_dir }
    }

    /// Creates a store backed by custom directories (useful for tests).
    #[cfg(test)]
    pub fn new_with_dirs(data_dir: PathBuf, cache_dir: PathBuf) -> Self {
        Self { data_dir, cache_dir }
    }

    fn images_dir(&self) -> PathBuf {
        self.data_dir.join("images")
    }

    fn thumbnails_dir(&self) -> PathBuf {
        self.cache_dir.join("thumbnails")
    }

    fn index_path(&self) -> PathBuf {
        self.data_dir.join("history.json")
    }

    fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(self.images_dir())?;
        fs::create_dir_all(self.thumbnails_dir())?;
        Ok(())
    }

    /// Full-resolution PNG path for a history item.
    pub fn image_path(&self, item: &HistoryItem) -> PathBuf {
        self.images_dir().join(&item.filename)
    }

    /// Cached thumbnail PNG path for a history item.
    pub fn thumbnail_path(&self, item: &HistoryItem) -> PathBuf {
        self.thumbnails_dir().join(&item.filename)
    }

    /// Returns all history items ordered newest-first, filtering out missing image files.
    pub fn list(&self) -> Vec<HistoryItem> {
        let Ok(raw) = fs::read_to_string(self.index_path()) else {
            return Vec::new();
        };
        let Ok(mut items) = serde_json::from_str::<Vec<HistoryItem>>(&raw) else {
            return Vec::new();
        };
        items.retain(|item| self.image_path(item).is_file());
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        items
    }

    fn write_index(&self, items: &[HistoryItem]) -> Result<()> {
        self.ensure_dirs()?;
        let json = serde_json::to_string_pretty(items)?;
        fs::write(self.index_path(), json)?;
        Ok(())
    }

    fn write_image_and_thumb(&self, item: &HistoryItem, img: &RgbaImage) -> Result<()> {
        self.ensure_dirs()?;
        let full_path = self.image_path(item);
        img.save_with_format(&full_path, ImageFormat::Png)
            .with_context(|| format!("Konnte Bild nicht speichern: {}", full_path.display()))?;

        let thumb = create_thumbnail(img, THUMB_MAX_DIM);
        let thumb_path = self.thumbnail_path(item);
        thumb
            .save_with_format(&thumb_path, ImageFormat::Png)
            .with_context(|| {
                format!(
                    "Konnte Thumbnail nicht speichern: {}",
                    thumb_path.display()
                )
            })?;
        Ok(())
    }

    /// Ensures a thumbnail exists on disk for the given item, regenerating it if missing.
    pub fn ensure_thumbnail(&self, item: &HistoryItem) -> Result<PathBuf> {
        let thumb_path = self.thumbnail_path(item);
        if thumb_path.is_file() {
            return Ok(thumb_path);
        }
        let img = self.load_image(item)?;
        self.ensure_dirs()?;
        let thumb = create_thumbnail(&img, THUMB_MAX_DIM);
        thumb.save_with_format(&thumb_path, ImageFormat::Png)?;
        Ok(thumb_path)
    }

    /// Loads the full-resolution RGBA image for a history item.
    pub fn load_image(&self, item: &HistoryItem) -> Result<RgbaImage> {
        let path = self.image_path(item);
        let dyn_img = image::open(&path)
            .with_context(|| format!("Konnte Bild nicht laden: {}", path.display()))?;
        Ok(dyn_img.to_rgba8())
    }

    /// Adds a new screenshot to the history and prunes older items beyond `max_history`.
    pub fn add_image(&self, img: &RgbaImage, max_history: usize) -> Result<HistoryItem> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let filename = format!("{}_{}.png", now.format("%Y%m%d_%H%M%S"), &id[..8]);
        let item = HistoryItem {
            id,
            filename,
            created_at: now,
            width: img.width(),
            height: img.height(),
        };

        self.write_image_and_thumb(&item, img)?;
        let mut items = self.list();
        items.insert(0, item.clone());
        self.prune_and_save(&mut items, max_history)?;
        Ok(item)
    }

    /// Replaces the image data of an existing history item (e.g. after annotation edits).
    pub fn update_image(&self, id: &str, img: &RgbaImage) -> Result<HistoryItem> {
        let mut items = self.list();
        let pos = items
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| anyhow!("Historien-Eintrag nicht gefunden: {id}"))?;
        items[pos].width = img.width();
        items[pos].height = img.height();
        let updated = items[pos].clone();
        self.write_image_and_thumb(&updated, img)?;
        self.write_index(&items)?;
        Ok(updated)
    }

    /// Deletes a history item and its image + thumbnail files.
    pub fn delete(&self, id: &str) -> Result<()> {
        let mut items = self.list();
        if let Some(pos) = items.iter().position(|item| item.id == id) {
            let removed = items.remove(pos);
            let _ = fs::remove_file(self.image_path(&removed));
            let _ = fs::remove_file(self.thumbnail_path(&removed));
            self.write_index(&items)?;
        }
        Ok(())
    }

    /// Enforces `max_history` on existing stored screenshots.
    pub fn enforce_limit(&self, max_history: usize) -> Result<()> {
        let mut items = self.list();
        self.prune_and_save(&mut items, max_history)
    }

    fn prune_and_save(&self, items: &mut Vec<HistoryItem>, max_history: usize) -> Result<()> {
        let limit = max_history.max(1);
        while items.len() > limit {
            if let Some(removed) = items.pop() {
                let _ = fs::remove_file(self.image_path(&removed));
                let _ = fs::remove_file(self.thumbnail_path(&removed));
            }
        }
        self.write_index(items)
    }
}

fn create_thumbnail(img: &RgbaImage, max_dim: u32) -> RgbaImage {
    let w = img.width();
    let h = img.height();
    if w <= max_dim && h <= max_dim {
        return img.clone();
    }
    let scale = (max_dim as f32 / w.max(h) as f32).min(1.0);
    let tw = ((w as f32 * scale).round() as u32).max(1);
    let th = ((h as f32 * scale).round() as u32).max(1);
    image::imageops::resize(img, tw, th, FilterType::Triangle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn temp_store() -> (HistoryStore, PathBuf) {
        let root = std::env::temp_dir().join(format!("jas-history-{}", Uuid::new_v4()));
        let store = HistoryStore::new_with_dirs(root.join("data"), root.join("cache"));
        (store, root)
    }

    fn solid_img(w: u32, h: u32, color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba(color))
    }

    #[test]
    fn add_update_prune_and_delete_history() {
        let (store, root) = temp_store();

        let i1 = store.add_image(&solid_img(640, 480, [255, 0, 0, 255]), 2).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let i2 = store.add_image(&solid_img(800, 600, [0, 255, 0, 255]), 2).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let i3 = store.add_image(&solid_img(100, 50, [0, 0, 255, 255]), 2).unwrap();

        let listed = store.list();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, i3.id);
        assert_eq!(listed[1].id, i2.id);
        assert!(!store.image_path(&i1).exists());
        assert!(!store.thumbnail_path(&i1).exists());
        assert!(store.thumbnail_path(&i2).is_file());

        // Thumbnail dimensions should be capped at THUMB_MAX_DIM
        let thumb_i2 = image::open(store.thumbnail_path(&i2)).unwrap();
        assert!(thumb_i2.width() <= THUMB_MAX_DIM);
        assert!(thumb_i2.height() <= THUMB_MAX_DIM);

        // Update i3 dimensions
        let updated = store
            .update_image(&i3.id, &solid_img(200, 100, [10, 20, 30, 255]))
            .unwrap();
        assert_eq!(updated.width, 200);
        assert_eq!(updated.height, 100);

        store.delete(&i2.id).unwrap();
        assert_eq!(store.list().len(), 1);
        assert!(!store.image_path(&i2).exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_index_recovers_gracefully() {
        let (store, root) = temp_store();
        fs::create_dir_all(&store.data_dir).unwrap();
        fs::write(store.data_dir.join("index.json"), "{ invalid json").unwrap();

        assert!(store.list().is_empty());

        let added = store.add_image(&solid_img(64, 48, [12, 34, 56, 255]), 5).unwrap();
        let listed = store.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, added.id);

        let _ = fs::remove_dir_all(root);
    }
}
