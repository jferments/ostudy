# Rich math and media content

## Purpose

Flashcards, study guides, generated educational content, and future tutor/chat surfaces need one safe representation for mathematical notation and local audio/video. Anki import is a consumer of that representation, not its owner. Import adapters must convert supported source syntax into native rich-document nodes and managed assets rather than preserving executable donor HTML.

This design extends the editor-neutral `RichDocument` boundary. It does not embed an Anki renderer, execute imported JavaScript or TeX programs, fetch remote media, or introduce a second asset lifecycle.

## Canonical rich-document nodes

Rich-document schema version 3 adds four node types:

- `inlineMath` — inline atomic node with one bounded `latex` attribute.
- `blockMath` — block atomic node with one bounded `latex` attribute.
- `audio` — block atomic node referring to one managed local asset by `assetId` and canonical `study-asset://` source.
- `video` — block atomic node referring to one managed local asset by the same identity contract, with optional bounded display dimensions.

The source of truth for math is canonical LaTeX text, not generated HTML, SVG, MathML DOM, or a renderer-specific object graph. This keeps persistence independent of KaTeX/MathJax/MathLive and makes it possible to change the editing or rendering implementation without rewriting notes.

Math nodes contribute their LaTeX source to authoritative searchable/plain-text projections. Audio/video nodes contribute their managed asset identities and accessible title/filename metadata, so a media-only card is not incorrectly considered empty.

## Math rendering and authoring

The current Tiptap 3 line has an official Mathematics extension whose inline/block nodes store LaTeX and render with KaTeX. oStudy uses that local renderer with `throwOnError: false`, no trusted HTML commands, no remote resources, and bounded input. KaTeX CSS/assets are packaged with the application so math works offline on desktop and mobile.

KaTeX is a renderer, not the long-term graphical authoring experience. A future formula dialog should use MathLive's `<math-field>` as an editor surface because it provides structured caret navigation, fractions/roots/matrices, touch virtual keyboards, and accessibility without requiring oStudy to reimplement a mature mathematical input engine. MathLive can emit LaTeX into the same node attribute, so adopting it later does not change persistence. A plain LaTeX source field may remain available as an advanced/debugging mode, not the default expectation for learners.

Do not build a bespoke full formula editor unless a concrete unsupported MathLive requirement appears. Cursor movement, nested selection, matrix editing, mobile keyboards, accessibility, and IME behavior are specialized enough that owning them would add risk without improving oStudy's domain model.

## Accepted math input forms

Native persistence is LaTeX. Import/ingestion boundaries may recognize and normalize:

- Anki MathJax delimiters `\\(...\\)` and `\\[...\\]`.
- safe LaTeX source already represented as text/data rather than executable generated images.
- a bounded presentation-MathML subset that can be converted structurally to equivalent LaTeX.

Unknown or ambiguous MathML must remain explicit rather than being flattened into misleading text. TeX is never passed to a system LaTeX executable. Commands that require trusted HTML/network behavior are not enabled by the renderer.

Document ingestion should preserve equations as typed math spans in its canonical text/semantic representation when the source adapter can identify them. Office documents commonly encode equations as OMML and PDFs may expose equations as text, vector glyphs, or images; these require source-adapter-specific extraction. The document pipeline should normalize identified equations to LaTeX before LLM prompts so flashcard/study-guide/tutor outputs can return the same math grammar instead of screenshots or lossy Unicode approximations.

## Managed audio and video

Audio/video bytes use the existing SQLite asset ledger, draft/import transaction ownership, reconciliation, backup/export, and `ostudy-asset` local protocol. The asset table is already MIME-generic; image-only validation/staging assumptions must be generalized without weakening image validation.

Imported media are accepted only after bounded signature/container validation. Filenames and donor MIME declarations are advisory; they never override detected bytes. The initial compatibility set should favor formats that browser/WebView engines handle consistently:

- audio: MP3, WAV, Ogg/Opus or Ogg/Vorbis, FLAC, and compatible M4A/AAC where container detection is unambiguous;
- video: MP4/H.264-compatible containers and WebM where safely identifiable.

Unsupported codecs/containers are reported per card rather than copied as arbitrary executable content. Remote `http:`, `https:`, `file:`, `data:`, Android content URIs, and donor filesystem paths are never persisted as card media sources.

## Playback model

Use native HTML `<audio controls>` and `<video controls playsinline>` as the playback engine. Native media elements provide accessibility semantics, keyboard/OS integration, codec negotiation, fullscreen behavior, and mature buffering without introducing a player dependency.

The Tiptap nodes are thin application-owned wrappers that supply only a hydrated local asset URL and safe attributes. Imported `autoplay`, arbitrary event handlers, scripts, and styling are not retained. `preload="metadata"` is the default so opening a large deck does not eagerly decode every media asset.

The local asset protocol must support single byte-range requests and `206 Partial Content` with `Accept-Ranges: bytes`; media seeking must not require loading an entire video into Rust memory. Invalid/multiple/unsatisfiable ranges fail explicitly. Existing image requests continue to receive ordinary complete responses.

Future language/bird-sound practice features should build controls around `HTMLMediaElement`, not replace it: replay hotkeys, speed control, A/B looping, waveform visualization, microphone recording, pronunciation comparison, or auto-play-after-user-gesture can be added as study behavior while retaining the same durable media node.

## Anki conversion

Anki stores media in its collection media map. Field content commonly references sound as `[sound:filename]`; HTML templates/fields may also contain `audio`, `video`, or `source` elements. The importer resolves only filenames present in the package media map, validates the bytes, creates managed native assets, and emits `audio`/`video` nodes. It never delegates donor HTML directly to a WebView.

Anki MathJax TeX is converted to native math nodes. MathML is converted only when the bounded structural converter understands it. Existing static-JavaScript salvage remains separate: a media or math node is data, while executable JavaScript remains stripped/unsupported.

The compatibility report should distinguish unsupported codec/container, missing packaged media, unsupported/ambiguous math markup, and other active HTML. Supported audio/video/math must not be counted as generic "active content" failures.

## Shared rendering surfaces

`RichTextView` and the editor extension registry are the rendering boundary for flashcards, templates, previews, practice, and rich study-guide content. Math/media support belongs there so features do not invent private renderers. Future AI tutor/chat messages should feed trusted/parsed response content through the same math rendering primitive rather than use unsanitized `dangerouslySetInnerHTML` or provider HTML.

The current Study Guide reader still has some plain-text authoring surfaces; adding schema support does not claim every guide editing control is already a full rich editor. Rendering of stored/generated math should converge on the shared rich-content view.

## Security invariants

1. Imported code is never executed.
2. TeX/MathML are parsed as bounded data and cannot invoke a system LaTeX process.
3. KaTeX trusted HTML/network commands remain disabled.
4. Media sources are managed asset IDs only; no remote or donor-local URLs.
5. Media type is determined from bounded bytes/container structure, not extension alone.
6. The asset protocol serves only ledger-owned ready/staged assets to the main WebView and validates paths/sizes.
7. Byte ranges are bounded to one valid interval and never permit arbitrary filesystem reads.
8. Rich-document native validation remains authoritative for node/attribute structure and asset references.
9. Interchange/export/backup preserve media bytes and math source through the existing transactional graph.

## Initial PR scope

This PR implements the native schema/rendering and safe Anki conversion for math/audio/video, including asset protocol streaming/range support and interchange round-trip coverage. It also documents the MathLive authoring direction and document-ingestion normalization contract.

A complete MathLive formula-authoring dialog, microphone recording/pronunciation comparison, automatic media playback policy, broad codec transcoding, OCR of equation screenshots, and source-specific OMML/PDF equation reconstruction are independent follow-ups unless they are required to preserve an already-identified equation in a supported ingestion adapter.
