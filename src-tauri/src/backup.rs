//! One-file backups: every book with its rating, notes and cover (as base64), so a library can
//! move between phones or survive a reinstall. Restoring only ever adds books.

use crate::store::{now_ms, Book, Store};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;

const FORMAT: &str = "shelf-backup";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Backup {
    format: String,
    version: u32,
    exported_at: u64,
    books: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    #[serde(flatten)]
    book: Book,
    #[serde(default)]
    cover_data: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Restored {
    pub added: usize,
    pub skipped: usize,
}

pub fn export(store: &Store) -> Result<String, String> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let books = store.lib.books.iter().map(|b| Entry {
        book: b.clone(),
        cover_data: b.cover.as_ref().and_then(|c| fs::read(store.covers_dir().join(c)).ok()).map(|bytes| b64.encode(bytes)),
    }).collect();
    let backup = Backup { format: FORMAT.into(), version: 1, exported_at: now_ms(), books };
    serde_json::to_string(&backup).map_err(|e| e.to_string())
}

fn same_book(a: &Book, b: &Book) -> bool {
    let k = |s: &str| s.trim().to_lowercase();
    k(&a.title) == k(&b.title) && k(&a.author) == k(&b.author)
}

/// Adds the books in [json] that aren't already on the shelf (same title and author).
pub fn import(store: &mut Store, json: &str) -> Result<Restored, String> {
    let backup: Backup = serde_json::from_str(json).map_err(|_| "That file isn't a Shelf backup.".to_string())?;
    if backup.format != FORMAT {
        return Err("That file isn't a Shelf backup.".into());
    }
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut out = Restored { added: 0, skipped: 0 };
    for Entry { mut book, cover_data } in backup.books {
        if store.lib.books.iter().any(|b| same_book(b, &book)) {
            out.skipped += 1;
            continue;
        }
        book.id = store.new_id();
        book.rating = book.rating.min(5);
        book.cover = cover_data
            .and_then(|d| b64.decode(d).ok())
            .and_then(|bytes| store.write_cover(&book.id, &bytes).ok());
        store.lib.books.push(book);
        out.added += 1;
    }
    store.save()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(title: &str) -> Book {
        Book { id: String::new(), title: title.into(), author: "A".into(), isbn: None, cover: None, rating: 3, notes: "n".into(), added_at: 1 }
    }

    #[test]
    fn round_trips_and_skips_what_is_already_there() {
        let base = std::env::temp_dir().join(format!("shelf-backup-test-{}", now_ms()));
        let mut a = Store::open(base.join("a"));
        let mut d = book("Dune");
        d.id = a.new_id();
        d.cover = a.write_cover(&d.id, &[0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3]).ok();
        a.lib.books.push(d);
        let json = export(&a).unwrap();

        let mut b = Store::open(base.join("b"));
        let mut e = book("Emma");
        e.id = b.new_id();
        b.lib.books.push(e);
        assert_eq!(import(&mut b, &json).unwrap(), Restored { added: 1, skipped: 0 });
        assert_eq!(import(&mut b, &json).unwrap(), Restored { added: 0, skipped: 1 });
        let dune = b.lib.books.iter().find(|x| x.title == "Dune").unwrap();
        assert_eq!((dune.rating, dune.notes.as_str()), (3, "n"));
        assert!(b.covers_dir().join(dune.cover.as_ref().unwrap()).exists());
        assert!(import(&mut b, "{\"hello\":1}").is_err());
        let _ = fs::remove_dir_all(base);
    }
}
