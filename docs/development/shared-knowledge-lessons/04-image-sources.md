# Image collections: automatic ingestion and learning materials

Part of the [shared-knowledge development plan](README.md) and [LK-04](01-foundations.md#lk-04--ordered-image-sets-and-screenshot-cleaning). The revised product boundary replaces the original manual image-preparation workspace. Source ordering remains a storage/provenance detail, not a task for the learner.

## User workflow and ownership

In Documents, **Upload images** opens a collection page. **Select images** accepts several PNG/JPEG/WebP images in one chooser. The **+** action adds more; the drop area sits beneath the file list, and clipboard image paste uses the same intake path. Selecting, dropping or pasting images starts processing without a separate import, preparation, filter or generation action. The screen contains no ordering controls, rotation buttons, source-coordinate fields or OCR-region classification forms.

`ImageSetWorkspace.tsx` renders the collection and optional original inspection. `useImageCollectionJobs.ts`, composed into the application-lifetime `useDocumentJobsController`, owns upload batches and the automatic learning workflow. Leaving the collection page does not cancel it. File selection, drop and paste enter `platform/imageInput.ts` and the typed `ipc/imageCollectionFiles.ts` gateway; `external_file_images.rs` validates bytes before admitting them to the native opaque transfer ledger. Pasted URLs/HTML are not fetched. Native drag interception is disabled for the main WebView so file drops reach the HTML drop area.

Native admission verifies the supported signature, decoded dimensions/pixels, byte hash, per-image bounds and aggregate member/byte/pixel budgets. Each file keeps a retry-stable identity. A failed file is visible and removable; successfully staged siblings are retained. The collection import/amendment request also retains its identity across an interrupted acknowledgement rather than publishing the same files twice.

## Automatic learning pipeline

After source admission, the controller runs local image preparation internally, then invokes the existing format-neutral document flashcard generation command and study-guide generation. This is the same native knowledge, relevance, assessment, curation and authoring infrastructure used by other documents, not a separate image-only card generator. The prior workspace's missing import-to-generation handoff is removed.

Further selections enter the same collection. While a batch is processing, later files remain queued; after the current task finishes, the source is amended and re-prepared. Existing generated decks receive additional cards rather than replacement decks. The guide is generated after the queued source/card work is complete. Saved sources and cards survive later provider or guide failures; retry resumes the unfinished stage without reimporting an acknowledged batch. A failed library refresh is reported separately from a successful write. Stop prevents the automatically chained next stage as well as requesting cancellation of the active task.

The user's configured document provider and permissions remain authoritative. Intake, original inspection, local OCR and overlap proof do not contact a provider. Interpretation, relevance selection, flashcards and guides use the existing explicit model boundary. Unreadable/ambiguous evidence is not invented to produce a card quota. No source URL is guessed or fetched.

## Retained originals and reusable extraction

`documents/image_sets/` owns independently retained originals, stable member identities, ordered manifests, revision checks and operation receipts. The manifest plus original members is the source authority; a stitched mega-image is not. Processing never replaces original bytes with a crop or cleaned derivative.

Compatible local OCR is cached by normalized-pixel/extractor identity, independently from source ownership. Successful siblings remain reusable after another member or publication fails. Literal text, source-member identity, geometry and pixel evidence remain distinct. Missing geometry is not reconstructed from flattened text. The collection adapter bounds retained OCR/proof metadata and decodes one source raster at a time rather than retaining all decoded images in memory.

## Upload-order-independent overlap

`document_ingestion/image_set/collection.rs` compares every bounded pair in both scrolling directions before any learning projection is published. It combines verified region correspondences so reversed uploads, nonadjacent duplicates and a connecting screenshot arriving last do not require manual reordering.

The pair proof remains deliberately conservative: identical admitted image views with matching OCR, or a suffix/prefix with multiple unique paragraph anchors, exact literal text, exact full-width pixel strips, compatible geometry, reading order and consistent displacement. Digits, signs, units, code operators, superscripts and negation are not normalized away. Repeated educational headers, table-like layouts, conflicting pixels and other ambiguity preserve candidates instead of choosing a majority reading.

Every equivalence class is checked directly against its representative's literal text and pixel proof; a chain alone cannot authorize a merge. One canonical learning passage retains every proved source occurrence. Original manifest ordinals continue to identify citations; they are not asserted to be the educational order. Unrelated images are not forced into a fabricated reading sequence.

Only proved duplicate strips can be masked in model-visible derivatives. Visual projections share content only when the actual derivative bytes match; an earlier masked derivative is not interchangeable with an unmasked original. The adapter/render identity advances for this projection change while compatible OCR extraction remains reusable.

## Automatic filtering and durable compatibility

Specific local rules can separate confident interface labels/notices from learning evidence and retain short attribution as metadata. Uncertain or unlocated educational text remains available. The shared knowledge/relevance stages decide what supports learning material; the learner is not required to label OCR regions or draw source crops.

Native source-amendment, inclusion and filter representations remain supported for previously saved sources and backups. Removing their manual UI does not erase existing decisions, rewrite original pixels, or retire durable readers. Revision-checked source amendments retain their staging journal, pending-source snapshot and stale-publication checks. Exact same-member/literal filter rebinding, unknown-geometry visual withholding and backup preservation regressions remain in force.

## Backup, deletion and inspection

Whole-library backup retains the current manifest, each owned original, processing receipts and any pending source snapshot/filter state. Restore validates signatures, hashes, pixel fingerprints, paths and database ownership before adoption. Permanent document deletion uses the existing owned-source lifecycle; surviving generated cards keep detached citation semantics.

**View collection** opens an existing source. Individual originals can be inspected through the controlled native preview command without supplying a filesystem path. Inspection is optional and never blocks automatic generation.

## Regression and qualification boundary

The workspace integration tests exercise multi-file selection, the additional chooser action, drop and clipboard intake, automatic card/guide calls, additions during generation, navigation survival, retry identity, failed-file removal, optional inspection and post-commit refresh failure. The native collection fixtures exercise all six permutations of three overlapping screenshots, late connecting evidence, nonadjacent identical images, OCR reuse, original ownership and rejection of changed literal/pixel proofs.

Existing native admission, protected-character/geometry ambiguity, filter, amendment, knowledge-rebinding, migration and complete-backup fixtures remain required. Synthetic OCR and mocked provider calls establish deterministic contracts, not live OCR accuracy or generated factual quality. Exact-SHA static/fast, full Linux, Android package/emulator, live-provider and human visual/physical acceptance are recorded separately in PR evidence.
