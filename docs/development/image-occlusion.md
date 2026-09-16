# Manual image occlusion: Stage 6 test drive

Scope: PR #101, `agent/interactive-question-types`. This guide covers manual image
question authoring and Practice, not automatic image extraction, automatic label
grading, typed deck exchange, or final release qualification. The accepted
[interactive-question contract](interactive-question-types.md) remains the roadmap.
The exact reviewed SHA and qualification results belong in the PR handoff.

## Implementation and ownership

The ordinary Create/Edit Card screen owns the Template selector, deck/tags,
per-type durable drafts, asset draft, Save, and Save + another. Selecting **Image
occlusion** mounts a purpose-built editor; changing an unsaved type preserves its
own draft rather than translating its content into a Basic card.

`src/editor/occlusion/` owns normalized geometry, pointer gestures, region history,
the SVG authoring workspace, and the shared read-only image viewport. The editor
supports mouse, touch, pen, keyboard adjustment, and non-drag buttons. The SVG is
an authoring surface with access to the managed source, not a secure redaction
system or the pre-answer Practice image.

`src/screens/card-editor/` owns image import/recovery, question-region assignments,
rich Question/Answer/Explanation fields and author preview. Image imports use the
existing managed file/draft boundary. Preview and Practice raster reads share a
serialized queue: obsolete pending reads are skipped and late results cannot
replace the current target. URLs are released on replacement or unmount.

Native code in `src-tauri/src/cards/questions/occlusion.rs` owns bounded decoding,
upright image normalization, metadata-free PNG output, and mask painting before
resampling. Accepted input is a still PNG, JPEG or WebP, at most 25 MiB, at most
8192 pixels on either edge and at most 16 million decoded pixels. Animated inputs
and unsupported formats fail admission rather than becoming partial questions.
The editor permits at most 128 rectangles and 64 questions per image note.

`src-tauri/src/practice/question_attempts/` owns captured source/target identity,
asset retention, authorized masked pixels, reveal receipts and the review link.
Before reveal, the WebView receives masked PNG bytes, not the original image
covered by CSS. A client cannot select the reveal phase; native saved attempt
state chooses it. The bounded process-local cache contains rebuildable derivatives
and does not bypass source availability, ownership or revision checks.

One question may target several regions. Several questions may share a region.
Each question owns an independent scheduled presentation; moving or resizing a
region does not create a new question identity or transfer its review history.
Hide-all reveals only the current question's targets, keeping unrelated regions
covered. Overlapping unrelated masks remain covered and trigger an editor warning.

Image Practice is **recall -> Reveal answer -> self-rate**. It does not compare a
typed label or manufacture an objective verdict. Reveal and review retries retain
their original mutation identities. A committed reveal remains committed even if
image loading or interval refresh subsequently fails. The existing transactional
FSRS path remains the only scheduler.

## Obtain and build the intended source

From an existing configured checkout, first inspect local edits. Do not discard
unrelated work or use a destructive reset to switch branches.

```bash
git status --short
git fetch origin --prune
git switch agent/interactive-question-types
git pull --ff-only
git rev-parse HEAD
```

Compare the printed SHA with the full SHA in the handoff. Stop on a mismatch.
For a separate detached review checkout, follow
[Local testing and handoff](local-testing-and-handoff.md#obtain-the-exact-source-safely).
A new checkout does not create a separate application library.

On the configured Linux workstation:

```bash
./scripts/build.sh
./run-app.sh
```

The wrapper creates a standalone debug application and the launcher verifies its
source/build stamp. Do not bypass a stale-build error with Cargo's mutable debug
binary. Machine provisioning is not required for each PR.

Back up the ordinary library before testing migrations. For disposable content,
launch with isolated file-backed state instead:

```bash
TEST_ROOT="$(mktemp -d -t ostudy-occlusion.XXXXXX)"
mkdir -p "$TEST_ROOT"/{data,config,cache,state}
printf 'Keep this directory for restart tests: %s\n' "$TEST_ROOT"
XDG_DATA_HOME="$TEST_ROOT/data" \
XDG_CONFIG_HOME="$TEST_ROOT/config" \
XDG_CACHE_HOME="$TEST_ROOT/cache" \
XDG_STATE_HOME="$TEST_ROOT/state" \
  ./run-app.sh
```

Reuse that same directory and all four overrides for restart tests. This does not
isolate the operating-system credential store. No real API key or provider is
needed for any manual image question or Practice test.

## Desktop acceptance

Create a disposable deck and use a synthetic labeled diagram or another image you
may freely share. Avoid private source documents in diagnostic reports.

| Test | Actions and expected result |
| --- | --- |
| Entry and image import | Create a card, choose Image occlusion in Card template, then Choose image. Try still PNG/JPEG/WebP. Canceling the picker preserves the draft; invalid or oversized input produces an error without a partial saved note. |
| Region creation | Use Draw rectangle, Tap two corners, and Add rectangle. Try all draw directions and a letterboxed image. Shapes stay aligned with image coordinates, including after resizing the window. |
| Adjustment and history | Select a region; drag/move/resize it, then use Move left/right/up/down and Wider/Narrower/Taller/Shorter. Undo region edit and Redo region edit restore geometry without replacing later question text. Escape cancels an unfinished gesture. |
| Many-to-many targets | Make one question target two checked regions. Use Add question to ask a different question about an existing region. Complete each Question and Answer. The displayed study-card count follows questions, not the number of rectangles. |
| Mask modes | Preview current question before and after reveal. Compare Hide all regions; reveal this question's targets with Hide only this question's targets. Unrelated masks stay covered in hide-all mode. Test an overlap and its warning. |
| Editing safety | Remove a referenced region and inspect the confirmation. A question left with no target remains in the draft but cannot be saved until retargeted. Undo restores its associations. Replace image requires confirmation and starts new targets only after successful import. |
| Save/reopen | Save question, reopen, and inspect text, masks and assignments. Move a region and save again: existing question schedules must remain associated with the same questions. Save + another starts an empty image draft. |
| Recovery | Switch to Basic and back without saving; the image draft remains. Close/reopen after the draft is saved and confirm recovery. Cancel a recovered import or resume it using its explicit controls. |
| Practice | Start Practice from the deck. The source labels are covered immediately, including during image loading and zoom. Reveal answer shows the target and authored answer, then permits Wrong/Hard/Medium/Easy. One rating advances once. |
| Resume and navigation | Restart after reveal but before rating; resume feedback rather than a new unanswered card. Use zoom/fullscreen and close it; viewport interaction must not flip or rate the card. Editing during Practice must not rewrite the captured attempt's answer key. |
| Existing types | Study a Basic, Cloze and objective question in the same deck. Their reveal/check/rating controls and normal session return still work. |

Repeat the main editor and Practice paths offline, in the console/light/dark themes,
and at compact window width. Physical visual acceptance is not established by a
unit-test pass.

## Android physical-device test

Use the same source checkout and the existing configured Android toolchain:

```bash
adb devices -l
pnpm android:doctor
pnpm android:install
```

The normal wrapper builds, installs with replacement and launches the ARM64 debug
app `io.github.jferments.ostudy.dev`. With several connected devices:

```bash
bash ./scripts/android-install.sh --serial 'REPLACE_WITH_ADB_SERIAL'
```

Do not uninstall or clear app storage as routine setup. Replacement installation
preserves the debug library; debug and release have separate application IDs.
Create a small image note directly on the phone for this checkpoint rather than
using the still-incompatible interactive transfer format.

Test Tap two corners, touch drawing, corner resizing, two-finger pan/pinch, Fit
image, text entry, portrait/landscape changes and the software keyboard. A second
finger should switch to viewport manipulation without committing a stray region.
Check Android Back closes an overlay before leaving the editor/session, unsaved
changes remain protected, and the Practice image stays masked through fullscreen
and orientation changes. Repeat one complete create/save/study cycle offline.

## Automated checks and remaining boundaries

The normal iteration gate is:

```bash
./scripts/check-fast.sh
```

Focused frontend checks after dependencies have been installed:

```bash
pnpm exec vitest run \
  src/editor/occlusion \
  src/screens/card-editor/occlusionDraft.test.ts \
  src/screens/card-editor/useOcclusionImage.test.tsx \
  src/screens/card-editor/useOcclusionPreview.test.tsx \
  src/screens/CardEditorOcclusion.test.tsx \
  src/screens/practice/ImageOcclusionActivity.test.tsx \
  src/screens/practice/useOcclusionReveal.test.tsx \
  src/screens/practice/usePracticeOcclusionImage.test.tsx
```

Fast qualification includes native image decode/masking, authoring, captured
attempt/review, migration and transfer-guard tests. Full Linux qualification is
`./scripts/check.sh`; Android compile/package and emulator gates are
`pnpm android:test` and `pnpm android:smoke`. These are separate from physical-device
acceptance and are not required merely to inspect this Stage 6 test drive. Terminal
release qualification stays at the frozen final PR candidate after Stages 7–9.

Typed .studydeck/.ostudy.json exchange and complete device synchronization remain
unfinished. The current sync path rejects typed question graphs before sending or
merging instead of dropping their presentation/attempt ownership. Documents-only
transfers omit unselected questions and private editor draft payloads. Do not use
typed export/sync as a backup workaround. Whole-library backup is a different
contract; retain a pre-upgrade backup before upgrading a valuable library to
schema 15. An older executable cannot be assumed to open that upgraded library.

Report the exact Git SHA, Linux/Android build mode, device/WebView environment,
smallest reproduction and expected/actual behavior. Use `pnpm android:diagnostics`
for bounded debug diagnostics. Do not attach a live library or API credentials.
