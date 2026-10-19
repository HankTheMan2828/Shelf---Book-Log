//! Shelf's Rust core: the library on disk, the catalog lookup and cover downloads. The UI in
//! `../src` calls these commands over Tauri's IPC.

mod backup;
mod lookup;
mod store;

use base64::Engine;
use serde::Serialize;
use std::io::Write;
use std::sync::Mutex;
use std::time::Duration;
use store::{Book, Prefs, Store};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};

struct App {
    store: Mutex<Store>,
    http: reqwest::Client,
}

type Res<T> = Result<T, String>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    books: Vec<Book>,
    prefs: Prefs,
    covers_dir: String,
}

#[tauri::command]
fn load(app: State<App>) -> Snapshot {
    let s = app.store.lock().unwrap();
    Snapshot {
        books: s.lib.books.clone(),
        prefs: s.lib.prefs.clone(),
        covers_dir: s.covers_dir().to_string_lossy().into_owned(),
    }
}

#[tauri::command]
async fn lookup(app: State<'_, App>, title: String, author: String) -> Res<Vec<lookup::Match>> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Type the book's title first.".into());
    }
    lookup::find(&app.http, title, author.trim(), 6).await
}

/// Images picked on the phone arrive as base64 (already shrunk by the UI); found covers as a URL.
fn decode(data: &str) -> Res<Vec<u8>> {
    let raw = data.split_once("base64,").map(|(_, b)| b).unwrap_or(data);
    base64::engine::general_purpose::STANDARD.decode(raw.trim()).map_err(|_| "That image couldn't be read.".to_string())
}

async fn download(http: &reqwest::Client, url: &str) -> Option<Vec<u8>> {
    let r = http.get(url).send().await.ok()?.error_for_status().ok()?;
    let bytes = r.bytes().await.ok()?;
    // Some catalogs answer with a tiny "no cover" image instead of an error.
    (bytes.len() > 1024).then(|| bytes.to_vec())
}

fn clean_isbn(isbn: Option<String>) -> Option<String> {
    isbn.map(|s| s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase())
        .filter(|s| !s.is_empty())
}

#[tauri::command]
async fn add_book(
    app: State<'_, App>,
    title: String,
    author: String,
    isbn: Option<String>,
    cover_url: Option<String>,
    cover_data: Option<String>,
) -> Res<Book> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err("A book needs a title.".into());
    }
    let bytes = match (&cover_data, &cover_url) {
        (Some(d), _) => Some(decode(d)?),
        (None, Some(u)) => download(&app.http, u).await,
        _ => None,
    };
    let mut s = app.store.lock().unwrap();
    let id = s.new_id();
    // A found cover that won't download still adds the book, with a drawn cover.
    let cover = match bytes {
        Some(b) => s.write_cover(&id, &b).ok(),
        None => None,
    };
    let book = Book {
        id,
        title,
        author: author.trim().to_string(),
        isbn: clean_isbn(isbn),
        cover,
        rating: 0,
        notes: String::new(),
        added_at: store::now_ms(),
    };
    s.lib.books.push(book.clone());
    s.save()?;
    Ok(book)
}

#[tauri::command]
fn set_rating(app: State<App>, id: String, rating: u8) -> Res<()> {
    let mut s = app.store.lock().unwrap();
    s.book_mut(&id)?.rating = rating.min(5);
    s.save()
}

#[tauri::command]
fn set_notes(app: State<App>, id: String, notes: String) -> Res<()> {
    let mut s = app.store.lock().unwrap();
    s.book_mut(&id)?.notes = notes;
    s.save()
}

#[tauri::command]
fn set_details(app: State<App>, id: String, title: String, author: String, isbn: Option<String>) -> Res<Book> {
    let mut s = app.store.lock().unwrap();
    let b = s.book_mut(&id)?;
    if !title.trim().is_empty() {
        b.title = title.trim().to_string();
    }
    b.author = author.trim().to_string();
    b.isbn = clean_isbn(isbn);
    let out = b.clone();
    s.save()?;
    Ok(out)
}

#[tauri::command]
fn set_cover(app: State<App>, id: String, cover_data: String) -> Res<Book> {
    let bytes = decode(&cover_data)?;
    let mut s = app.store.lock().unwrap();
    let name = s.write_cover(&id, &bytes)?;
    let old = s.book_mut(&id)?.cover.replace(name);
    if let Some(old) = old {
        s.remove_cover(&old);
    }
    let out = s.book_mut(&id)?.clone();
    s.save()?;
    Ok(out)
}

#[tauri::command]
fn delete_book(app: State<App>, id: String) -> Res<()> {
    let mut s = app.store.lock().unwrap();
    let Some(i) = s.lib.books.iter().position(|b| b.id == id) else { return Ok(()) };
    let b = s.lib.books.remove(i);
    if let Some(c) = b.cover {
        s.remove_cover(&c);
    }
    s.save()
}

#[tauri::command]
fn set_prefs(app: State<App>, prefs: Prefs) -> Res<()> {
    let mut s = app.store.lock().unwrap();
    s.lib.prefs = prefs;
    s.save()
}

/// Writes a backup to [path], a file the user picked in the save dialog (a content:// URI on
/// Android, which only the fs plugin can open). Returns how many books it holds.
#[tauri::command]
fn export_backup(handle: AppHandle, app: State<App>, path: FilePath) -> Res<usize> {
    let (json, count) = {
        let s = app.store.lock().unwrap();
        (backup::export(&s)?, s.lib.books.len())
    };
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    let mut file = handle.fs().open(path, opts).map_err(|e| format!("Couldn't save the backup ({e})."))?;
    file.write_all(json.as_bytes()).map_err(|e| format!("Couldn't save the backup ({e})."))?;
    Ok(count)
}

#[tauri::command]
fn import_backup(handle: AppHandle, app: State<App>, path: FilePath) -> Res<backup::Restored> {
    let json = handle.fs().read_to_string(path).map_err(|e| format!("Couldn't open that file ({e})."))?;
    backup::import(&mut app.store.lock().unwrap(), &json)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let http = reqwest::Client::builder()
                // Open Library asks callers to identify themselves.
                .user_agent(concat!("Shelf/", env!("CARGO_PKG_VERSION"), " (personal book logger)"))
                .timeout(Duration::from_secs(15))
                .build()?;
            app.manage(App { store: Mutex::new(Store::open(dir)), http });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load, lookup, add_book, set_rating, set_notes, set_details, set_cover, delete_book, set_prefs,
            export_backup, import_backup
        ])
        .run(tauri::generate_context!())
        .expect("error while running Shelf");
}
