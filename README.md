# Shelf

A small Android book log in the look of [Rise](https://github.com/HankTheMan2828). Books sit on a
shelf as covers (1, 3 or 6 per row) with star ratings; tap one to rate it and keep notes. **Add**
looks the book up on Google Books and Open Library for its cover and ISBN, and falls back to letting
you pick a photo and type the ISBN yourself. Settings has the light/dark look and one-file
backup/restore.

- `src-tauri/` – the Rust core (Tauri 2): the library on disk (`store.rs`), the catalog lookup
  (`lookup.rs`), backups (`backup.rs`) and the commands the UI calls (`lib.rs`).
- `src/` – the UI: `index.html`, `styles.css` (Rise's Ember/Cream palettes, Geist and Space Mono),
  `app.js`. `mock.js` stands in for the Rust core when `src/` is opened in a plain browser.
- `docs/` – the website and privacy policy, served by GitHub Pages.
- `store/` – Play Store listing text, graphics, screenshots and the [launch guide](store/PLAY-LAUNCH.md).

Data lives in the app's private storage: `library.json` plus a `covers/` folder.

## Book search

Google Books and Open Library are asked at once; results are merged, ranked by how well they match
what was typed (typos included), and spin-offs like summaries and workbooks are pushed down.
Google needs an API key for reliable results: put it in `google-books.key` (never committed) and
`build-apk.ps1` bakes it into the build. Without it Google runs on a shared keyless quota that is
usually used up, and search falls back to Open Library alone.

## Building

Needs JDK 17, the Android SDK with NDK 28, and the Rust Android targets (all set up on this PC;
`JAVA_HOME`, `ANDROID_HOME` and `NDK_HOME` are set for your user).

```
npm install
.\build-apk.ps1            # debug APK for sideloading: dist\Shelf.apk
.\build-apk.ps1 -Emulator  # plus x86_64, for the Android emulator
.\build-apk.ps1 -Release   # signed App Bundle for Google Play: dist\Shelf.aab
```

Two things on this PC shape the script. `tauri android build` compiles the Rust library, then
tries to symlink it into the Android project, which Windows refuses outside Developer Mode; the
script copies it instead and runs Gradle itself. And Smart App Control blocks some helper programs
that a release-profile Rust build compiles, so the Rust library is always built with the dev
profile (trimmed for size in `Cargo.toml`). Android-side, `-Release` is a real release build:
minified, not debuggable, devtools off, signed with the upload key.

## Releasing

The Play upload key and its passwords live in `~\.shelf-signing\` (outside the repo; back that
folder up). Gradle reads `keystore.properties` from there; point `SHELF_SIGNING_PROPERTIES` at
another file to use a different one. Bump `version` in `src-tauri/tauri.conf.json` for every
upload: the Android version code is derived from it. Then follow [store/PLAY-LAUNCH.md](store/PLAY-LAUNCH.md).

## App icon

`src-tauri/app-icon.svg` is the source. After changing it run `npx tauri icon src-tauri/app-icon.svg`,
which also writes the Android launcher icons into `src-tauri/gen/android/.../res`. The adaptive
icon's background color lives in `res/values/ic_launcher_background.xml`.

## Installing a sideloaded build

Send the `.apk` to the phone (message, Drive, USB), open it, and allow "install unknown apps" for
whatever app opened it when Android asks. Sideloaded builds are debug-signed, so they can't update
to or from the Play version; use **Settings → Back up / Restore** to move a library across.

## Preview the UI on a computer

Run the `shelf-ui` preview (`.claude/launch.json`), or any static server on `src/`. It uses
`mock.js` and the browser's storage instead of the Rust core.

## Tests

```
npm test                                                        # unit tests
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored    # also hits the live catalogs
```

Fonts: Geist and Space Mono, both under the SIL Open Font License (`licenses/`).
