# UI screenshots for before/after review

`./scripts/ui-screenshots.sh` photographs the main screens of a built desktop app against a seeded, disposable library and turns two captures into a side-by-side comparison page. Use it as review evidence for a UI change. It is not a correctness gate and not a committed baseline; the historical [Stage-0 visual baseline](../history/ui-workflow-redesign-visual-baseline.md) is separate.

## Requirements

Linux with the native test stack from `./scripts/setup-ubuntu.sh` (WebKitWebDriver, `tauri-driver`, Xvfb), PDFium provisioned in the checkout, and ImageMagick with WebP support (`sudo apt-get install imagemagick`). libwebp's `cwebp` is used for captures when present. One capture runs at a time per machine.

## Before and after for a branch

```bash
./scripts/build.sh
base="$(git merge-base origin/main HEAD)"
./scripts/ui-screenshots.sh capture --ref "$base" --output .artifacts/ui-screenshots/before
./scripts/ui-screenshots.sh capture --output .artifacts/ui-screenshots/after
./scripts/ui-screenshots.sh compare .artifacts/ui-screenshots/before .artifacts/ui-screenshots/after \
  --output .artifacts/ui-screenshots/compare
```

Without `--ref`, `capture` uses this checkout's current `./scripts/build.sh` output and records `HEAD` (and whether the tree had local changes). `--ref REV` builds that commit once in a private worktree under `~/.cache/ostudy/ui-screenshots/` with the embedded production frontend, keeps a stripped copy of the binary by commit SHA (the three most recently used are kept), and removes that worktree's executables from the shared Cargo target. A cold build takes four to six minutes with a warm Cargo target; later captures of the same commit start immediately. `--application PATH` captures any other binary. `OSTUDY_UI_SCREENSHOTS_CACHE`, `_KEEP`, `_HOME`, `_PORT` and `_TIMEOUT` override the cache directory, the number of kept binaries, the disposable data directory, the WebDriver port (4460) and the capture time limit.

Open `compare/index.html` in a browser, or publish the folder as it is: the page has inline CSS and script and refers to its images by relative path.

## What is captured

`./scripts/ui-screenshots.sh list` prints the screen ids. A full capture is 296 images in a few minutes: twenty-one screens at phone 390×844, tablet 1024×768 and desktop 1440×900 (Home, the first-run welcome of an empty library, a subject's Library Overview, Library's Decks, Documents and Study guides views, a deck's card list, the card editor, the study guide reader, a document's generation-info dialog, the Progress chart, summary tiles and topic table, Tutor, and each Settings tab), the practice setup dialog at phone and desktop, and the Practice question, revealed answer and session summary at all three widths, each in the light, dark, console and synthwave themes. The first-run screen is photographed in its own session on a library the app created and nothing else has touched. `--screens`, `--themes` and `--widths` take comma-separated values (`settings` and `practice` select groups; widths also accept `phone`, `tablet` and `desktop`) on both `capture` and `compare`. Screens that start below the fold are scrolled into view: on a phone the card editor shows its question and answer, and the two lower Progress captures start at the summary tiles and the topic table.

The engine recognizes which navigation a build has, so the same tool captures builds from before the navigation redesign: there it reaches each screen through the sidebar of section tabs, captures the empty deck library as the first-run screen, and skips Home and Review, which the manifest notes and the comparison shows as added screens.

The output directory gets one `<screen>--<theme>--<width>.webp` per capture (lossy WebP, quality 85) and `manifest.json`: the source commit, the binary's hash, locale and time zone, timings, and each capture's pixel hash. A screen that cannot be reached is recorded as failed together with what was on screen at that moment, and the command exits non-zero.

The comparison page has a theme and width picker, shows each changed screen before and after side by side (stacked on narrow windows), and folds screens whose pixels are identical into a "No change" list. A full matrix stays under 300 files and 40 MiB: unchanged images are copied once and each changed pair becomes one joined image.

## Seeded library and determinism

Every capture starts the app with fresh data under `/tmp/ostudy-ui-screenshots` and never touches a real library or the desktop keyring. It seeds a medical course through the app's own commands: four subjects (Physiology with three decks and a document, Biochemistry with decks only, Pharmacology with a deck and a document, and Microbiology, whose cards are all reviewed so nothing is due today), one more deck and a lecture under No subject, 47 notes covering Basic, Cloze, true/false, single-choice, select-all, matching, ordering and grouping cards with tags, 30 practice sessions spread over six weeks, and a card template. Study guides, document records and the Tutor's replies normally need an AI provider, so they are written directly into the disposable database after the app closes, with each document filed in its subject; there is no seeding code in the application.

The seed content is fixed, tags get fixed ids in name order (a deck's card list orders each card's tags by id), the app runs in UTC with US English formatting, motion is set to none, and an injected stylesheet turns off animations, transitions, the caret and scrollbars and makes WebKit paint scaled images at full quality straight away. A screen is captured only after fonts have loaded, visible images have decoded, busy indicators and notifications are gone, the pointer is parked on an inert spot, and two consecutive frames match; the manifest notes any screen that never settled. The app has no fixed clock, so review history and other dates are placed relative to the capture day: compare captures made on the same UTC day, or expect date text to differ (the page warns when the days differ).

Some screens show what an isolated machine shows: AI settings have no provider, device sync reports that the secure store is unavailable, Tutor says no model is selected, and seeded documents show no rendered pages or generated cards.
