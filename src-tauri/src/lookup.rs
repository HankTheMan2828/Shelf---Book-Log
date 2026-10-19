//! Finds a book's cover and ISBN on Open Library: free, keyless, with good ISBNs and covers.
//!
//! Open Library's title search needs the title spelled right, so when the exact search isn't
//! convincing Shelf asks again several ways at once: a plain keyword search, and one search per
//! main word of the title (a single misspelled word then can't sink the others). The local
//! ranking below then picks the real book out of everything that came back, typos included.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub title: String,
    pub author: String,
    pub isbn: Option<String>,
    pub cover_url: Option<String>,
    pub year: Option<i64>,
    pub source: &'static str,
}

/// Words not worth a search of their own.
const STOPWORDS: &[&str] = &["the", "and", "of", "a", "an", "to", "in", "for", "with", "on", "at", "by", "from"];

/// How many single-word rescue searches to send at most.
const RESCUE_WORDS: usize = 3;

/// Returns the best matches first, at most [limit]. Errors only when Open Library didn't answer.
pub async fn find(http: &reqwest::Client, title: &str, author: &str, limit: usize) -> Result<Vec<Match>, String> {
    let exact = search(http, vec![("title", title.to_string())], author).await;
    let mut all = exact.clone().unwrap_or_default();
    let first = rank(all.clone(), title, author, limit);
    if first.first().is_some_and(|m| convincing(m, title, author)) {
        return Ok(first);
    }

    // Not convincing: probably a typo. Ask the rescue searches all at once.
    let mut queries = vec![vec![("q", format!("{title} {author}").trim().to_string())]];
    queries.extend(rescue_words(title).into_iter().map(|w| vec![("title", w)]));
    let tasks: Vec<_> = queries
        .into_iter()
        .map(|q| {
            let (http, author) = (http.clone(), author.to_string());
            tokio::spawn(async move { search(&http, q, &author).await })
        })
        .collect();
    let mut answered = exact.is_ok();
    for t in tasks {
        if let Ok(Ok(found)) = t.await {
            answered = true;
            all.extend(found);
        }
    }
    if !answered {
        return Err("Couldn't reach Open Library.".into());
    }
    let mut out = rank(all, title, author, limit);

    // A rescued match can be a record without ISBNs; look its corrected title up once more.
    if let Some(best) = out.first_mut() {
        if best.isbn.is_none() {
            let main = main_title(&best.title);
            if let Ok(docs) = search(http, vec![("title", main.clone())], &last_name(&best.author)).await {
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

/// The title's main words, longest first (the longest are the least likely to be common).
fn rescue_words(title: &str) -> Vec<String> {
    let mut words: Vec<String> = norm(title)
        .split(' ')
        .filter(|w| w.len() >= 3 && !STOPWORDS.contains(w))
        .map(String::from)
        .collect();
    words.sort_by(|a, b| b.len().cmp(&a.len()));
    words.dedup();
    // A one-word title is already covered by the exact and keyword searches.
    if words.len() < 2 {
        return vec![];
    }
    words.truncate(RESCUE_WORDS);
    words
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

/// One Open Library search. [params] is the title or keyword part; the author narrows it when typed.
async fn search(http: &reqwest::Client, mut params: Vec<(&'static str, String)>, author: &str) -> Result<Vec<Match>, String> {
    params.push(("limit", "8".into()));
    params.push(("fields", "title,author_name,isbn,cover_i,first_publish_year".into()));
    if !author.is_empty() && params[0].0 == "title" {
        params.push(("author", author.to_string()));
    }
    let res: OlResponse = http
        .get("https://openlibrary.org/search.json")
        .query(&params)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(res.docs.into_iter().map(|d| Match {
        isbn: pick_isbn(d.isbn.iter().map(String::as_str)),
        // default=false makes a missing cover a 404 instead of a blank 1px image.
        cover_url: d.cover_i.filter(|&c| c > 0).map(|c| format!("https://covers.openlibrary.org/b/id/{c}-L.jpg?default=false")),
        title: d.title,
        author: d.author_name.into_iter().next().unwrap_or_default(),
        year: d.first_publish_year,
        source: "Open Library",
    }).collect())
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

/// Good enough to skip the rescue searches: the title as typed (give or take a subtitle), a
/// cover, and the author too when one was typed.
fn convincing(m: &Match, title: &str, author: &str) -> bool {
    let (t, a) = (title_key(title), norm(author));
    let title_ok = { let mt = title_key(&m.title); mt == t || mt.starts_with(&t) || t.starts_with(&mt) };
    let author_ok = a.is_empty() || score(m, &t, &a) - score(m, &t, "") >= 4.0;
    title_ok && author_ok && m.cover_url.is_some()
}

/// Scores each match on how well it fits what was typed, then folds together records of the same
/// book (the searches overlap), keeping a cover and an ISBN from whichever record has them.
fn rank(all: Vec<Match>, title: &str, author: &str, limit: usize) -> Vec<Match> {
    let (t, a) = (title_key(title), norm(author));
    let mut scored: Vec<(f32, usize, Match)> = all.into_iter().enumerate().map(|(i, m)| (score(&m, &t, &a), i, m)).collect();
    // Ties keep arrival order, which is Open Library's own relevance.
    scored.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
    let key = |m: &Match| (title_key(&m.title), last_name(&m.author));
    let mut out: Vec<Match> = Vec::new();
    for (_, _, m) in scored {
        if let Some(prev) = out.iter_mut().find(|p| key(p) == key(&m)) {
            if prev.cover_url.is_none() { prev.cover_url = m.cover_url }
            if prev.isbn.is_none() { prev.isbn = m.isbn }
            if prev.year.is_none() { prev.year = m.year }
            continue;
        }
        if out.len() < limit {
            out.push(m);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(title: &str, author: &str, cover: bool, isbn: bool) -> Match {
        Match {
            title: title.into(),
            author: author.into(),
            isbn: isbn.then(|| "9780441013593".into()),
            cover_url: cover.then(|| "https://x/c.jpg".into()),
            year: None,
            source: "Open Library",
        }
    }

    #[test]
    fn prefers_isbn13() {
        assert_eq!(pick_isbn(["0441013597", "978-0-441-01359-3"].into_iter()).as_deref(), Some("9780441013593"));
        assert_eq!(pick_isbn(["9785878600125", "9780547928227"].into_iter()).as_deref(), Some("9780547928227"));
        assert_eq!(pick_isbn(["ocm123"].into_iter()), None);
    }

    #[test]
    fn picks_the_rescue_words() {
        assert_eq!(rescue_words("The Great Gatsbey"), vec!["gatsbey", "great"]);
        assert_eq!(rescue_words("Pride and Predjudice"), vec!["predjudice", "pride"]);
        assert!(rescue_words("Dune").is_empty());
    }

    #[test]
    fn survives_typos_and_sinks_spinoffs() {
        let out = rank(
            vec![m("Atomic Habits Journal", "James Clear", true, true), m("Atomic Habits", "James Clear", true, true), m("automic habbit", "Mr X", false, false)],
            "atomic habbits",
            "",
            5,
        );
        assert_eq!(out[0].title, "Atomic Habits");
    }

    #[test]
    fn rescues_only_when_the_exact_search_falls_short() {
        assert!(convincing(&m("Dune", "Frank Herbert", true, true), "Dune", "Frank Herbert"));
        assert!(convincing(&m("The Midnight Library: A Novel", "Matt Haig", true, true), "midnight library", "haig"));
        assert!(!convincing(&m("Atomic Habits", "James Clear", true, true), "atomic habbits", ""));
        assert!(!convincing(&m("Dune", "Frank Herbert", false, true), "Dune", ""));
        assert!(!convincing(&m("Dune", "Jane Smith", true, true), "Dune", "Frank Herbert"));
    }

    /// Hits Open Library; run with `cargo test -- --ignored --nocapture` when online.
    #[tokio::test]
    #[ignore]
    async fn finds_real_books_even_misspelled() {
        let http = reqwest::Client::builder().user_agent("Shelf-test/0.1").build().unwrap();
        for (t, a, want) in [("atomic habbits", "", "atomic habits"), ("the great gatsbey", "", "great gatsby")] {
            let out = find(&http, t, a, 5).await.unwrap();
            println!("{t} -> {:?}", out.first().map(|m| (&m.title, &m.author, &m.isbn)));
            assert!(out[0].title.to_lowercase().contains(want), "{t} -> {}", out[0].title);
        }
    }
}
