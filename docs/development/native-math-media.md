# Native mathematics and managed audio/video

Status: source implementation is complete through AC-11; terminal platform
qualification and human acceptance are recorded separately in the replacement PR.

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
or arbitrary XML. Preserve child order, operator meaning, and source evidence.
Unsupported expressions stay explicit; conversion does not prove mathematical
correctness.

Local KaTeX behind an application-owned renderer serves cards, previews,
Practice, Tutor and guides. Trust is disabled, macro/work bounds are finite,
macros do not leak across notes, and errors/source fallbacks are escaped text.
Code fences and code spans stay literal. Math is tokenized before Markdown
escaping loses delimiters; currency and unfinished expressions remain text.
MathLive owns structured editing and the mathematical keyboard; oStudy owns
inline/block placement, local draft state, validation and Save/Cancel. Runtime
fonts and assets are packaged locally. Persistence remains bounded LaTeX; KaTeX
is the read-only renderer, not a replacement editor.

Media uses existing draft/import ownership and local asset serving. Acquisition
works for native authors as well as Anki. Admission inspects bounded container and
track data; signatures/extensions alone do not certify codecs. Browser decode
capability remains distinct from valid storage. The initial profile keeps a 25 MiB
per-media limit, with aggregate limits at transfer boundaries.

Native audio/video elements provide controls; oStudy owns local URLs, metadata
policy and lifecycle. No autoplay, remote embedding, donor events or scripts.
Inactive answer media must not load or reveal filenames. Player gestures/keys
must not flip or grade cards. Face/card/route changes pause and release media.
List views do not create a player for every card.

Anki `[sound:]`, packaged audio/video/source elements and supported math normalize
into native nodes. Existing template semantics, static-JS salvage, cloze fallback,
transactional replay and post-commit reporting remain. Reports distinguish
missing/unsupported media, unsupported math and unsupported active HTML.

Document adapters preserve identifiable math without rewriting hashed source text.
Existing generation instructions retain signs, units, limits and assumptions in
supported TeX. Model-produced TeX is interpretation, not source quotation. No new
provider, permission, implicit cloud call or second semantic-analysis pipeline.
The native PDF exporter remains separate from browser KaTeX and preserves readable
equation source rather than silently dropping formulas.

## Current implementation boundary

The concrete Tiptap schema retains all four rich atoms through ordinary editing
and save/reopen, with text/clipboard serialization and protection against marks
that native atom validation rejects. Media uses native players only for admitted
managed assets; unavailable assets remain inert. Native acquisition, Anki
conversion, package/sync/backup transport, shared draft/promotion/reconciliation
lifecycle, inactive-face teardown, media-aware duplicate identity, startup media
recovery and raw-IPC-to-Blob playback are implemented. Target-device acceptance
remains separate from source qualification.

Managed-media duplicate equality keeps the historical normalized-text hash as the
candidate index, then compares ordered managed node kind plus `assetId` on each
side. Filenames, paths, runtime URLs, titles and player metadata do not participate.
Direct create/update, move/copy and rich template generation use this equality;
the public text-only advisory defers media-placeholder cases to the authoritative
transaction.

### Formula authoring and card rendering

The editor toolbar's **Formula** action opens the real MathLive input in the
existing shared Dialog. Double-clicking a formula or pressing Enter on a selected
formula opens the same editor. MathLive owns structured input and its mathematical
keyboard; oStudy owns inline/block placement, local draft state, validation and
Save/Cancel. Its optional context menu is disabled because it creates a separate
untracked overlay and offers out-of-scope computation/speech.

The formula dialog is ordered for touch authoring as well as desktop use. The
MathLive formula field is first. Its right edge contains an accessible keyboard
icon that toggles the full MathLive virtual keyboard without replacing the active
formula selection. The keyboard container is immediately below the field and
above the Structures/Symbols/Relations/Greek/Calculus toolbox, so the expression
being edited remains adjacent to the keyboard on compact screens. On Android the
full MathLive keyboard is shown automatically when the editor becomes ready; it
can still be collapsed/reopened from the field icon. Desktop-only keyboard
shortcut help is suppressed on mobile runtimes, where those shortcuts are not an
appropriate primary interaction model. Formula placement (`Inline with text` /
`On its own line`) is intentionally the final authoring control before the dialog
actions.

`src/math/mathlive.ts` loads the bundled MathLive module only on authoring intent.
It owns an abortable, exclusive keyboard lifecycle inside the shared Dialog.
Mounted-only MathLive options are configured from the vendor `mount` event,
subscribed before DOM attachment; loading does not finish until that event.
Abort during loading or mounting releases the field and keyboard ownership.
A late import after dismissal cannot attach a field or take focus. Escape/platform
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

`src/math/render.ts` is the shared KaTeX rendering boundary, used by card NodeViews
(including read-only cards/Practice) and the dialog preview. Stored JSON and
clipboard HTML keep source-only atoms, never typesetter DOM. The renderer uses
untrusted mode, independent macro state, finite expansion/size bounds and a bounded
source/nesting admission step. Unsupported commands, invalid source and parser
failures display escaped source with an accessible explanation. Renderability is
not mathematical correctness, and storage admission is intentionally broader than
rendering admission. Source-bearing failures are not logged.

MathLive **0.110.0** and KaTeX **0.18.7** are exact pinned MIT dependencies. Their
font CSS is imported by the foundation stylesheet, so Vite packages and rewrites
font assets locally. No runtime CDN is used. MathLive's dynamic font/sound loaders,
compute engine and URL-opening hook are disabled.

Focused tests cover real KaTeX output/fallbacks and actual Tiptap serialization,
selection, undo/redo, shared-dialog behavior, mobile ordering, Android default
keyboard visibility, the in-field keyboard toggle, and suppression of desktop
shortcut help on mobile. A separate integration test loads the packaged MathLive
browser build and exercises mount, source, insert and reopen; only browser APIs
missing in jsdom are supplied by the test environment. Physical touch keyboard,
assistive-technology, target-WebView font and geometry behavior remain manual
acceptance.

```bash
pnpm vitest run src/math/render.test.ts src/math/mathlive.test.ts src/editor/FormulaDialog.test.tsx src/editor/FormulaDialog.layout.test.tsx src/editor/FormulaToolbox.test.tsx src/editor/formula.test.ts src/editor/tiptap/formulaSession.test.ts src/editor/richText.mathMedia.test.ts
```

On an isolated test library, insert a fraction using MathLive, save/reopen and
undo/redo. On Android, verify the keyboard opens expanded immediately below the
formula field, the field remains visible, the in-field keyboard icon collapses and
reopens it, desktop shortcut help is absent, and placement is the last authoring
control. Verify first Back hides an open MathLive keyboard and subsequent Back
cancels/dismisses according to the shared dialog contract. Passing automated tests
are not physical-device acceptance.

### Tutor and study-guide mathematical text

`src/components/StudyMarkdown.tsx` owns the shared safe Markdown reader. Tutor and
study-guide presentation use the same math renderer as cards. Guide headings use
inline-only Markdown to preserve their host structure; source locators and
navigation labels remain literal. Math presentation remains read-only and does not
rewrite saved source.

An isolated Marked lexer in `src/math/markdown.ts` recognizes `\(...\)`,
`\[...\]`, `$$...$$` and complete `$...$` inline pairs before ordinary Markdown
escaping. Single-dollar pairs are guarded so ordinary prices, escaped dollars and
code examples remain literal. Unsupported KaTeX commands use the escaped fallback.
The reader never inserts donor HTML or loads Markdown images.

Each field/message has bounded character/formula budgets. Exceeding a shared budget
or failing to tokenize displays the original field/message as plain text rather
than truncating it. Budgets and macros are not shared across messages, guides or
renders. There is no MathLive load, provider request or source lookup on this path.

### Generated-card mathematical source

Document-generated Basic cards and direct `.ostudy.json` Basic imports share the
generated-import Markdown path. Explicit supported math delimiters become native
`inlineMath`/`blockMath` atoms and therefore use the same KaTeX renderer and
MathLive editor as imported Anki formulas. This is conversion, not another math
renderer. Code, escaped delimiters and ordinary currency remain literal.

### Extraction, MathML and the PDF export boundary

MathML is incoming structured notation, not a competing renderer or a new storage
format. The bounded presentation-MathML adapter converts supported structures to
the same LaTeX atoms. Unsupported structure is not flattened into guessed math.
KaTeX also derives MathML alongside its visual HTML for accessibility. Neither
operation is equation OCR.

PDF ingestion's native text recovery, rendered-page evidence and selective OCR are
text/evidence paths, not a guarantee of fraction, matrix or superscript
reconstruction. Dedicated equation recognition and universal source-format
fidelity remain deferred. Generated-card conversion preserves supplied TeX; it
cannot recover math already lost during extraction.

Study-guide PDF export uses the native PDF backend rather than browser KaTeX. Its
fallback preserves readable formula source and discloses source-only math output
instead of silently omitting equations. Final target-system visual acceptance
remains required.
