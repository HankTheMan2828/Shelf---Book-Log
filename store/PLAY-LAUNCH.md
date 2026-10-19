# Launching Shelf on Google Play

Your developer account is newer than November 2023, so Google requires a **closed test with at
least 12 testers who stay opted in for 14 days** before you can publish to everyone. The order:

1. Create the app and fill in its "App content" answers (an hour, once)
2. Upload the build to a **closed test**, and invite 12+ testers
3. Wait 14 days with them opted in
4. **Apply for production**, then publish

## 1. Create the app

Play Console → **Create app**
- App name: `Shelf: Book Log` · Default language: English (United States)
- App · Free · tick both declarations

Then work through **Dashboard → Set up your app**. Answers for each:

| Section | Answer |
| --- | --- |
| Privacy policy | `https://hanktheman2828.github.io/Shelf---Book-Log/privacy.html` (goes live once the repo is pushed and Pages is on, see below) |
| App access | All functionality is available without any special access |
| Ads | No, my app does not contain ads |
| Content rating | Category **Reference, News, or Educational**; answer **No** to every content question → rated Everyone / 3+ |
| Target audience | **13–15, 16–17, 18+** (leaving out under-13 keeps you out of the stricter Families program) |
| News app | No |
| Data safety | See below |
| Government app | No |
| Financial features | My app doesn't provide any financial features |
| Health | No health features |

### Data safety answers

Shelf keeps everything on the phone, with one exception: the title and author typed into **Add**
are sent to Open Library as a search. Play counts sending data off the phone as
"collecting" it, even when you never see or store it. The honest answer:

- **Does your app collect or share any of the required user data types?** Yes
- **Is all of the user data collected by your app encrypted in transit?** Yes
- **Do you provide a way for users to request that their data is deleted?** Yes (books are deleted in the app; searches are never stored)
- Data type: **App activity → In-app search history**
  - Collected: **Yes** · Shared: **No** (it goes to the service the app uses to do the search, not passed on for anyone else's use)
  - **Processed ephemerally:** Yes · **Required** (not optional)
  - Purpose: **App functionality**

Nothing else: no location, contacts, identifiers, photos (a picked cover photo never leaves the
phone), or analytics. If you'd rather be maximally cautious, you can also tick **Shared** for
search history; it only makes the label stricter, never wrong.

## 2. Store listing

**Grow users → Store presence → Main store listing**: paste the texts from
[listing.md](listing.md) and upload the icon, feature graphic and five screenshots from `store/`.

## 3. Closed test

1. **Test and release → Testing → Closed testing → Create track** (call it `Friends`)
2. **Testers** tab: create an email list with the Google account emails of 12 or more people.
   Your friend and the people he's giving it to are perfect. Save, then copy the **opt-in link**.
3. **Create release**
   - Play App Signing: keep **"Let Google manage and protect your app signing key"** (the default).
     Your own key is only the *upload* key.
   - Upload `dist/Shelf.aab`
   - Release name `0.1.0`, release notes: `First version of Shelf.`
4. **Review release → Start rollout to Closed testing.** The first review takes a few days.
5. Send testers the opt-in link. Each one opens it, taps **Become a tester**, then installs Shelf
   from the Play Store link on that page.

**Testers already using the sideloaded app:** that copy can't update to the Play version (it's
signed with a different key), so they must uninstall it first, which erases its library. Before
uninstalling: install the latest sideloaded `Shelf.apk` (it updates in place and keeps their
books), open **Settings → Back up**, save the file, then uninstall, install from Play, and use
**Settings → Restore**.

## 4. The 14 days

Testers need to stay opted in (installing and opening it now and then helps). Google asks later
how testing went, so note any feedback and the fixes you shipped. To ship a fix: bump `version`
in `src-tauri/tauri.conf.json` (e.g. `0.1.1`), run `.\build-apk.ps1 -Release`, and upload the new
AAB as a new closed-testing release.

## 5. Production

After 14 days with 12+ opted-in testers, **Dashboard → Apply for production** unlocks. Answer the
questions about the test, wait for review (usually a few days, up to 7), then create a
**Production** release with the same AAB and roll it out.

## Turning on GitHub Pages (for the privacy policy URL)

After the code is on GitHub: repo **Settings → Pages → Build and deployment** → Source **Deploy
from a branch**, branch `main`, folder `/docs` → Save. In a minute the policy is live at
`https://hanktheman2828.github.io/Shelf---Book-Log/privacy.html`.

## Keys to keep safe

- `C:\Users\DyckH\.shelf-signing\` holds the **upload key** and its password. Copy that folder
  somewhere safe (Proton Drive, a USB stick). If it's lost, Google can reset the upload key, but
  it takes a support request and a few days.
