//! The library on disk: one `library.json` beside a `covers/` folder of images.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: String,
    #[serde(default)]
    pub isbn: Option<String>,
    /// File name inside `covers/`, or none for a drawn placeholder cover.
    #[serde(default)]
    pub cover: Option<String>,
    /// 0 means not rated yet.
    #[serde(default)]
    pub rating: u8,
    #[serde(default)]
    pub notes: String,
    /// Milliseconds since the epoch.
    pub added_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prefs {
    /// Books per row: 1, 3 or 6.
    pub columns: u8,
    /// One of the sort keys the UI offers (see app.js SORTS).
    pub sort: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { columns: 3, sort: "added-desc".into() }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Library {
    #[serde(default)]
    pub books: Vec<Book>,
    #[serde(default)]
    pub prefs: Prefs,
}

pub struct Store {
    dir: PathBuf,
    pub lib: Library,
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl Store {
    /// Opens the library in [dir], starting empty when there is none yet or it cannot be read.
    pub fn open(dir: PathBuf) -> Store {
        let _ = fs::create_dir_all(dir.join("covers"));
        let lib = fs::read_to_string(dir.join("library.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Store { dir, lib }
    }

    pub fn covers_dir(&self) -> PathBuf {
        self.dir.join("covers")
    }

    /// Writes to a temp file first so a crash mid-save never leaves a half-written library.
    pub fn save(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.lib).map_err(|e| e.to_string())?;
        let tmp = self.dir.join("library.json.tmp");
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, self.dir.join("library.json")).map_err(|e| e.to_string())
    }

    pub fn new_id(&self) -> String {
        let base = format!("{:x}", now_ms());
        let mut id = base.clone();
        let mut n = 1;
        while self.lib.books.iter().any(|b| b.id == id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }

    pub fn book_mut(&mut self, id: &str) -> Result<&mut Book, String> {
        self.lib.books.iter_mut().find(|b| b.id == id).ok_or_else(|| "That book is gone.".to_string())
    }

    /// Saves image bytes as this book's cover. The name carries a timestamp so a replaced cover
    /// never shows the WebView's cached copy of the old one.
    pub fn write_cover(&self, id: &str, bytes: &[u8]) -> Result<String, String> {
        let ext = sniff_ext(bytes).ok_or("That file isn't an image we can show.")?;
        let name = format!("{id}-{:x}.{ext}", now_ms());
        fs::write(self.covers_dir().join(&name), bytes).map_err(|e| e.to_string())?;
        Ok(name)
    }

    pub fn remove_cover(&self, name: &str) {
        // Names only ever come from write_cover, but never follow one out of the folder.
        if !name.contains(['/', '\\']) && !name.contains("..") {
            let _ = fs::remove_file(self.covers_dir().join(name));
        }
    }
}

fn sniff_ext(b: &[u8]) -> Option<&'static str> {
    match b {
        [0xFF, 0xD8, 0xFF, ..] => Some("jpg"),
        [0x89, b'P', b'N', b'G', ..] => Some("png"),
        [b'G', b'I', b'F', b'8', ..] => Some("gif"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some("webp"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_ignores_missing_fields() {
        let dir = std::env::temp_dir().join(format!("shelf-test-{}", now_ms()));
        let mut s = Store::open(dir.clone());
        assert!(s.lib.books.is_empty());
        let id = s.new_id();
        s.lib.books.push(Book {
            id: id.clone(),
            title: "Dune".into(),
            author: "Frank Herbert".into(),
            isbn: None,
            cover: None,
            rating: 4,
            notes: "Spice".into(),
            added_at: now_ms(),
        });
        s.save().unwrap();
        let s2 = Store::open(dir.clone());
        assert_eq!(s2.lib.books[0].rating, 4);
        assert_eq!(s2.lib.prefs.columns, 3);
        // An id is never reused while the old book is still there.
        assert_ne!(s2.new_id(), id);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn sniffs_images() {
        assert_eq!(sniff_ext(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(sniff_ext(b"\x89PNG\r\n"), Some("png"));
        assert_eq!(sniff_ext(b"<html>"), None);
    }
}
