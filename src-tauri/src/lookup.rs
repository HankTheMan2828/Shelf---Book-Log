//! Finds a book's cover and ISBN from public catalogs, asked at once:
//! - Google Books: forgiving of typos. Needs the API key baked in at build time through the
//!   SHELF_GOOGLE_BOOKS_KEY environment variable (build-apk.ps1 reads it from google-books.key);
//!   without one it runs on Google's shared keyless quota, which is usually used up.
//! - Open Library: good ISBNs and covers, but needs the title spelled right.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Source {
    #[serde(rename = "Google Books")]
    Google,
    #[serde(rename = "Open Library")]
    OpenLibrary,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub title: String,
    pub author: String,
    pub isbn: Option<String>,
    pub cover_url: Option<String>,
    pub year: Option<i64>,
    pub source: Source,
}

/// Returns the best matches first, at most [limit]. Errors only when no catalog answered.
pub async fn find(http: &reqwest::Client, title: &str, author: &str, limit: usize) -> Result<Vec<Match>, String> {
    let (gb, ol) = tokio::join!(google_books(http, title, author), open_library(http, title, author));
    if gb.is_err() && ol.is_err() {
        return Err("Couldn't reach the book catalogs.".into());
    }
    let mut all = gb.unwrap_or_default();
    all.extend(ol.unwrap_or_default());
    let mut out = rank(all, title, author, limit);

    // When Google is the only one to find the book (a typo sinks Open Library) it sometimes has no
    // ISBN. Ask Open Library again with Google's spelling to fill it in.
    if let Some(best) = out.first_mut() {
        if best.isbn.is_none() {
            let main = main_title(&best.title);
            if let Ok(docs) = open_library(http, &main, &last_name(&best.author)).await {
                if let Some(twin) = docs.into_iter().find(|d| similarity(&title_key(&d.title), &title_key(&main)) >= 0.85 && d.isbn.is_some()) {
                    best.isbn = twin.isbn;
                    if best.cover_url.is_none() { best.cover_url = twin.cover_url }
                    if best.year.is_none() { best.year = twin.year }
                }
            }
        }
    }
    Ok(out)
}

#[derive(Deserialize)]
struct OlResponse {
    #[serde(default)]
    docs: Vec<OlDoc>,
}

#[derive(Deserialize)]
struct OlDoc {
    #[serde(default)]
    title: String,
    #[serde(default)]
    author_name: Vec<String>,
    #[serde(default)]
    isbn: Vec<String>,
    cover_i: Option<i64>,
    first_publish_year: Option<i64>,
}

async fn get_json<T: serde::de::DeserializeOwned>(req: reqwest::RequestBuilder) -> Result<T, String> {
    req.send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

async fn open_library(http: &reqwest::Client, title: &str, author: &str) -> Result<Vec<Match>, String> {
    let mut q = vec![
        ("title", title),
        ("limit", "8"),
        ("fields", "title,author_name,isbn,cover_i,first_publish_year"),
    ];
    if !author.is_empty() {
        q.push(("author", author));
    }
    let res: OlResponse = get_json(http.get("https://openlibrary.org/search.json").query(&q)).await?;
    Ok(res.docs.into_iter().map(|d| Match {
        isbn: pick_isbn(d.isbn.iter().map(String::as_str)),
        // default=false makes a missing cover a 404 instead of a blank 1px image.
        cover_url: d.cover_i.filter(|&c| c > 0).map(|c| format!("https://covers.openlibrary.org/b/id/{c}-L.jpg?default=false")),
        title: d.title,
        author: d.author_name.into_iter().next().unwrap_or_default(),
        year: d.first_publish_year,
        source: Source::OpenLibrary,
    }).collect())
}

async fn google_books(http: &reqwest::Client, title: &str, author: &str) -> Result<Vec<Match>, String> {
    // A plain query, not intitle:/inauthor:, so Google's spelling correction gets to work.
    let q = format!("{title} {author}");
    let mut params = vec![("q", q.trim()), ("maxResults", "10"), ("printType", "books")];
    if let Some(key) = option_env!("SHELF_GOOGLE_BOOKS_KEY") {
        params.push(("key", key));
    }
    let res: Value = get_json(http.get("https://www.googleapis.com/books/v1/volumes").query(&params)).await?;
    let items = res["items"].as_array().cloned().unwrap_or_default();
    Ok(items.iter().filter_map(|it| {
        let v = &it["volumeInfo"];
        let ids = v["industryIdentifiers"].as_array().cloned().unwrap_or_default();
        Some(Match {
            title: main_title(v["title"].as_str()?),
            author: v["authors"][0].as_str().unwrap_or_default().to_string(),
            isbn: pick_isbn(ids.iter().filter_map(|i| i["identifier"].as_str())),
            cover_url: v["imageLinks"]["thumbnail"].as_str().map(google_cover),
            year: v["publishedDate"].as_str().and_then(|d| d.get(..4)).and_then(|y| y.parse().ok()),
            source: Source::Google,
        })
    }).collect())
}

/// Google's thumbnails come over http with a page-curl effect and at about 128px wide; `fife`
/// asks its image server for a cover-sized one instead.
fn google_cover(url: &str) -> String {
    url.replacen("http://", "https://", 1).replace("&edge=curl", "").replace("zoom=5", "zoom=1") + "&fife=w800-h1200"
}

/// The title before any subtitle.
fn main_title(t: &str) -> String {
    t.split(':').next().unwrap_or(t).trim().to_string()
}

/// A title for comparing: normalized, subtitle and leading article dropped, so "The Midnight
/// Library: A Novel" and "Midnight Library" count as the same book.
fn title_key(t: &str) -> String {
    let n = norm(&main_title(t));
    ["the ", "a ", "an "].iter().find_map(|a| n.strip_prefix(a)).unwrap_or(&n).to_string()
}

/// Prefers an English-language ISBN-13 (groups 0 and 1), then any ISBN-13, then an ISBN-10.
/// Open Library lists every edition's ISBN for a work, so without this a translation often wins.
/// Anything else (an OCLC number, junk) is skipped.
pub fn pick_isbn<'a>(ids: impl Iterator<Item = &'a str>) -> Option<String> {
    let clean: Vec<String> = ids
        .map(|s| s.chars().filter(|c| c.is_ascii_digit() || *c == 'X' || *c == 'x').collect::<String>().to_uppercase())
        .collect();
    let is13 = |s: &&String| s.len() == 13 && (s.starts_with("978") || s.starts_with("979"));
    clean.iter().find(|s| is13(s) && (s.starts_with("9780") || s.starts_with("9781")))
        .or_else(|| clean.iter().find(is13))
        .or_else(|| clean.iter().find(|s| s.len() == 10 && (s.starts_with('0') || s.starts_with('1'))))
        .or_else(|| clean.iter().find(|s| s.len() == 10))
        .cloned()
}

fn norm(s: &str) -> String {
    s.chars()
        .filter_map(|c| if c.is_alphanumeric() { Some(c.to_ascii_lowercase()) } else if c.is_whitespace() || c == '-' { Some(' ') } else { None })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn last_name(author: &str) -> String {
    norm(author).split(' ').last().unwrap_or("").to_string()
}

/// 1.0 for equal strings, falling toward 0.0 with each edit (Levenshtein distance).
fn similarity(a: &str, b: &str) -> f32 {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + (ca != cb) as usize).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    1.0 - prev[b.len()] as f32 / longest as f32
}

/// Companion books that crowd the results for anything popular.
const SPINOFFS: &[&str] = &[
    "summary", "study guide", "workbook", "conversation starters", "discussion", "analysis",
    "sparknotes", "cliffsnotes", "journal", "calendar", "coloring", "box set", "collection set",
    "trivia", "quiz", "notebook", "planner", "adaptation",
];

/// [typed_title] is already a [title_key].
fn score(m: &Match, typed_title: &str, typed_author: &str) -> f32 {
    let mt = title_key(&m.title);
    let full = norm(&m.title);
    let mut s = if mt == typed_title {
        8.0
    } else if mt.starts_with(typed_title) || typed_title.starts_with(&mt) {
        6.0
    } else {
        // Typos land here: "atomic habbits" is still ~0.93 like "atomic habits".
        let sim = similarity(&mt, typed_title);
        if sim >= 0.6 { sim * 7.0 } else { 0.0 }
    };
    if !typed_author.is_empty() {
        let (want, got) = (last_name(typed_author), norm(&m.author));
        if got.split(' ').any(|w| w == want || similarity(w, &want) >= 0.8) {
            s += 4.0;
        }
    }
    if SPINOFFS.iter().any(|w| full.contains(w) && !typed_title.contains(w)) {
        s -= 6.0;
    }
    if m.cover_url.is_some() { s += 2.0 }
    if m.isbn.is_some() { s += 1.0 }
    s
}

fn cover_rank(s: Source) -> u8 {
    // Google's covers are the current editions; Open Library's are hit-and-miss (old printings).
    match s { Source::Google => 2, Source::OpenLibrary => 1 }
}

/// Scores each match on how well it fits what was typed, then folds together the same book found
/// in several catalogs, keeping the best cover, an ISBN, and Open Library's first-published year.
fn rank(all: Vec<Match>, title: &str, author: &str, limit: usize) -> Vec<Match> {
    let (t, a) = (title_key(title), norm(author));
    let mut scored: Vec<(f32, usize, Match)> = all.into_iter().enumerate().map(|(i, m)| (score(&m, &t, &a), i, m)).collect();
    // Ties keep arrival order (Google, then Open Library), each catalog's own relevance.
    scored.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
    let key = |m: &Match| (title_key(&m.title), last_name(&m.author));
    let mut out: Vec<(Match, Source)> = Vec::new(); // the match, and where its cover came from
    for (_, _, m) in scored {
        if let Some((prev, cover_from)) = out.iter_mut().find(|(p, _)| key(p) == key(&m)) {
            if m.cover_url.is_some() && (prev.cover_url.is_none() || cover_rank(m.source) > cover_rank(*cover_from)) {
                prev.cover_url = m.cover_url;
                *cover_from = m.source;
            }
            if prev.isbn.is_none() { prev.isbn = m.isbn }
            if m.source == Source::OpenLibrary && m.year.is_some() { prev.year = m.year } else if prev.year.is_none() { prev.year = m.year }
            continue;
        }
        if out.len() == limit { continue }
        let from = m.source;
        out.push((m, from));
    }
    out.into_iter().map(|(m, _)| m).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(title: &str, author: &str, cover: bool, isbn: bool, source: Source) -> Match {
        Match {
            title: title.into(),
            author: author.into(),
            isbn: isbn.then(|| "9780441013593".into()),
            cover_url: cover.then(|| format!("https://x/{source:?}.jpg")),
            year: None,
            source,
        }
    }

    #[test]
    fn prefers_isbn13() {
        assert_eq!(pick_isbn(["0441013597", "978-0-441-01359-3"].into_iter()).as_deref(), Some("9780441013593"));
        assert_eq!(pick_isbn(["9785878600125", "9780547928227"].into_iter()).as_deref(), Some("9780547928227"));
        assert_eq!(pick_isbn(["044101359X"].into_iter()).as_deref(), Some("044101359X"));
        assert_eq!(pick_isbn(["ocm123"].into_iter()), None);
    }

    #[test]
    fn survives_typos_and_sinks_spinoffs() {
        let out = rank(
            vec![
                m("Atomic Habits Workbook", "James Clear", true, true, Source::Google),
                m("Summary of Atomic Habits", "Easy Reads", true, false, Source::Google),
                m("Atomic Habits", "James Clear", true, false, Source::Google),
            ],
            "atomic habbits",
            "",
            5,
        );
        assert_eq!(out[0].title, "Atomic Habits");
    }

    #[test]
    fn merges_catalogs_keeping_the_best_parts() {
        let mut ol = m("Dune", "Frank Herbert", true, true, Source::OpenLibrary);
        ol.year = Some(1965);
        let out = rank(
            vec![m("Dune", "Frank Herbert", true, false, Source::Google), ol, m("Dune Messiah", "Frank Herbert", true, true, Source::OpenLibrary)],
            "dune",
            "frank herbert",
            5,
        );
        assert_eq!(out.iter().filter(|x| x.title == "Dune").count(), 1);
        assert_eq!(out[0].cover_url.as_deref(), Some("https://x/Google.jpg"));
        assert!(out[0].isbn.is_some());
        assert_eq!(out[0].year, Some(1965));
    }

    #[test]
    fn similarity_is_sane() {
        assert_eq!(similarity("abc", "abc"), 1.0);
        assert!(similarity("atomic habbits", "atomic habits") > 0.9);
        assert!(similarity("dune", "emma") < 0.5);
    }

    /// Hits the real catalogs; run with `cargo test -- --ignored --nocapture` when online.
    #[tokio::test]
    #[ignore]
    async fn finds_real_books_even_misspelled() {
        let http = reqwest::Client::builder().user_agent("Shelf-test/0.1").build().unwrap();
        for (t, a, want) in [("The Hobbit", "Tolkien", "hobbit"), ("atomic habbits", "", "atomic habits"), ("midnite libary", "", "midnight library")] {
            let out = find(&http, t, a, 5).await.unwrap();
            println!("{t}: {:#?}", out.first());
            assert!(out[0].title.to_lowercase().contains(want), "{t} -> {}", out[0].title);
            assert!(out[0].cover_url.is_some());
        }
    }
}
