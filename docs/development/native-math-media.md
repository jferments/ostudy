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

Math normalization is independent of Anki. It accepts explicit supported TeX
and a bounded presentation-MathML subset, not a system TeX program or arbitrary
XML. Preserve child order, operator meaning, and source evidence. Unsupported
expressions stay explicit; conversion does not prove mathematical correctness.

Local KaTeX behind an application-owned renderer serves cards, previews,
Practice, Tutor and guides. Trust is disabled, macro/work bounds are finite,
macros do not leak across notes, and errors/source fallbacks are escaped text.
Code fences and code spans stay literal. Math is tokenized before Markdown
escaping loses delimiters; currency and unfinished expressions remain text.
The initial formula dialog uses current shared controls, guided common forms,
inline/block placement, source editing and live preview. MathLive is a future
structural editor, not a new persistence format.

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

## Acceptance criteria

| ID | Required outcome |
| --- | --- |
| AC-01 | Fresh post-#97 baseline; protected #94/#96/#97 behavior and narrow diff. |
| AC-02 | Native and frontend schema-3 round trips, old-schema compatibility, projections and asset ownership. |
| AC-03 | Safe offline math rendering with visible invalid/unsupported-source fallback. |
| AC-04 | Guided insert/edit formula UI using current editor/overlay contracts and one undoable save. |
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
2. Math normalization, local rendering, editor nodes and guided authoring.
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
pronunciation scoring, remote playback, transcoding, full MathLive UX, universal
PDF/OMML equation reconstruction, or typeset-native-PDF parity. No automatic merge.

Source rollback does not make old clients understand new rich nodes. Preserve a
compatible backup before testing. Incompatible readers must fail before mutation,
not silently flatten content. PR #95 stays unmerged as provenance until superseded.
Finish source and documentation before terminal qualification; do not restart the
expensive matrix for opportunistic cleanup. Report exact source, actual gates,
manual desktop/physical-Android playback boundaries and the next unfinished step.
