// Shelf's UI. The Rust core (src-tauri) owns the data; this file draws it and sends edits back.
(() => {
  "use strict";

  const T = window.__TAURI__;
  const api = T ? { invoke: T.core.invoke, src: T.core.convertFileSrc } : window.ShelfMock;
  const $ = (s, el = document) => el.querySelector(s);
  const $$ = (s, el = document) => [...el.querySelectorAll(s)];

  const state = { books: [], prefs: { columns: 3, sort: "added-desc" }, coversDir: "" };

  // ---------- Small helpers ----------
  const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  const pad = (n) => String(n).padStart(2, "0");
  const byId = (id) => state.books.find((b) => b.id === id);
  const errText = (e) => (typeof e === "string" ? e : e?.message || "Something went wrong.");

  let toastTimer;
  function toast(msg) {
    const t = $("#toast");
    t.textContent = msg;
    t.classList.add("on");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => t.classList.remove("on"), 2600);
  }

  // A pixel star on an 11-cell grid, in the style of Rise's sprites.
  const STAR = "M5 0h1v2h-1zM4 2h3v2h-3zM0 4h11v1h-11zM1 5h9v1h-9zM2 6h7v1h-7zM3 7h5v1h-5zM2 8h3v1h-3zM6 8h3v1h-3zM2 9h2v1h-2zM7 9h2v1h-2zM1 10h2v1h-2zM8 10h2v1h-2z";
  const star = (on, cls = "", style = "") =>
    `<svg viewBox="0 0 11 11" shape-rendering="crispEdges" class="${on ? "on" : ""} ${cls}" style="${style}" aria-hidden="true"><path d="${STAR}"/></svg>`;
  const stars = (n) => [1, 2, 3, 4, 5].map((i) => star(i <= n)).join("");

  // Drawn jackets for books without a cover: Rise's type colors plus a few deep tones.
  const JACKETS = [
    ["#FFAF01", "#101013"], ["#FA5111", "#101013"], ["#FEC63A", "#101013"], ["#6CC287", "#101013"],
    ["#2A2A2E", "#F2F1E8"], ["#E4E2D6", "#101013"], ["#8E2F28", "#F2F1E8"], ["#2F5F5A", "#F2F1E8"],
  ];
  function jacket(title) {
    let h = 0;
    for (const c of title) h = (h * 31 + c.charCodeAt(0)) >>> 0;
    return JACKETS[h % JACKETS.length];
  }
  function drawnCover(title, author) {
    const [bg, ink] = jacket(title || "?");
    return `<div class="drawn" style="--jacket:${bg};--ink:${ink}">
      <div class="drawn-title">${esc(title)}</div><div class="drawn-rule"></div>
      <div class="drawn-author">${esc(author)}</div><span class="pix pix-book-sm"></span></div>`;
  }
  /** A cover from [src], falling back to a drawn one if there is none or it fails to load. */
  function coverHtml(src, title, author) {
    const inner = src
      ? `<img src="${esc(src)}" alt="" decoding="async" data-t="${esc(title)}" data-a="${esc(author)}">`
      : drawnCover(title, author);
    return `<div class="cover">${inner}</div>`;
  }
  const bookCoverSrc = (b) => {
    if (!b.cover) return null;
    const sep = state.coversDir.includes("\\") ? "\\" : "/";
    return api.src(state.coversDir + sep + b.cover);
  };
  const bookCover = (b) => coverHtml(bookCoverSrc(b), b.title, b.author);

  document.addEventListener("error", (e) => {
    const img = e.target;
    if (img.tagName === "IMG" && img.dataset.t !== undefined) {
      img.parentElement.innerHTML = drawnCover(img.dataset.t, img.dataset.a);
    }
  }, true);

  // ---------- Sorting ----------
  const collator = new Intl.Collator(undefined, { sensitivity: "base", numeric: true });
  const titleKey = (t) => t.replace(/^(the|a|an)\s+/i, "");
  const lastName = (a) => (a || "").trim().split(/\s+/).pop() || "";
  const byTitle = (a, b) => collator.compare(titleKey(a.title), titleKey(b.title));
  const SORTS = {
    "added-desc": (a, b) => b.addedAt - a.addedAt,
    "added-asc": (a, b) => a.addedAt - b.addedAt,
    title: byTitle,
    author: (a, b) => collator.compare(lastName(a.author), lastName(b.author)) || collator.compare(a.author, b.author) || byTitle(a, b),
    "rating-desc": (a, b) => b.rating - a.rating || b.addedAt - a.addedAt,
    // Unrated books go last, not first.
    "rating-asc": (a, b) => (a.rating || 6) - (b.rating || 6) || b.addedAt - a.addedAt,
  };
  const sorted = () => [...state.books].sort(SORTS[state.prefs.sort] || SORTS["added-desc"]);

  // ---------- The shelf ----------
  const grid = $("#grid");

  function cardHtml(b, i, animate) {
    const label = `${b.title}${b.author ? ", " + b.author : ""}. ${b.rating ? b.rating + " of 5 stars" : "Not rated"}`;
    return `<button class="book press${animate ? " rise" : ""}" style="--i:${i}" data-id="${esc(b.id)}" aria-label="${esc(label)}">
      ${bookCover(b)}
      <div class="stars">${stars(b.rating)}</div>
      <div class="book-meta"><div class="book-title">${esc(b.title)}</div><div class="book-author">${esc(b.author)}</div></div>
    </button>`;
  }

  function render(animate = false) {
    const list = sorted();
    grid.dataset.cols = state.prefs.columns;
    grid.innerHTML = list.map((b, i) => cardHtml(b, i, animate)).join("");
    $("#empty").hidden = list.length > 0;
    const rated = list.filter((b) => b.rating > 0).length;
    $("#tally").textContent = `${pad(list.length)} ${list.length === 1 ? "BOOK" : "BOOKS"}${list.length ? ` · ${pad(rated)} RATED` : ""}`;
    $$("#toolbar .seg button").forEach((b) => b.setAttribute("aria-checked", String(+b.dataset.cols === +state.prefs.columns)));
    $("#sort").value = state.prefs.sort;
    $("#sort-label").textContent = $("#sort").selectedOptions[0]?.textContent || "Sort";
  }

  /** Redraws one card in place (a rating or cover change) without re-laying out the shelf. */
  function refreshCard(b) {
    const el = grid.querySelector(`[data-id="${CSS.escape(b.id)}"]`);
    if (!el) return render();
    const i = sorted().indexOf(b);
    el.outerHTML = cardHtml(b, i, false);
    $("#tally").textContent = $("#tally").textContent.replace(/\d+ RATED/, `${pad(state.books.filter((x) => x.rating).length)} RATED`);
  }

  function savePrefs() {
    api.invoke("set_prefs", { prefs: state.prefs }).catch(() => {});
  }

  $$("#toolbar .seg button").forEach((btn) => btn.addEventListener("click", () => {
    if (+btn.dataset.cols === +state.prefs.columns) return;
    state.prefs.columns = +btn.dataset.cols;
    savePrefs();
    render(true);
  }));
  $("#sort").addEventListener("change", (e) => {
    state.prefs.sort = e.target.value;
    savePrefs();
    render(true);
    window.scrollTo({ top: 0 });
  });
  grid.addEventListener("click", (e) => {
    const card = e.target.closest(".book");
    if (card) openDetail(card.dataset.id);
  });

  // ---------- Theme ----------
  function setTheme(t) {
    document.documentElement.dataset.theme = t;
    $$("[data-theme-choice]").forEach((b) => b.setAttribute("aria-checked", String(b.dataset.themeChoice === t)));
    try { localStorage.setItem("shelf-theme", t); } catch {}
  }
  try { setTheme(localStorage.getItem("shelf-theme") === "cream" ? "cream" : "ember"); } catch { setTheme("ember"); }
  $$("[data-theme-choice]").forEach((b) => b.addEventListener("click", () => setTheme(b.dataset.themeChoice)));

  // ---------- Modals ----------
  // Each open modal pushes a history entry so the phone's back gesture closes it.
  const scrim = $("#scrim");
  let openEl = null;
  const onClose = new Map();

  function openModal(el) {
    if (openEl) closeModal(true);
    el.hidden = false;
    el.classList.remove("out");
    scrim.classList.add("on");
    document.body.classList.add("modal-open");
    history.pushState({ modal: el.id }, "");
    openEl = el;
  }
  function closeModal(fromHistory = false) {
    if (!openEl) return;
    const el = openEl;
    openEl = null;
    onClose.get(el)?.();
    el.classList.add("out");
    scrim.classList.remove("on");
    document.body.classList.remove("modal-open");
    setTimeout(() => { if (openEl !== el) { el.hidden = true; el.classList.remove("out"); } }, 220);
    if (!fromHistory && history.state?.modal) history.back();
  }
  window.addEventListener("popstate", () => closeModal(true));
  scrim.addEventListener("click", () => closeModal());
  $$("[data-close]").forEach((b) => b.addEventListener("click", () => closeModal()));
  document.addEventListener("keydown", (e) => { if (e.key === "Escape") closeModal(); });

  // ---------- Picking a photo ----------
  /** Opens the phone's photo picker and returns the image shrunk to a cover-sized JPEG data URL. */
  function pickImage() {
    return new Promise((resolve) => {
      const input = $("#file");
      input.value = "";
      input.onchange = async () => {
        const file = input.files?.[0];
        if (!file) return resolve(null);
        try { resolve(await shrink(file)); } catch {
          toast("That image couldn't be opened. Try a JPG or PNG.");
          resolve(null);
        }
      };
      input.oncancel = () => resolve(null);
      input.click();
    });
  }
  async function shrink(file, max = 1000) {
    const url = URL.createObjectURL(file);
    try {
      const img = new Image();
      img.src = url;
      await img.decode();
      const k = Math.min(1, max / Math.max(img.naturalWidth, img.naturalHeight));
      const c = document.createElement("canvas");
      c.width = Math.round(img.naturalWidth * k);
      c.height = Math.round(img.naturalHeight * k);
      c.getContext("2d").drawImage(img, 0, 0, c.width, c.height);
      return c.toDataURL("image/jpeg", 0.86);
    } finally {
      URL.revokeObjectURL(url);
    }
  }

  // ---------- One book, opened ----------
  const detail = $("#detail");
  const RATING_WORDS = ["Not rated", "1 of 5 · Didn't like it", "2 of 5 · It was OK", "3 of 5 · Liked it", "4 of 5 · Really liked it", "5 of 5 · Loved it"];
  let cur = null;
  let notesTimer = null;
  let deleteTimer = null;

  function fillDetail() {
    const b = byId(cur);
    if (!b) return;
    const list = sorted();
    $("#d-pos").textContent = `Book · ${pad(list.indexOf(b) + 1)} of ${pad(list.length)}`;
    $("#d-cover").innerHTML = bookCover(b) + `<span class="change">${b.cover ? "CHANGE COVER" : "ADD COVER"}</span>`;
    $("#d-title").textContent = b.title;
    $("#d-author").textContent = b.author || "Unknown author";
    const added = new Date(b.addedAt).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
    $("#d-meta").innerHTML = `<div class="meta">${b.isbn ? "ISBN " + esc(b.isbn) : "No ISBN"}</div><div class="meta">Added ${esc(added)}</div>`;
    drawRating(b.rating, 0);
    $("#d-notes").value = b.notes || "";
    setSaved("");
    disarmDelete();
  }

  function drawRating(n, popUpTo) {
    $("#d-stars").innerHTML = [1, 2, 3, 4, 5].map((i) =>
      `<button role="radio" aria-checked="${i === n}" aria-label="${i} star${i > 1 ? "s" : ""}" data-n="${i}">${star(i <= n, i <= popUpTo ? "pop" : "", `--k:${i}`)}</button>`,
    ).join("");
    $("#d-rating-label").textContent = RATING_WORDS[n];
  }
  $("#d-stars").addEventListener("click", (e) => {
    const btn = e.target.closest("button[data-n]");
    const b = byId(cur);
    if (!btn || !b) return;
    const n = +btn.dataset.n;
    // Tapping the current rating again clears it.
    b.rating = b.rating === n ? 0 : n;
    drawRating(b.rating, b.rating);
    refreshCard(b);
    api.invoke("set_rating", { id: b.id, rating: b.rating }).catch((err) => toast(errText(err)));
  });

  function setSaved(s) {
    const el = $("#d-saved");
    el.className = "status" + (s === "saved" ? " saved" : "");
    el.textContent = s === "saved" ? "Saved" : s === "saving" ? "Saving…" : "";
  }
  function flushNotes() {
    if (!notesTimer) return;
    clearTimeout(notesTimer);
    notesTimer = null;
    const b = byId(cur);
    if (!b) return;
    b.notes = $("#d-notes").value;
    api.invoke("set_notes", { id: b.id, notes: b.notes })
      .then(() => setSaved("saved"))
      .catch((err) => { setSaved(""); toast(errText(err)); });
  }
  $("#d-notes").addEventListener("input", () => {
    setSaved("saving");
    clearTimeout(notesTimer);
    notesTimer = setTimeout(flushNotes, 600);
  });
  $("#d-notes").addEventListener("blur", flushNotes);

  $("#d-cover").addEventListener("click", async () => {
    const b = byId(cur);
    const data = b && await pickImage();
    if (!data) return;
    try {
      Object.assign(b, await api.invoke("set_cover", { id: b.id, coverData: data }));
      fillDetail();
      refreshCard(b);
    } catch (err) { toast(errText(err)); }
  });

  $("#d-edit").addEventListener("click", async () => {
    const b = byId(cur);
    if (!b) return;
    if (!detail.classList.contains("editing")) {
      $("#e-title").value = b.title;
      $("#e-author").value = b.author;
      $("#e-isbn").value = b.isbn || "";
      detail.classList.add("editing");
      $("#d-edit").textContent = "Save details";
      $("#e-title").focus();
      return;
    }
    try {
      Object.assign(b, await api.invoke("set_details", {
        id: b.id, title: $("#e-title").value, author: $("#e-author").value, isbn: $("#e-isbn").value || null,
      }));
      stopEditing();
      render();
      fillDetail();
    } catch (err) { toast(errText(err)); }
  });
  function stopEditing() {
    detail.classList.remove("editing");
    $("#d-edit").textContent = "Edit details";
  }

  function disarmDelete() {
    clearTimeout(deleteTimer);
    const btn = $("#d-delete");
    btn.classList.remove("armed");
    btn.lastElementChild.textContent = "Delete";
  }
  $("#d-delete").addEventListener("click", async () => {
    const btn = $("#d-delete");
    const b = byId(cur);
    if (!b) return;
    if (!btn.classList.contains("armed")) {
      btn.classList.add("armed");
      btn.lastElementChild.textContent = "Tap again to delete";
      deleteTimer = setTimeout(disarmDelete, 3500);
      return;
    }
    try {
      await api.invoke("delete_book", { id: b.id });
      state.books = state.books.filter((x) => x !== b);
      notesTimer = null;
      closeModal();
      render();
      toast(`Deleted “${b.title}”`);
    } catch (err) { toast(errText(err)); }
  });

  function openDetail(id) {
    cur = id;
    stopEditing();
    fillDetail();
    detail.querySelector(".detail-scroll").scrollTop = 0;
    openModal(detail);
  }
  onClose.set(detail, () => { flushNotes(); disarmDelete(); stopEditing(); });

  // ---------- Adding a book ----------
  const sheet = $("#add-sheet");
  let found = [];
  let pick = 0;
  let manualCover = null;
  let typed = { title: "", author: "" };

  function showStep(name) {
    $$(".step", sheet).forEach((s) => (s.hidden = s.dataset.step !== name));
    sheet.querySelector(".block").scrollTop = 0;
  }
  function setBusy(btn, busy, label) {
    btn.disabled = busy;
    btn.firstElementChild.textContent = label;
  }

  $("#add").addEventListener("click", () => {
    $("#a-form").reset();
    found = [];
    showStep("form");
    setBusy($("#a-find"), false, "Find book");
    $("#a-progress").hidden = true;
    openModal(sheet);
    setTimeout(() => $("#a-title").focus(), 260);
  });

  $("#a-title").addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !$("#a-author").value) {
      e.preventDefault();
      $("#a-author").focus();
    }
  });

  $("#a-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    typed = { title: $("#a-title").value.trim(), author: $("#a-author").value.trim() };
    if (!typed.title) return $("#a-title").focus();
    document.activeElement?.blur();
    setBusy($("#a-find"), true, "Searching…");
    $("#a-progress").hidden = false;
    let offline = false;
    try {
      found = await api.invoke("lookup", typed);
    } catch {
      found = [];
      offline = true;
    }
    setBusy($("#a-find"), false, "Find book");
    $("#a-progress").hidden = true;
    if (!openEl) return;
    if (found.length) {
      pick = 0;
      drawFound();
      showStep("found");
    } else {
      goManual(offline
        ? ["Couldn't search", "The book catalogs didn't answer. Check your connection and try again, or add it now."]
        : ["Couldn't find it", `Nothing came up for “${typed.title}”. You can still add it.`]);
    }
  });

  function matchMeta(m) {
    return [m.year, m.isbn && "ISBN " + m.isbn, m.source].filter(Boolean).map(esc).join(" · ");
  }
  function drawFound() {
    const m = found[pick];
    $("#f-label").textContent = pick === 0 ? "Best match" : "Your pick";
    $("#f-best").innerHTML = `${coverHtml(m.coverUrl, m.title, m.author)}
      <div class="match-info"><div class="match-title">${esc(m.title)}</div>
      <div class="match-author">${esc(m.author || typed.author || "Unknown author")}</div>
      <div class="meta">${matchMeta(m)}</div>
      ${m.coverUrl ? "" : `<div class="meta">No cover found · one will be drawn</div>`}</div>`;
    const others = found.map((x, i) => [x, i]).filter(([, i]) => i !== pick);
    $("#f-others-wrap").hidden = !others.length;
    $("#f-others").innerHTML = others.map(([x, i]) => `<button class="other press" data-i="${i}">
      ${coverHtml(x.coverUrl, x.title, x.author)}
      <div class="other-text"><div class="other-title">${esc(x.title)}</div><div class="meta">${esc(x.author)}${x.year ? " · " + x.year : ""}</div></div></button>`).join("");
  }
  $("#f-others").addEventListener("click", (e) => {
    const b = e.target.closest("[data-i]");
    if (!b) return;
    pick = +b.dataset.i;
    drawFound();
    sheet.querySelector(".block").scrollTo({ top: 0, behavior: "smooth" });
  });

  let adding = false;
  const sameBook = (a, b) => titleKey(a.title).toLowerCase() === titleKey(b.title).toLowerCase()
    && lastName(a.author).toLowerCase() === lastName(b.author).toLowerCase();

  async function addBook(btn, args) {
    // A second tap while the first is saving, or while the sheet slides away, must not add twice.
    if (adding) return;
    const dupe = state.books.find((b) => sameBook(b, args));
    if (dupe) {
      closeModal();
      toast(`“${dupe.title}” is already on your shelf`);
      setTimeout(() => openDetail(dupe.id), 260);
      return;
    }
    adding = true;
    setBusy(btn, true, "Adding…");
    try {
      const b = await api.invoke("add_book", args);
      state.books.push(b);
      closeModal();
      render();
      const el = grid.querySelector(`[data-id="${CSS.escape(b.id)}"]`);
      if (el) {
        el.classList.add("rise");
        el.scrollIntoView({ block: "center", behavior: "smooth" });
      }
      toast(`Added “${b.title}”`);
    } catch (err) {
      toast(errText(err));
    } finally {
      adding = false;
      setBusy(btn, false, "Add to shelf");
    }
  }

  $("#f-add").addEventListener("click", () => {
    const m = found[pick];
    addBook($("#f-add"), {
      title: m.title, author: m.author || typed.author, isbn: m.isbn ?? null, coverUrl: m.coverUrl ?? null, coverData: null,
    });
  });
  $("#f-none").addEventListener("click", () => goManual(["Add it yourself", "Add a cover photo and the ISBN if you have them."]));

  function goManual([heading, why]) {
    manualCover = null;
    $("#m-heading").textContent = heading;
    $("#m-why").textContent = why;
    $("#m-book").textContent = typed.author ? `${typed.title} · ${typed.author}` : typed.title;
    $("#m-isbn").value = "";
    const p = $("#m-pick");
    p.classList.remove("has");
    p.querySelector("img")?.remove();
    showStep("manual");
  }
  $("#m-pick").addEventListener("click", async () => {
    const data = await pickImage();
    if (!data) return;
    manualCover = data;
    const p = $("#m-pick");
    p.querySelector("img")?.remove();
    p.insertAdjacentHTML("beforeend", `<img src="${data}" alt="Chosen cover">`);
    p.classList.add("has");
  });
  $("#m-add").addEventListener("click", () => {
    const isbn = $("#m-isbn").value.replace(/[^0-9xX]/g, "");
    if (isbn && isbn.length !== 10 && isbn.length !== 13) return toast("An ISBN is 10 or 13 digits.");
    addBook($("#m-add"), { title: typed.title, author: typed.author, isbn: isbn || null, coverUrl: null, coverData: manualCover });
  });
  $("#m-back").addEventListener("click", () => {
    showStep("form");
    $("#a-title").focus();
  });

  // ---------- Settings and backups ----------
  const settingsSheet = $("#settings-sheet");
  T?.app?.getVersion().then((v) => ($("#s-version").textContent = v)).catch(() => {});
  $("#settings").addEventListener("click", () => {
    settingsSheet.querySelector(".block").scrollTop = 0;
    openModal(settingsSheet);
  });

  const today = () => new Date().toISOString().slice(0, 10);
  const plural = (n, w) => `${n} ${w}${n === 1 ? "" : "s"}`;

  $("#s-backup").addEventListener("click", async () => {
    if (!state.books.length) return toast("There's nothing to back up yet.");
    try {
      if (!T) return toast(`Backed up ${plural(await window.ShelfMock.downloadBackup(), "book")}`);
      const path = await T.dialog.save({
        defaultPath: `shelf-backup-${today()}.json`,
        filters: [{ name: "Shelf backup", extensions: ["json"] }],
      });
      if (!path) return;
      const n = await api.invoke("export_backup", { path });
      toast(`Backed up ${plural(n, "book")}`);
    } catch (err) { toast(errText(err)); }
  });

  // The picker is the WebView's own file chooser in the browser preview; in the app it's Android's,
  // whose content:// answer only the Rust side can read.
  function pickBackupText() {
    return new Promise((resolve) => {
      const input = $("#backup-file");
      input.value = "";
      input.onchange = () => input.files?.[0] ? input.files[0].text().then(resolve, () => resolve(null)) : resolve(null);
      input.oncancel = () => resolve(null);
      input.click();
    });
  }
  $("#s-restore").addEventListener("click", async () => {
    try {
      let res;
      if (!T) {
        const text = await pickBackupText();
        if (!text) return;
        res = await api.invoke("import_backup_text", { json: text });
      } else {
        const path = await T.dialog.open({ multiple: false, directory: false });
        if (!path) return;
        res = await api.invoke("import_backup", { path });
      }
      const snap = await api.invoke("load");
      state.books = snap.books;
      render(true);
      closeModal();
      toast(res.added
        ? `Restored ${plural(res.added, "book")}${res.skipped ? ` · ${res.skipped} already here` : ""}`
        : "Every book in that backup is already here.");
    } catch (err) { toast(errText(err)); }
  });

  // ---------- Start ----------
  api.invoke("load").then((snap) => {
    state.books = snap.books;
    state.prefs = snap.prefs;
    state.coversDir = snap.coversDir;
    render(true);
  }).catch((err) => toast(errText(err)));
})();
