# Ordered image sources: implemented LK-04 boundary

Part of the [shared-knowledge development plan](README.md) and its [LK-04 scope](01-foundations.md#lk-04--ordered-image-sets-and-screenshot-cleaning). LK-04 implements ordered image sets as first-class managed document sources. Platform qualification and human visual/source-quality acceptance remain separate evidence from the implementation described here.

## Retained originals and ordered manifests

`documents/image_sets/` owns bounded admission, independently retained originals, stable member identities, sequence manifests, import/edit operation receipts, and local extraction reuse. The source authority is the ordered manifest plus its owned original members; oStudy never replaces it with a stitched mega-image.

Selection inspection verifies the staged-file purpose, supported image signature, admitted byte hash, decoded dimensions/pixels, per-image bounds, and aggregate member/byte/pixel budgets before publication. A failed selection remains a distinct selection that the Documents workspace can retry or remove. A successful import commits the whole bounded set with one operation identity; it does not start provider work as part of source admission.

Original image bytes are immutable managed evidence. Reordering, rotation, source inclusion changes, filtering, overlap cleaning, and model-visible derivatives never rewrite them. Controlled source inspection reads those retained originals through the native preview command rather than exposing arbitrary managed paths.

## Cheap local extraction before interpretation

`document_ingestion/image_set.rs` routes an image set through platform-local region OCR. Compatible extraction is cached by normalized pixel identity and extractor contract, separately from source ownership. Reorder and inclusion changes reuse unchanged OCR; rotation or novel appended pixels invalidate only the affected member extraction. Successful siblings remain cached when another member is unreadable or later publication fails.

The adapter groups only extractor-supplied consecutive paragraph lines. Literal OCR text, normalized pixels, geometry, source-member identity, and reading order remain separate evidence. Missing geometry is not guessed. A paragraph cut by an inclusion boundary is not promoted into a complete qualified statement.

Import, original inspection, local OCR preparation, overlap proof, filtering, and source amendment do not choose or contact an AI provider. Later flashcard, guide, Tutor, or other semantic consumers retain their existing explicit provider/permission boundaries.

## Conservative overlap proof and canonical projection

`document_ingestion/image_set/overlap.rs` accepts two deliberately narrow proof shapes: an identical admitted image view with matching OCR, or a forward-scrolling suffix/prefix containing at least two unique paragraph anchors. Partial overlap requires exact literal text, exact full-width RGBA strip evidence, compatible relative line geometry, non-overlapping reading order, and one consistent positive displacement.

Digits, signs, units, code operators, superscripts, and negation are not normalized away. Repeated headers, missing geometry, intersecting/table-like layouts, changed source pixels, protected-character changes, and other ambiguity preserve both candidates instead of choosing a majority reading. Pixel comparison uses the already decoded raster and a bounded per-image work budget; exhaustion disables an equivalence proof rather than truncating OCR.

A verified repeated passage receives one canonical learning projection while retaining all proved image occurrences in its source locator. Canonical source units and blocks still retain each admitted OCR occurrence and its geometry. Model-visible image derivatives may mask only proven duplicate strips; original image files and OCR cache entries are never cleaned in place.

## Reversible Content filtering

`documents/image_sets/filter.rs` applies deliberately specific local rules. Confident, located, short interface labels or cookie notices at image edges may be excluded automatically. Short author/date/copyright attribution is retained as metadata. Ordinary prose, educational qualifications, uncertain recognition, and unlocated text stay available rather than being discarded by a broad keyword rule.

The Documents **Content filter** view exposes the literal retained OCR region, its automatic/current role and reason, and an explicit **Automatic / Include for learning / Exclude / Metadata only** choice. **Include interface content** lets interface material remain learning content when the interface itself is the lesson. An explicit exclusion with unknown geometry withholds the model-visible image instead of leaking excluded text back through pixels. Obscured text stays unreadable; this workflow never recovers it by guessing or fetching a source URL.

Manual decisions are revision checked and retry safe. A rotation can legitimately change OCR region IDs, so an old manual choice is carried only when the same stable member has exactly one new region with the same literal text. No fuzzy text, position-only, or majority transfer is allowed. Publication atomically rebinds that decision to the new region ID.

## Selective source amendments

`documents/image_sets/amend.rs` implements append, reorder, quarter-turn rotation, and source-inclusion amendments against an expected source and filter revision. Existing member IDs and original-byte ownership remain stable. Appended staged selections are re-inspected and revalidated before the transaction consumes them.

Managed amendment files use a bounded native staging journal. Candidate originals/manifests are written under their owned document paths before the SQLite publication transaction; a failed transaction leaves the old manifest authoritative and cleanup removes only exact unowned candidate files. Startup reconciliation handles an interrupted staging marker without inventing ownership or deleting altered files.

An acknowledged amendment invalidates the current projection but preserves its previous processing receipt as an inspectable source snapshot until local preparation publishes the successor. Stale extraction cannot publish against the amended manifest. Unchanged compatible OCR and exact native knowledge identities survive reordering; changed adjacency, rotation, inclusion, filters, or appended members rebuild only the affected projection/evidence boundary. Durable mutation acknowledgement remains successful even when a subsequent UI refresh or local preparation reports a separate failure.

## Documents workspace

Documents now exposes **Add image set** and **Edit images** for image-bundle sources. The feature-local controller uses the typed IPC gateways; shared UI does not call Tauri directly.

Before initial import the workspace supports:
- repeated image selection with retained failed rows;
- explicit **Retry inspection** and **Remove**;
- reading-order movement;
- quarter-turn rotation;
- normalized source-inclusion bounds; and
- one source name and one retry-stable import identity.

For an existing image set it provides:
- controlled original-source inspection;
- reorder/rotation/inclusion edits and appended images;
- a separate **Save source changes** mutation followed by local preparation;
- **Retry local OCR**;
- the reversible Content filter described above; and
- clear prepared versus pending source state.

A pending append is committed at the end of the current sequence. Once it becomes an owned member it participates in the same reorder controls as every other member. This avoids presenting a UI ordering operation that the native amendment contract did not actually authorize.

## Backup, deletion, and source-location ownership

Whole-library backup already packages the current image-set manifest plus independently retained original members. Restore validates image signatures, byte hashes, decoded pixel fingerprints, managed paths, and the SQLite ownership graph before adoption. LK-04 regression coverage additionally round-trips a pending rotated amendment with the previous source snapshot and manual Content-filter decision intact.

Permanent document deletion continues through the existing document-owned lifecycle, so image manifests, original members, derivatives, receipts, filters, knowledge/source links, and retained processing state follow the same source-deletion transaction and managed-file reconciliation as other documents. Generated surviving study artifacts keep their existing detached citation semantics.

Canonical image chunks retain native source locators with every verified occurrence. Original inspection is intentionally a controlled source view rather than an external guessed deep-link mechanism.

## Regression boundary

Deterministic native/frontend fixtures cover:
- ordered admission, stable identities, aggregate limits, retry/removal semantics, and original-byte preservation;
- compatible OCR reuse and partial extraction failure;
- translated overlap, protected-number/unit/operator changes, repeated educational headers, table-like ambiguity, missing geometry, conflicting pixels, and bounded dense-layout comparison;
- filtered derivative pixels, unknown-geometry withholding, UI-course policy, attribution, restoration, stale/forged filter publication, and exact filter rebinding after rotation;
- reorder, rotation, inclusion, append, stale extraction rejection, failed SQL publication, retry idempotency, interrupted amendment cleanup, and unchanged knowledge reuse;
- complete-backup validation/extraction for current originals/manifests plus pending amendment/filter state; and
- Documents workspace flows for failed-selection retry, ordered import, original inspection, source edits, and Content-filter publication.

Synthetic OCR/provider seams establish deterministic contracts; they do not establish live OCR quality, live-model factual quality, physical-device behavior, or human visual/accessibility acceptance.

## Qualification and acceptance boundaries

Source publication, static qualification, fast qualification, full Linux qualification, Android compile/package, Android emulator smoke, live-provider testing, physical-device testing, and human visual/source-quality review are recorded separately. LK-04 implementation completion does not turn an unrun human/platform gate into a pass.

Before PR #103 can be considered finally ready, the frozen LK-04 candidate still requires the applicable exact-head terminal repository/platform qualification and the broader later-stage acceptance defined by the PR roadmap. LK-05 is the next implementation stage; no LK-05 folders/ranges behavior is implied by this image-source boundary.
