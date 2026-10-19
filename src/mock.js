// Stands in for the Rust core when the UI is opened in a plain browser (for previewing the
// design). Inside the app window.__TAURI__ exists and this does nothing. The lookup is a short
// port of src-tauri/src/lookup.rs, so the preview finds the same books the app does.
(() => {
  if (window.__TAURI__) return;
  const KEY = "shelf-mock";
  let lib;
  try { lib = JSON.parse(localStorage.getItem(KEY)); } catch {}
  lib = lib || { books: [], prefs: { columns: 3, sort: "added-desc" } };
  const save = () => { try { localStorage.setItem(KEY, JSON.stringify(lib)); } catch {} };
  // Like the real IPC, every answer is a copy: the UI must never share the mock's own arrays.
  const copy = (v) => (v === undefined ? v : structuredClone(v));
  const book = (id) => {
    const b = lib.books.find((x) => x.id === id);
    if (!b) throw "That book is gone.";
    return b;
  };

  // ---------- Lookup (mirrors lookup.rs) ----------
  const norm = (s) => (s || "").toLowerCase().replace(/[-\s]+/g, " ").replace(/[^\p{L}\p{N} ]/gu, "").trim().replace(/ +/g, " ");
  const mainTitle = (t) => t.split(":")[0].trim();
  const titleKey = (t) => norm(mainTitle(t)).replace(/^(the|a|an) /, "");
  const lastName = (a) => norm(a).split(" ").pop() || "";
  function similarity(a, b) {
    const n = Math.max(a.length, b.length);
    if (!n) return 1;
    let prev = [...Array(b.length + 1).keys()];
    for (let i = 0; i < a.length; i++) {
      const cur = [i + 1];
      for (let j = 0; j < b.length; j++) cur[j + 1] = Math.min(prev[j] + (a[i] !== b[j]), prev[j + 1] + 1, cur[j] + 1);
      prev = cur;
    }
    return 1 - prev[b.length] / n;
  }
  const pickIsbn = (ids = []) => {
    const c = ids.map((s) => String(s).replace(/[^0-9Xx]/g, "").toUpperCase());
    const is13 = (s) => s.length === 13 && /^97[89]/.test(s);
    return c.find((s) => is13(s) && /^978[01]/.test(s)) || c.find(is13) || c.find((s) => s.length === 10) || null;
  };
  const getJson = async (url) => {
    const r = await fetch(url);
    if (!r.ok) throw new Error(String(r.status));
    return r.json();
  };

  // One Open Library search: [params] is the title or keyword part; the author narrows a title search.
  async function search(params, author) {
    const q = new URLSearchParams({ ...params, limit: "8", fields: "title,author_name,isbn,cover_i,first_publish_year" });
    if (author && params.title) q.set("author", author);
    const { docs = [] } = await getJson("https://openlibrary.org/search.json?" + q);
    return docs.map((d) => ({
      title: d.title, author: (d.author_name || [])[0] || "", isbn: pickIsbn(d.isbn),
      coverUrl: d.cover_i > 0 ? `https://covers.openlibrary.org/b/id/${d.cover_i}-L.jpg?default=false` : null,
      year: d.first_publish_year || null, source: "Open Library",
    }));
  }

  const STOPWORDS = ["the", "and", "of", "a", "an", "to", "in", "for", "with", "on", "at", "by", "from"];
  function rescueWords(title) {
    const words = [...new Set(norm(title).split(" ").filter((w) => w.length >= 3 && !STOPWORDS.includes(w)))].sort((a, b) => b.length - a.length);
    return words.length < 2 ? [] : words.slice(0, 3);
  }

  const SPINOFFS = ["summary", "study guide", "workbook", "conversation starters", "discussion", "analysis", "sparknotes", "cliffsnotes", "journal", "calendar", "coloring", "box set", "collection set", "trivia", "quiz", "notebook", "planner", "adaptation"];
  function score(m, t, a) {
    const mt = titleKey(m.title), full = norm(m.title);
    let s = mt === t ? 8 : mt.startsWith(t) || t.startsWith(mt) ? 6 : (() => { const x = similarity(mt, t); return x >= 0.6 ? x * 7 : 0; })();
    if (a) {
      const want = lastName(a);
      if (norm(m.author).split(" ").some((w) => w === want || similarity(w, want) >= 0.8)) s += 4;
    }
    if (SPINOFFS.some((w) => full.includes(w) && !t.includes(w))) s -= 6;
    return s + (m.coverUrl ? 2 : 0) + (m.isbn ? 1 : 0);
  }
  function convincing(m, title, author) {
    const t = titleKey(title), a = norm(author), mt = titleKey(m.title);
    const titleOk = mt === t || mt.startsWith(t) || t.startsWith(mt);
    return titleOk && (!a || score(m, t, a) - score(m, t, "") >= 4) && !!m.coverUrl;
  }
  function rank(all, title, author, limit) {
    const t = titleKey(title), a = norm(author);
    const scored = all.map((m, i) => [score(m, t, a), i, m]).sort((x, y) => y[0] - x[0] || x[1] - y[1]);
    const key = (m) => titleKey(m.title) + "|" + lastName(m.author);
    const out = [];
    for (const [, , m] of scored) {
      const prev = out.find((p) => key(p) === key(m));
      if (prev) { prev.coverUrl ||= m.coverUrl; prev.isbn ||= m.isbn; prev.year ||= m.year; }
      else if (out.length < limit) out.push({ ...m });
    }
    return out;
  }

  async function lookup({ title, author }) {
    let all = [], answered = false;
    try { all = await search({ title }, author); answered = true; } catch {}
    const first = rank(all, title, author, 6);
    if (first[0] && convincing(first[0], title, author)) return first;
    const rescues = [{ q: `${title} ${author}`.trim() }, ...rescueWords(title).map((w) => ({ title: w }))];
    for (const r of await Promise.allSettled(rescues.map((p) => search(p, author)))) {
      if (r.status === "fulfilled") { answered = true; all = all.concat(r.value); }
    }
    if (!answered) throw "Couldn't reach Open Library.";
    const out = rank(all, title, author, 6);
    const best = out[0];
    if (best && !best.isbn) {
      try {
        const main = mainTitle(best.title);
        const twin = (await search({ title: main }, lastName(best.author))).find((d) => d.isbn && similarity(titleKey(d.title), titleKey(main)) >= 0.85);
        if (twin) { best.isbn = twin.isbn; best.coverUrl ||= twin.coverUrl; best.year ||= twin.year; }
      } catch {}
    }
    return out;
  }
  // ---------- Commands ----------
  const cmds = {
    load: () => ({ books: lib.books, prefs: lib.prefs, coversDir: "mock" }),
    lookup,
    add_book: ({ title, author, isbn, coverUrl, coverData }) => {
      let id = Date.now().toString(16);
      while (lib.books.some((b) => b.id === id)) id += "x";
      const b = { id, title, author, isbn, cover: coverData || coverUrl || null, rating: 0, notes: "", addedAt: Date.now() };
      lib.books.push(b);
      save();
      return b;
    },
    set_rating: ({ id, rating }) => { book(id).rating = rating; save(); },
    set_notes: ({ id, notes }) => { book(id).notes = notes; save(); },
    set_details: ({ id, title, author, isbn }) => { Object.assign(book(id), { title, author, isbn }); save(); return book(id); },
    set_cover: ({ id, coverData }) => { book(id).cover = coverData; save(); return book(id); },
    delete_book: ({ id }) => { lib.books = lib.books.filter((b) => b.id !== id); save(); },
    set_prefs: ({ prefs }) => { lib.prefs = copy(prefs); save(); },
    // The preview's covers are URLs or data URLs, so a backup here just carries them as they are.
    import_backup_text: ({ json }) => {
      let backup;
      try { backup = JSON.parse(json); } catch {}
      if (backup?.format !== "shelf-backup") throw "That file isn't a Shelf backup.";
      const same = (a, b) => a.title.trim().toLowerCase() === b.title.trim().toLowerCase() && a.author.trim().toLowerCase() === b.author.trim().toLowerCase();
      let added = 0, skipped = 0;
      for (const { coverData, ...b } of backup.books) {
        if (lib.books.some((x) => same(x, b))) { skipped++; continue; }
        lib.books.push({ ...b, id: Date.now().toString(16) + "-" + added, cover: coverData || b.cover || null });
        added++;
      }
      save();
      return { added, skipped };
    },
  };

  window.ShelfMock = {
    invoke: async (cmd, args = {}) => copy(await cmds[cmd](args)),
    src: (p) => p.slice("mock/".length),
    downloadBackup() {
      const json = JSON.stringify({ format: "shelf-backup", version: 1, exportedAt: Date.now(), books: lib.books });
      const a = document.createElement("a");
      a.href = URL.createObjectURL(new Blob([json], { type: "application/json" }));
      a.download = `shelf-backup-${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 1000);
      return lib.books.length;
    },
  };
})();
