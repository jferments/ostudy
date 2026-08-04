# Stage 3.4 and 3.5 local acceptance

## Safety first

Schema 13 is forward-only. Before using a regular library, copy the complete application-data directory shown under **Settings → Storage**. Use an isolated profile for initial testing:

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

## Automated gates

```bash
bash ./scripts/check-fast.sh
./scripts/check.sh
```

The complete gate must pass frontend tests, TypeScript, production build, rustfmt, warning-free Clippy, Rust and migration tests, standalone Tauri build, dependency verification, and the native create-review-restart persistence workflow.

## Current `.studydeck` package

Create a deck containing:

- an ordinary Basic note;
- a reverse-enabled Basic note;
- a one-at-a-time cloze note;
- several overlapping tags; and
- at least one managed image.

Export the deck, then verify:

1. the destination is one `.studydeck` SQLite file;
2. an existing destination is replaced only after the new file validates;
3. the original library can still edit and practice the deck;
4. importing the file creates one deck with the same name and description;
5. Basic, reverse, and cloze presentation counts match;
6. tags and images render correctly;
7. imported schedules are fresh New schedules rather than copies of the source learner's history;
8. importing the same file again as a new user action creates another independent deck; and
9. retrying the same native operation ID does not create a second deck.

Also import a known package-version-1 fixture containing tags and historical multi-row cloze cards. It should import successfully as one active cloze schedule.

## Failure and atomicity checks

Using disposable packages or the automated failure-injection tests, verify:

- damaged SQLite packages are rejected before mutation;
- missing metadata and unsupported package versions are rejected;
- invalid rich documents, tag links, generated-card links, or image signatures leave no deck;
- an image staging or rename failure leaves no partial deck;
- a database failure after file staging cleans only that import's unreferenced draft;
- reusing one operation ID with another file returns `idempotency_conflict`; and
- an ambiguous retry with the same file returns the originally committed result.

## Complete library backup

Populate the isolated library with multiple decks, reusable templates, images, reviewed cards, practice sessions, manual schedule actions, and non-default settings.

Open **Settings → Storage** and choose **Back up library**. Verify that the success message reports deck, template, and review-event counts.

The backup must be refused while an unsaved card or template editor owns staged images.

## Complete restore

1. After creating a backup, make obvious changes to the current library: delete a deck, alter settings, and add reviews.
2. Choose **Restore backup** and select the saved `.ostudybackup` file.
3. Confirm that ostudy says the restore is prepared rather than claiming it already replaced the open database.
4. Close every ostudy window and launch the application again.
5. Confirm the backed-up decks, templates, images, schedules, review histories, sessions, analytics, and settings have returned.
6. Confirm changes made after the backup are absent.
7. Open Analytics for several time ranges and compare review counts and session history with the pre-backup values.
8. Confirm **Settings → Storage** reports schema 13.

Test an invalid backup as well. Preparation must fail without creating a pending restore. A validated restore whose database cannot open must roll back to the prior database and asset directory rather than leaving a mixed library.

## One-time repair behavior

Upgrade copies of historical schema fixtures and a real pre-Stage-3.5 library. Verify:

- missing duplicate fingerprints are populated;
- legacy template patterns become rich documents;
- explicit template placeholders are canonical;
- multi-row cloze notes have one active forward card;
- prior sibling card rows remain soft-deleted so schedules and immutable history are retained;
- abandoned image-occlusion values become ordinary Basic content;
- the active template table no longer exposes `front_pattern` or `back_pattern`;
- new image-occlusion values and nullable template documents are rejected; and
- all repair ledger rows are present with stable checksums.

Close and reopen the canonical database without editing anything. The second startup must not change application rows, timestamps, revisions, the repair ledger, or the WAL merely because ostudy launched.

## Merge conditions

- The self-hosted `Check / check` status is green on the final branch head.
- `./scripts/check.sh` passes locally.
- Current and legacy deck round trips pass.
- Complete backup and restart restore pass in an isolated profile.
- A copied regular library upgrades without data loss.
- No superseded transfer wrapper or temporary code-generation workflow remains in the final diff.
