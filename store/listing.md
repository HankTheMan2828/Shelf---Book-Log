# Play Store listing

Paste these into **Grow users → Store presence → Main store listing**.

## App name (30 max)

```
Shelf: Book Log
```

## Short description (80 max)

```
Log your books with covers found for you, star ratings and notes. No ads.
```

## Full description (4000 max)

```
Shelf is a simple, good-looking place to log the books you read.

Type a title and an author. Shelf looks the book up and fills in its cover and ISBN for you, even when you spell it wrong. Can't find it? Add your own cover photo, or let Shelf draw one.

YOUR SHELF
• Scroll your books as covers, 1, 3 or 6 wide
• Sort by most recent, oldest, title, author, or rating
• Light and dark looks

EVERY BOOK
• Tap a cover to open it
• Rate it from one to five stars
• Keep notes: thoughts, quotes, where you left off
• Fix the details or change the cover any time

YOURS, ON YOUR PHONE
• No account, no sign-in, no ads, no tracking
• Your library stays on your phone
• Back up your whole library to one file and restore it on a new phone

Shelf is free and open source. Book details and covers come from Google Books and Open Library.
```

## Category and contact

- **App category:** Books & Reference
- **Tags:** Books, Reading tracker (pick the closest Play offers)
- **Website:** https://hanktheman2828.github.io/Shelf---Book-Log/
- **Email:** required and shown publicly on the listing; use the address you want readers to write to
- **Privacy policy:** https://hanktheman2828.github.io/Shelf---Book-Log/privacy.html

## Graphics

| Asset | File | Play's size |
| --- | --- | --- |
| App icon | `store/icon-512.png` | 512 × 512 PNG |
| Feature graphic | `store/feature-graphic.png` | 1024 × 500 |
| Phone screenshots | `store/screenshots/1-shelf.png` … `5-cream-one-wide.png` | 1080 × 1920 (9:16) |

The screenshots use public-domain classics with Open Library's scans of old editions, so no
publisher's cover art appears in the listing. To redraw the feature graphic after changing
`feature-graphic.html`:

```
& "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe" --headless=new --hide-scrollbars --force-device-scale-factor=1 --screenshot="$PWD\store\feature-graphic.png" --window-size=1024,500 "file:///$PWD/store/feature-graphic.html"
```
