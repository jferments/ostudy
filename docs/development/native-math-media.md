# Native mathematics and managed audio/video

Status: implementation contract for the replacement of PR #95. Publication,
qualification, and human acceptance are recorded separately in the replacement PR.

## Baseline and preservation

Start from merged PR #97: `d658bf8507b8b4d62f0a062621e8d861d78bae0a`.
Its tree is `87ba43a71ac1d1b7ffccd3a8a537d3a4616d4e19`. PR #95 at
`e18fc0d4bffed57258e643a9d9b04f703edf7fb0` is a design/test reference,
not a tree to merge or copy over the current application.

Preserve PR #94's optional grounding, exact source permissions, hidden answers,
private conversation retrieval, evidence offsets/hashes, checkpoint reuse and
application-owned document jobs. Preserve PR #96's editor-neutral adapters,
oStudy UI facades and semantic Tailwind ownership. Preserve PR #97's accepted
presentation, native startup, system-owned Android USB permission, authenticated
transport, byte-accurate progress, and compact navigation. Scheduling, review
identities, pagination, and post-commit truth are unchanged.

Every changed file needs a content-capability justification. No old startup,
Tutor, sync, UI, lockfile, migration, or temporary-file replacement is allowed.
Current critical files are edited from exact verified bytes; candidate diffs and
blob identities are checked before expected-parent Git-data publication.

## Architecture

Anki and native authoring feed application-owned typed content. Native validation
and the existing managed-asset transaction remain the durable authority.

Rich root schema 3 adds `inlineMath` / `blockMath` with bounded delimiter-free
`latex`, and `audio` / `video` with a managed `assetId` and canonical
`study-asset://` reference. Rendered HTML and platform URLs are derived only.
Existing root schemas 1/2 remain readable. Root schema, transport envelope,
SQLite schema, package version and sync compatibility are separate contracts.

Extend the current native validator rather than replacing the current schema-2
rules. Frontend canonicalization, hydration, asset collection, emptiness, drafts,
search projections and duplicate identity must all understand the new content.
Opening and saving imported content must not drop an unknown node or orphan media.

Math normalization is independent of Anki. Its target contract includes explicit
supported TeX and a bounded presentation-MathML subset, not a system TeX program
or arbitrary XML. The current implemented subset is documented below. Preserve child order, operator meaning, and source evidence. Unsupported
expressions stay explicit; conversion does not prove mathematical correctness.

Local KaTeX behind an application-owned renderer serves cards, previews,
Practice, Tutor and guides. Trust is disabled, macro/work bounds are finite,
macros do not leak across notes, and errors/source fallbacks are escaped text.
Code fences and code spans stay literal. Math is tokenized before Markdown
escaping loses delimiters; currency and unfinished expressions remain text.
MathLive is mandatory from the first usable formula editor in this PR. It owns
structured editing and the mathematical keyboard; do not substitute a custom
guided-form editor or defer MathLive to another PR. Integrate it with the current
shared Dialog, inline/block placement, bounded source validation, selection/focus
restoration, cancellation without mutation and one undoable save. Runtime fonts
and assets must be packaged locally. Persistence remains bounded LaTeX; KaTeX
is the read-only renderer, not a replacement editor.

Media uses existing draft/import ownership and local asset serving. Acquisition
must work for native authors as well as Anki. Admission inspects bounded container
and track data; signatures/extensions alone do not certify codecs. Browser decode
capability remains distinct from valid storage. Keep 25 MiB per media asset in the
initial profile, with aggregate limits at every transfer boundary.

Native audio/video elements provide controls; oStudy owns local URLs, metadata
policy and lifecycle. No autoplay, remote embedding, donor events or scripts.
Inactive answer media must not load or reveal filenames. Player gestures/keys
must not flip or grade cards. Face/card/route changes pause and release media.
List views do not create a player for every card.

The local asset protocol supports GET/HEAD and single ranges with accurate lengths,
path/lifecycle isolation and bounded buffers. A whole-file/open-ended request is
not constant-memory streaming. File size and concurrent allocations are bounded;
file reads do not hold the database mutex. No unrestricted local HTTP server.

Anki `[sound:]`, packaged audio/video/source elements and supported math normalize
into these native nodes. Existing template semantics, static-JS salvage, cloze
fallback, transactional replay and post-commit reporting remain. Reports distinguish
missing/unsupported media, unsupported math and unsupported active HTML. Conversion
progress says prepared/processed/skipped; imported means durable commit succeeded.

Document adapters preserve identifiable math without rewriting hashed source text.
Existing generation instructions retain signs, units, limits and assumptions in
supported TeX. Model-produced TeX is interpretation, not source quotation. No new
provider, permission, implicit cloud call or second semantic-analysis pipeline.
The native PDF exporter is separate from browser KaTeX: preserve readable equation
source and disclose source-only output rather than silently dropping formulas.

## Current implementation boundary

The concrete Tiptap schema retains all four rich atoms through ordinary editing
and save/reopen, with text/clipboard serialization and protection against marks
that native atom validation rejects. Media placeholders are still inert and omit
runtime URLs and titles; audio/video acquisition and playback remain unfinished.

### Formula authoring and card rendering

The editor toolbar's **Formula** action opens the real MathLive input in the
existing shared Dialog. Double-clicking a formula or pressing Enter on a selected
formula opens the same editor. MathLive owns structured input and its mathematical
keyboard; oStudy owns inline/block placement, local draft state, validation and
Save/Cancel. Its optional context menu is disabled because it creates a separate
untracked overlay and offers out-of-scope computation/speech. This is not a custom
formula editor. More elaborate menu integration is not claimed by this increment.

`src/math/mathlive.ts` loads the bundled MathLive module only on authoring intent.
It owns an abortable, exclusive keyboard lifecycle inside the shared Dialog. A
late import after dismissal cannot attach a field or take focus. Escape/platform
Back hides an open math keyboard first; a subsequent dismissal cancels the draft.
Explicit Cancel, backdrop and close-button dismissal cancel immediately. Cleanup
releases listeners, the field and the global keyboard container. StrictMode view
cleanup does not consume the editor transaction. Disabling the card editor closes
its formula session. Clipboard input admits bounded plain LaTeX, not donor HTML.

`EditorAdapter.beginFormulaEdit()` captures document identity and selection before
the dialog takes focus. Save does not use a later cursor position. Placement
changes preserve surrounding text and quote/list/table containers. Unchanged Save
and Cancel do not add content/history entries. One changed Save is one undo group,
isolated from earlier/later typing. Replaced/read-only/destroyed editors reject
stale writes. Initial source is retained exactly until actual input, not silently
normalized merely by opening MathLive. Code, multi-block and whole-document
selections remain outside this operation.

`src/math/render.ts` is the shared KaTeX rendering boundary, now used by card
NodeViews (including read-only cards/Practice) and the dialog preview. Stored JSON
and clipboard HTML keep source-only atoms, never typesetter DOM. The renderer uses
untrusted mode, independent macro state, finite expansion/size bounds and a bounded
source/nesting admission step. Unsupported commands, invalid source and parser
failures display escaped source with an accessible explanation. Renderability is
not mathematical correctness, and storage admission is intentionally broader than
rendering admission. Source-bearing failures are not logged.

MathLive **0.110.0** and KaTeX **0.18.7** are exact pinned MIT dependencies. The
lockfile also pins their resolved transitive graph; existing dependency resolutions
are preserved. Their distributed font CSS is imported by the existing foundation
stylesheet, so Vite packages and rewrites font assets locally. No runtime CDN is
used. MathLive's dynamic font/sound loaders, compute engine and URL-opening hook
are disabled. The native Content Security Policy is unchanged. Third-party package
licenses remain with their distributions; no runtime binary or downloaded font is
committed as product source.

Focused tests cover real KaTeX output/fallbacks and actual Tiptap serialization,
selection, undo/redo and shared-dialog behavior. The MathLive port is stubbed for
lifecycle tests; those tests do not establish physical-keyboard, touch keyboard,
assistive-technology, theme contrast or target-WebView font/geometry acceptance.
Desktop and physical Android testing, complete application packaging and full
platform qualification remain explicit acceptance work. Source extraction, MathML
normalization and native-PDF math integration remain unfinished; card and Markdown
rendering do not complete those paths.

```bash
pnpm vitest run src/math/render.test.ts src/math/mathlive.test.ts src/editor/FormulaDialog.test.tsx src/editor/formula.test.ts src/editor/tiptap/formulaSession.test.ts src/editor/richText.mathMedia.test.ts
```

On an isolated test library, insert a fraction using MathLive, switch placement,
save/reopen and undo/redo. Select the atom and press Enter or double-click to edit.
Cancel changes and verify the old formula remains. With networking disabled, check
font rendering and the mathematical keyboard on desktop and physical Android,
including first-Back keyboard dismissal, second-Back dialog dismissal, compact
layout, all themes and focus restoration. Use the maintained build/run wrappers
in [Local testing and handoff](local-testing-and-handoff.md); a worktree alone does
not isolate application data. Passing automated tests are not human acceptance.

### Tutor and study-guide mathematical text

`src/components/StudyMarkdown.tsx` owns the shared safe Markdown reader. The Tutor
facade, saved/new ordinary replies, feedback, and guide structured sections,
notes, active overrides and preserved conflicts use the same math renderer as
cards. Guide headings use inline-only Markdown to preserve their host structure;
source locators and navigation labels remain literal. The hint-only Tutor view
remains deliberately plain, with its existing source/answer visibility rules.

An isolated Marked lexer in `src/math/markdown.ts` recognizes `\(...\)`,
`\[...\]` and `$$...$$` before ordinary Markdown escaping. Single-dollar text
is deliberately not math, so prices and currency cannot absorb surrounding prose.
Code spans, fences, indented code and raw code/script blocks remain literal.
Unfinished expressions retain their original backslashes until complete. Math
operators/subscripts are masked from surrounding Markdown emphasis, not rewritten
in the stored text. Unsupported KaTeX commands use the existing escaped fallback.
The reader never inserts Marked-generated HTML or loads Markdown images.

Each field/message has a 24,000-character presentation bound, at most 64 formulas,
and at most 32,768 total formula-source UTF-8 bytes. Exceeding a shared budget or
failing to tokenize displays the entire original field/message as plain text,
without truncation. Each formula retains the stricter existing renderer bounds.
Budgets and macros are not shared across messages, guides or renders. Unchanged
formula components reuse their DOM instead of typesetting again on unrelated UI
updates. There is no MathLive load, provider request or source lookup on this path.

This is a read-only presentation change: guide edit fields and saved overlays
retain the original source, not KaTeX HTML/MathML. Generated hashes, locators,
checkpoints, source citations and permission decisions are not rewritten. The
native PDF exporter does not use this browser renderer and still needs its own
source-fallback/disclosure acceptance. Existing saved content needs no migration.

```bash
pnpm vitest run src/math/markdown.test.ts src/screens/tutor/TutorMarkdown.test.tsx src/screens/tutor/TutorPresentation.test.tsx src/screens/StudyGuideDialog.test.tsx
```

Regressions cover real KaTeX output, code/currency/unfinished input, hostile math,
aggregate bounds, update cleanup, saved/generated guides, note Save/Cancel with
exact source, and unchanged historical citation disabling. Native database and
physical-device acceptance are not established by mocked gateway fixtures.

### Generated-card mathematical source

Document-generated Basic cards and direct `.ostudy.json` Basic imports share
`authoring/card_types_and_templates/generated_import/markdown.rs`. Its mathematical
source protection runs before Markdown emphasis, links or block splitting.
Explicit `\(...\)`, `\[...\]` and `$$...$$` become the existing native
`inlineMath`/`blockMath` atoms and therefore use the same KaTeX renderer and MathLive
editor as imported Anki formulas. This is conversion, not another math renderer.

The temporary conversion markers cannot collide with source text and must each
be restored exactly once before persistence. Delimiter-free source retains its
operators, subscripts and line breaks after the existing newline normalization.
Formatting stays on surrounding prose, not math atoms. Display formulas split
text blocks while preserving their heading, quote or list context. Supported code
spans/fences, indented source lines, escaped openings and single-dollar currency
are not interpreted as formulas; link destinations retain normal link validation.
No HTML is admitted outside inert formula source. No TeX program executes.

Each card side admits at most 64 formulas and 32,768 aggregate UTF-8 formula bytes,
with the existing 16,384-byte per-formula limit. Empty, invalid-control, oversized
or unclosed final formulas report the affected card-side path. Final JSON imports
remain whole-file validated before mutation; streaming Tutor text has its separate
read-only incomplete-expression fallback. Root schema 3 is added only for converted
math; the transport envelope and old math-free documents are unchanged. Rich cloze
and universal conversion of arbitrary math-looking prose remain outside this step.

```bash
cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib document_generated_math
```

Regressions cover source/emphasis preservation, display placement, quote/list
containers, code/currency, adversarial marker collisions, source limits and exact
native SQLite persistence with reverse presentations. A mixed invalid import is
rejected without writing notes. These tests do not establish provider transcription
accuracy, file-backed database reopening or physical-device acceptance.

### Extraction, MathML and the PDF export boundary

MathML is an incoming structured notation, not a competing renderer or a new
storage format. The intended bounded presentation-MathML adapter converts supported
structures to the same LaTeX atoms; it is still unfinished. KaTeX also derives MathML
alongside its visual HTML for accessibility. Neither operation is equation OCR.

PDF ingestion currently uses `pdf_backend/extraction.rs` for native text/semantic
partitioning, alternate native-text recovery through `pdf_document_ingestion/recovery.rs`,
page rendering and selective platform OCR through `pdf_document_ingestion/pipeline.rs`.
Desktop OCR uses local Tesseract; Android uses bundled ML Kit text recognition.
These are text-recovery paths, not a guarantee of correct fraction, matrix or
superscript reconstruction. Rendered source images remain available as bounded
visual evidence when the selected task permits an image-capable provider.

Non-PDF sources use the isolated Xberg adapter. Its normalization maps upstream
`Formula` nodes to canonical equation blocks while preserving the returned text;
this does not prove every DOCX/EPUB/HTML equation reaches such a node or is LaTeX.
Structured equation reconstruction, source fidelity and model interpretation must
be evaluated separately. Do not rewrite existing hashed evidence to substitute a
model's inferred equation. Generated-card conversion preserves supplied TeX; it
cannot recover an equation already lost or mistranscribed during extraction.

"PDF source-only fallback" refers to **exporting a generated study guide**, not
importing or extracting a source PDF. `document_study_guide_export.rs` currently
writes selectable text using the native oxidize-pdf backend; it does not execute
browser KaTeX or lay out its HTML/CSS. The fallback means readable equation source
rather than silently omitted equations. Explicit export disclosure and typeset
export integration are not implemented by the generated-card converter. A future
typeset PDF path should reuse the application-owned math-rendering boundary rather
than introduce a second equation implementation. Original PDF viewing remains a
faithful page-rendering path, not this study-guide export path.

### Native Anki math preservation

The shared native math boundary now recognizes explicit `\(...\)` / `\[...\]`
and legacy Anki `[$]...[/$]` / `[$$]...[/$$]` in visible HTML text. It emits
bounded, delimiter-free LaTeX without rewriting the mathematical source. Inline
formulas remain in their paragraph; block formulas split the paragraph without
losing text on either side. Surrounding formatting stays on text, not formula
atoms. Only documents with converted math opt into rich-root schema 3; the
transport envelope remains unchanged.

HTML code/preformatted regions and dollar currency stay literal. Unclosed or
HTML-split delimiters remain visible source with a report warning. `[latex]`
programs remain literal with an explicit warning: system TeX never executes.
Empty, over-limit or prohibited-control formula sources reject the affected card,
not otherwise supported cards in the package. This is source admission, not
mathematical correctness or renderer-support validation. MathML conversion is
still unfinished, as are packaged audio/video conversion and playback.

Formula-only static content can survive the existing JavaScript-stripping path;
JavaScript is still never executed, and active/hidden HTML restrictions remain.
Math-bearing cloze presentations use the existing rich Basic fallback instead of
flattening typed formulas into the plain-text native cloze representation. This
does not introduce a new rich-cloze implementation.

`src-tauri/src/cards/math.rs` contains normalizer regressions.
`src-tauri/src/studydeck/anki/math_import_tests.rs` exercises generated real Anki
packages, reporting, schema compatibility, native import/replay, fresh schedules
and exact formula nodes after database reopening. Its `document_` test names are
included in the existing fast gate. Focused native commands, after normal toolchain
and PDFium setup, are:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib cards::math::
cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib document_anki_math
```

These native import tests do not establish card-UI or device acceptance. Imported
formula atoms now use the shared card renderer and captured MathLive edit session
described above; unsupported rendering still preserves visible source. The remaining
Anki media and MathML work stays in this PR.

## Acceptance criteria

| ID | Required outcome |
| --- | --- |
| AC-01 | Fresh post-#97 baseline; protected #94/#96/#97 behavior and narrow diff. |
| AC-02 | Native and frontend schema-3 round trips, old-schema compatibility, projections and asset ownership. |
| AC-03 | Safe offline math rendering with visible invalid/unsupported-source fallback. |
| AC-04 | MathLive insert/edit formula UI using current editor/overlay contracts and one undoable save. |
| AC-05 | Native local audio/video acquisition, bounded inspection and durable lifecycle. |
| AC-06 | Native players, inactive-face isolation, keyboard/gesture safety and resource release. |
| AC-07 | Asset-ID-only GET/HEAD/range serving, allocation bounds and path/lifecycle tests. |
| AC-08 | Safe Anki math/media conversion and accurate bounded reporting/progress. |
| AC-09 | Shared math in saved/new Tutor and guides without privacy/evidence changes. |
| AC-10 | Identifiable source math and generated math preservation, correct escaping, PDF fallback disclosure. |
| AC-11 | Interchange, backup, sync compatibility, deletion/Undo and media-only identity tests. |
| AC-12 | Current documentation, exact-head Linux/Android qualification and explicit human playback acceptance. |

## Implementation sequence

1. Native/frontend content contract and backward-compatibility tests.
2. Math normalization, local rendering, editor nodes and MathLive authoring.
3. Media inspection/acquisition, player lifecycle and bounded asset serving.
4. Anki integration through real staging and generated packages.
5. Tutor/guide/source math and PDF source-only disclosure.
6. Round trips, privacy, resource limits, documentation and complete diff audit.
7. Freeze, static/fast, required full Linux/Android gates, then human acceptance.

Use genuinely decodable generated media fixtures for playback, not just fake
headers. Test import -> display -> edit unrelated text -> save -> reopen. Test
malformed MathML/TeX, escaped currency/code, media-only cards, rollback/replay,
missing media, old receivers, inactive-answer isolation and reference cleanup.
A passing parser test does not prove real native playback or live-provider quality.

## Non-goals and handoff

No Image Occlusion, rich-cloze redesign, typed grading, recording, transcription,
pronunciation scoring, remote playback, transcoding, custom formula editors, universal
PDF/OMML equation reconstruction, or typeset-native-PDF parity. No automatic merge.

Source rollback does not make old clients understand new rich nodes. Preserve a
compatible backup before testing. Incompatible readers must fail before mutation,
not silently flatten content. PR #95 stays unmerged as provenance until superseded.
Finish source and documentation before terminal qualification; do not restart the
expensive matrix for opportunistic cleanup. Report exact source, actual gates,
manual desktop/physical-Android playback boundaries and the next unfinished step.
