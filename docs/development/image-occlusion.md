# Manual image occlusion: Stage 6 test drive

Scope: PR #101, `agent/interactive-question-types`. This guide covers the manual
interactive-question/editor/Practice checkpoint before automatic generation and
final integration in Stages 7–9. It does not claim automatic occlusion extraction,
typed deck exchange, release qualification, or physical-device acceptance.

## Current image-occlusion behavior

The ordinary Create/Edit Card screen owns the Template selector, deck/tags,
durable draft recovery, managed assets, Save/Save + another and the shared
**Preview card** action. Selecting **Image occlusion** mounts a purpose-built
rectangle editor without creating a separate save workflow.

`src/editor/occlusion/` owns normalized geometry, pointer gestures, region history,
the authoring SVG workspace and the read-only study viewport. The authoring editor
supports mouse/touch/pen drawing, tap-two-corners, selection, move/resize/delete,
undo/redo, zoom, pan, fit and zoom-to-region. Those authoring controls are distinct
from the deliberately minimal Practice controls.

Each occlusion question owns rich Question and Answer fields plus an optional
Explanation exposed through **Add explanation**. One question can target several
regions and several questions can share a region. Moving/resizing geometry
preserves the scheduled question identity. Both **hide all / reveal this target**
and **hide target only** modes are supported.

The editor no longer carries a second inline study preview. **Preview card** in
the page header opens the same read-only study presentation used by the other
interactive question editors. Image preview uses the native masked renderer;
Reveal answer in preview is local only and does not create an attempt, review or
schedule update.

## Practice image security and loading

Before reveal, the WebView receives a native-generated masked PNG, never the
original source image hidden underneath CSS. The request is authorized from the
captured session/card/instance/revision and native saved attempt state chooses the
hidden or revealed phase. Cache hits still recheck source ownership, availability
and revision.

Practice now requests a **1024 px initial masked raster** for faster first display.
As the learner zooms, it automatically refines to 2048 px and then 4096 px. A
usable same-revision raster remains visible while the sharper version is prepared;
a revision/target change invalidates the old pixels immediately. Native rendering
also keeps one short-lived, bounded decoded-source cache keyed by the actual source
bytes and uses a faster lossless PNG path for interactive-size derivatives. These
changes reduce repeated decode/encode work without weakening masking or asset
authorization.

Practice exposes only three small image controls:

- zoom out icon
- zoom in icon
- drag-to-pan icon

Dragging the image pans it, pinch gestures zoom it and focused-image arrow keys
pan it. There is no Practice **Enlarge image**, **Load sharper image**, **Fit image**,
**Zoom to target**, target dropdown or visible instructional paragraph. Sharpening
is automatic. The authoring workspace still has its richer geometry/navigation
controls because they are required for precise mask editing.

Image Practice remains **recall -> Reveal answer -> self-rate**. It does not invent
an objective correctness verdict. Reveal and review retries preserve their
operation identities, and a committed reveal stays committed if a later raster or
interval refresh fails.

## Build the intended source

From an existing configured checkout, inspect local edits first. Do not discard or
reset unrelated work.

```bash
git status --short
git fetch origin --prune
git switch agent/interactive-question-types
git pull --ff-only
git rev-parse HEAD
```

Compare the SHA with the exact handoff SHA. Stop on a mismatch or failed
fast-forward. Then build and launch through the maintained wrappers:

```bash
./scripts/build.sh
./run-app.sh
```

Back up a valuable library before testing migrations. For disposable file-backed
state, use one isolated XDG root and reuse it for restart/resume tests:

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

No API provider or credential is needed for these tests.

## Desktop acceptance checklist

Use a disposable deck and a synthetic/non-private labeled image.

| Test | Expected result |
| --- | --- |
| Image import | PNG/JPEG/WebP imports locally. Cancel/failure preserves the draft and never creates a partial saved note. |
| Region creation | Draw rectangle, Tap two corners and Add rectangle all create image-relative regions; resize/window changes do not move them off the source. |
| Geometry/history | Drag/move/resize and explicit adjustment controls work. Undo/redo changes geometry without rolling back newer question text. |
| Multi-target identity | One question can target multiple regions and another can reuse a region. Study-card count follows questions, not rectangles. |
| Mask modes | Preview card shows both mask modes correctly. In hide-all mode unrelated masks remain hidden when the current target reveals. |
| Optional explanation | Explanation controls start collapsed; Add explanation opens them and collapse/reopen preserves entered content. |
| Save/reopen | Save, reopen and edit geometry without changing the question's schedule identity. Save + another starts a clean image draft. |
| Preview card | The header Preview card shows the masked presentation and authored answer without creating a Practice review. |
| Practice first paint | A masked image appears without exposing the original. Initial display uses the smaller derivative; zooming automatically refines it without blanking the already usable image. |
| Practice controls | Only zoom-out, zoom-in and pan icon controls appear. Drag/pinch/keyboard image navigation never reveals or rates the answer. |
| Reveal/resume | Reveal shows only the current authorized target/answer. Restart before rating and resume to the already revealed state rather than a fresh attempt. |
| Existing cards | Basic, Cloze and objective interactive questions still use their normal study/review behavior in the same deck. |

Repeat the main flow offline, in supported themes, and at compact desktop width.
A green automated check does not establish visual or physical-touch acceptance.

## Android physical-device test

From the same exact source checkout:

```bash
adb devices -l
pnpm android:doctor
pnpm android:install
```

With multiple authorized devices:

```bash
bash ./scripts/android-install.sh --serial "REPLACE_WITH_ADB_SERIAL"
```

The wrapper builds/installs the ARM64 debug app with replacement. Do not uninstall
or clear app data as routine setup. Create the image question directly on the phone
while typed interactive transfer remains incompatible.

Test touch drawing and corner resizing in the **editor**, two-finger pan/pinch,
portrait/landscape changes, software-keyboard interaction and Android Back. In
**Practice**, confirm only the three compact image controls appear, initial masked
loading is acceptable, zoom refinement does not blank the image, and reveal/rating
still work offline.

## Automated checks and remaining boundaries

Normal iteration qualification is:

```bash
./scripts/check-fast.sh
```

Focused frontend checks for this area include the existing occlusion/editor and
Practice tests, notably:

```bash
pnpm exec vitest run \
  src/editor/occlusion \
  src/screens/CardEditorOcclusion.test.tsx \
  src/screens/CardEditorQuestionPreview.test.tsx \
  src/screens/practice/ImageOcclusionActivity.test.tsx \
  src/screens/practice/useOcclusionReveal.test.tsx \
  src/screens/practice/usePracticeOcclusionImage.test.tsx
```

Full Linux qualification is `./scripts/check.sh`; Android compile/package and
emulator gates are `pnpm android:test` and `pnpm android:smoke`. They remain final
candidate gates rather than requirements for this Stage 6 hands-on checkpoint.

Typed `.studydeck`/`.ostudy.json` exchange and complete interactive device sync are
still unfinished. Current device sync rejects an unsupported typed graph before
send/merge instead of silently flattening it. Whole-library backup is a different
contract. Retain a pre-upgrade backup before upgrading a valuable library to the
schema used by this branch.

When reporting acceptance issues, include the exact Git SHA, desktop/Android build
mode, device/WebView environment, smallest reproduction and expected versus actual
behavior. Do not attach a live library, private source image or API credentials.
