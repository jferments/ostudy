# Ordered image sources: current implementation boundary

Part of the [shared-knowledge development plan](README.md) and its [LK-04 scope](01-foundations.md#lk-04--ordered-image-sets-and-screenshot-cleaning). LK-04 remains in progress; this note describes native behavior, not an enabled or accepted Documents interface.

## Retained source and cheap extraction

`documents/image_sets/` owns admitted originals, stable member identities, ordered manifests, import receipts, and local OCR reuse. Pixel identity and the native extractor contract determine reuse; position and inclusion crops do not transfer ownership. The adapter validates retained bytes before using them. Failed OCR does not delete successful siblings or the original source. No provider call occurs in the overlap implementation.

`document_ingestion/image_set/regions.rs` groups only extractor-supplied consecutive paragraph lines. Literal text, normalized source pixels and line geometry remain distinct from screenshot order. Region identities include the stable member identity and retained extraction, not its current position. A paragraph intersected by an inclusion boundary is not falsely promoted to a complete qualified statement. Unknown geometry is not guessed.

## Native overlap proof and projection

`overlap.rs` accepts two deliberately narrow proof shapes: identical admitted image views with matching OCR, or a forward-scrolling suffix/prefix containing at least two unique paragraph anchors. Partial overlap requires exact text, identical full-width RGBA strips, matching relative line geometry, non-overlapping reading order, and a consistent positive vertical displacement. Digits, signs, units, code operators, superscripts and negation are not normalized away. Repeated headers, unlocated text, intersecting table-like geometry and contradictory pixel/text evidence preserve both candidates with an ambiguity count.

Strip hashing uses the already decoded raster. Its per-image budget is at most twice the raster area, so pathological overlapping OCR boxes cannot multiply pixel work indefinitely. Budget exhaustion disables affected equivalence proofs; it never truncates the retained OCR. This conservative matcher intentionally leaves reflowed, scaled, compressed or otherwise uncertain overlaps unmerged.

`passages.rs` creates one text projection for a verified passage and keeps every proven image occurrence in its native source locator. Each additional occurrence is compared directly with the representative's exact text/pixel signature; a chain of candidate links is not sufficient. Canonical source units and blocks still retain every admitted OCR occurrence and its geometry. Native region IDs, representative links, frame identities and available pixel signatures are retained as occurrence attributes.

Later model-visible image derivatives mask only proven duplicate strips. Exact whole-image duplicates share a visual context while preserving independently owned original members and assets. Novel pixels and uncertain overlaps remain visible. The OCR cache and original image files are never rewritten by cleaning. The adapter/render identities are advanced for the changed projection; the underlying compatible OCR extraction contract is unchanged.

## Regression boundary

`document_ingestion/image_set/overlap_tests.rs` exercises the real image admission, raster hashing, adapter, canonical persistence and compatible retry path with synthetic PNGs and injected local OCR. Fixtures cover translated overlap, duplicate text with different pixels, protected character changes, repeated educational headers, table-like reading-order ambiguity, missing geometry, original-byte preservation, masked derivative pixels, exact occurrence mapping, and bounded dense-layout comparison work. Existing image admission/cache and current-library preservation tests remain required.

Source publication, static/fast qualification, full Linux qualification, Android package/emulator checks, provider quality, and human source review remain separate evidence. Synthetic OCR does not establish the recognition accuracy of the platform OCR runtime or the faithfulness of a live interpretation provider.

## Remaining LK-04 work

The reversible Content filter interface and persisted user overrides are not implemented by this increment. Automatic role classification must retain attribution as metadata, preserve educational qualifications, and allow interface elements to be the lesson subject. Obscured text remains unreadable; no guessed source URL may be fetched.

Append, reorder, rotate and inclusion amendments still need an atomic native update boundary, affected-neighborhood invalidation, and Documents controls with explicit retry/removal of failed selections. Stable knowledge/context reuse through those amendments, filter restoration, complete-backup round trips, source-location inspection, and Linux/Android UI acceptance must be completed before LK-04 is marked finished.
