# Managed assets and deletion Undo testing

Stage 3.3 makes the SQLite asset ledger authoritative while keeping filesystem changes recoverable across cancellation, partial writes, and process interruption.

## Safety rules

- Test schema 11 against an isolated XDG profile before opening a regular library.
- Do not open a schema-11 database with a schema-10 build.
- Keep a copy of the regular application-data directory until the pull request is merged.
- Never manually remove an image that is still referenced by a card or template.

## Automated gates

Run the fast high-return gate while iterating:

```bash
bash ./scripts/check-fast.sh
```

Before merging, run the complete gate:

```bash
./scripts/check.sh
```

The complete gate includes frontend tests, TypeScript, rustfmt, warning-free Clippy, Rust/SQLite/migration tests, a standalone Tauri build, dependency verification, and the native create-review-restart persistence workflow.

## Isolated manual profile

```bash
TEST_ROOT="$(mktemp -d)"
mkdir -p \
  "$TEST_ROOT/data" \
  "$TEST_ROOT/config" \
  "$TEST_ROOT/cache" \
  "$TEST_ROOT/state"

XDG_DATA_HOME="$TEST_ROOT/data" \
XDG_CONFIG_HOME="$TEST_ROOT/config" \
XDG_CACHE_HOME="$TEST_ROOT/cache" \
XDG_STATE_HOME="$TEST_ROOT/state" \
./run-app.sh
```

The profile should report schema 11 under **Settings → Storage**.

## Card editor lifecycle

1. Start a new card and add an image.
2. Save the card, restart ostudy, and confirm the image still renders.
3. Start another card, add an image, then cancel the editor.
4. Restart ostudy and confirm the canceled card does not exist.
5. Add two images, remove one from the document before saving, and save.
6. Confirm only the retained image remains associated with the saved card.
7. Edit a saved card that already has an image, make a text-only change, and save.
8. Confirm the previously ready image was not copied or deleted.
9. Use **Save + another**, add a different image to the second card, and confirm the two editor sessions use independent drafts.

## Template lifecycle

1. Create a reusable template and add an image to one layout.
2. Save and reopen the template; the image must render.
3. Begin another template, add an image, and cancel.
4. Restart and confirm no canceled template was created.
5. Generate cards from a template value containing an image and verify the generated card after restart.

## Exact deletion Undo

### One card

1. Delete a card.
2. Confirm the success toast contains **Undo**.
3. Activate Undo before the toast expires.
4. Confirm the card returns to its deck with its tags, images, schedule, and review history intact.

### Multiple cards

1. In **Search/Filter**, select several source notes and delete them.
2. Activate Undo.
3. Confirm every exact selected note and generated direction returns.
4. Confirm unrelated cards remain unchanged.

### Deck

1. Delete a temporary deck containing cards, tags, images, and review history.
2. Activate Undo.
3. Confirm the deck and only the rows deleted by that operation return.
4. Confirm cards that were already deleted before the deck deletion are not revived.

### Expiry and conflicts

1. Delete a temporary item and allow the Undo toast to expire.
2. Confirm the item remains deleted.
3. Restart ostudy so expired relationship cleanup and asset reconciliation run.
4. Confirm the application opens normally and no active card lost an image.

## Crash and filesystem reconciliation

Use only a disposable isolated profile for these tests.

1. Save an image card and close ostudy normally.
2. Copy the profile as a backup.
3. Place an unrelated image file directly in the managed image directory.
4. Start ostudy. The unknown file should be moved into managed quarantine rather than adopted or deleted silently.
5. Restore the profile backup.
6. Rename one referenced managed image outside ostudy.
7. Start ostudy. The asset should be marked missing, the application should remain usable, and no unrelated file should be removed.
8. Restore the image and restart. Reconciliation should return the asset to ready state.
9. Interrupt a `.studydeck` import while files are being staged, then restart. Temporary or unowned final files should be quarantined, while a package whose SQLite transaction committed must retain its referenced assets.

## Shared references

1. Create two durable owners that reference the same managed asset through a supported copy/template workflow.
2. Delete one owner and let its Undo window expire.
3. Restart ostudy.
4. Confirm the remaining owner still renders the shared image.

The reconciler must never quarantine or retire a file while any durable note or template relationship still references its asset row.
