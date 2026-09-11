# Knowledge representation and evidence ownership

This is the maintained contract for `knowledge/` and its document, Tutor and artifact consumers. [AI and document processing](ai-and-document-processing.md) describes the surrounding pipeline; [the Tutor acceptance contract](../development/semantic-ai-tutor-implementation.md) defines qualification. GitHub records the tested source revision separately.

## Separate evidence, interpretation and presentation

Canonical documents, user-authored cards, imported content and effective guide sections remain the material the learner owns. An AI interpretation never replaces those records. A conversation can be retrieved only under its explicit privacy rules and is never promoted to independent document evidence.

`SourceRecord` is a bounded, revision-aware projection over that material. Its identity identifies the material/segment; its content hash binds the evidence revision. New document projections use identity version 2, which excludes display titles from the evidence hash. Legacy serialized records default to version 1 and remain readable. Changes to source content or assets invalidate dependent interpretations, while changing a title alone does not make v2 canonical text different.

`EvidenceReference` belongs to native code. A text selector contains half-open UTF-8 byte offsets into the exact hashed source text. Its excerpt is obtained by slicing that source, not by asking a model to quote it. The native span builder preserves text bytes and chooses bounded locations; it does not claim to solve linguistic sentence segmentation. Several adjacent or disjoint spans can support one interpretation. Image evidence identifies an admitted managed asset, and only images actually attached to the analysis request are available for selection.

`KnowledgeUnit` is a fallible interpretation: a definition, relationship, comparison, process or fact-shaped statement, with qualifications, topic and advisory importance. Its grounding contains one to eight native references. References may span multiple sources of the same document. A unit cannot mix a user assertion, generated derivative or unrelated document into an apparent single original-source authority. The legacy primary-source/excerpt fields are compatibility projections; the complete evidence list governs v2 dependencies.

Artifact links describe what a card assesses or a guide explains. These are explicit native-validated relationships, not citations inferred from text similarity. A vector index supports retrieval but owns neither source identity nor truth. Document outlines and global core-ID selections are derived learning views, not new sources.

## Provider contract and factual limits

A request presents an evidence catalog with short local aliases such as `E0`. The provider returns statements and selected aliases. It does not choose authoritative hashes, byte offsets, origins or quotation text. Native code resolves IDs, enforces source membership, checks revisions, rejects duplicates and retains all dependencies.

This proves where the selected evidence is, not whether the model's statement follows from it. Selecting a real but irrelevant passage remains a semantic quality error. Structured output cannot establish truth, exhaustive extraction, useful question direction or faithful preservation of every qualification. Prompts require preservation of conditions and contradictions; tests explicitly distinguish location validation from entailment. No fuzzy matcher, hidden confidence score or automatic factual-verification label overrides that distinction.

Manual material keeps user-authored origin. Repeated AI-generated cards or guides do not count as independent corroboration. Retrieval may expose the original evidence alongside a derivative; it must not silently treat the derivative as a stronger authority.

## Generated-output protocol boundary

The application schema is sent both in model instructions and in the provider's structured-output mechanism. Provider schema lowering visits schema nodes, not arbitrary JSON objects: property names and literal enum/default objects are data. Portable wire schemas can omit unsupported bounds; the full schema remains visible in instructions and native validators enforce those bounds. A response is a proposed JSON instance, never an authority-bearing IPC or storage record.

Knowledge extraction requests exactly `units` and `noLearningSourceIds`. It admits independently valid proposals from the named `units` array; a bare proposal array is the one documented alternate envelope. Missing, null or unusable ancillary dispositions mean no usable dispositions, not loss of otherwise valid units and not permission to mark an uncovered source complete. Unknown envelope properties are diagnosed and ignored, not promoted into evidence. Arbitrary `data`, `sources` or `results` wrappers and invented field aliases are not guessed. Missing or incorrectly typed primary content still requires repair. Per-unit evidence, source authority, cardinality and revision rules remain unchanged.

Targeted repairs similarly isolate their explicitly named collections. An unusable ancillary collection cannot erase valid independent corrections. A schema-shaped diagnostic is advisory at transport: whole-response validation there would recreate the all-or-nothing failure boundary. Domain decoders decide admissibility. Native IPC, durable receipts and source contracts continue using their strict typed validation.

Shared transport rejects refusal/filter responses and output-length termination even when the returned fragment happens to parse as JSON. It reserves context for the schema-bearing prompt and completion before a request. Invalid or excess admitted images are errors rather than silently omitted evidence. No provider fallback disables evidence validation.

## One reusable semantic analysis

Document adapters preserve canonical chunks and their locators. Bounded analysis neighborhoods can join neighboring source records from the same document rather than interpreting each physical page independently. Hydration batches carry unfinished neighborhoods forward so a database batch boundary does not become a conceptual boundary. Request budgets still impose finite context: the system does not claim unlimited cross-document or long-distance synthesis.

Shared knowledge is prepared before constructing the outline or authoring cards and guides. The old raw-source outline traversal is not run in parallel with a second knowledge traversal. Native grouping creates a bounded topic/source-range view. An optional balanced knowledge-only reduction supplies broader context and a small core-unit set; if every nonempty input window cannot fit, it falls back locally instead of silently dropping the tail. Importance is advisory. No automatic semantic merge destroys conflicting claims, and exclusion from a sampled global set does not mean a unit is unimportant.

Artifact prompts receive selected knowledge, a deduplicated catalog of its exact evidence, compact global context and applicable visual inputs. They do not repeat the complete raw source window alongside copies of its excerpts. Cards select their actual unit targets; guides retain support for their sections. Existing transactional artifact publication, cancellation and scheduling boundaries remain in force.

Study-guide requests use short `K0`-style aliases mapped back to the exact supplied native units. Alias resolution cannot introduce new targets or hide duplicates. Missing/null optional outline categories can be represented as empty arrays; mandatory content, key points and evidence selections cannot be invented. Visual explanations require attached images. The compact learning plan remains valid JSON: the active topic is retained first, whole background topics are omitted with an omission count, and no factual sentence or serialized object is cut mid-value.

Guide contract/truncation failures receive one bounded repair with safe field feedback and, when bounded, the invalid draft itself as untrusted data. The draft is not logged or accepted as evidence. The repair has additional output headroom but retains content/concision limits. Every successful member of a completed parallel topic batch is checkpointed before a sibling failure is propagated. Checkpoint format v3 invalidates older derived section candidates without deleting published guides, annotations or shared semantic analysis.

## Failure boundaries and resumability

A malformed transport response is a request failure. A malformed unit or nonexistent evidence ID is a proposal failure. A changed canonical source or competing interpretation is a stale-work failure. These do not have the same recovery semantics.

A bounded analysis report stores admitted units, rejected proposal slots and source dispositions. Admitted units are published before a targeted repair request. A cancellation, provider outage or bad repair cannot erase those siblings. The provider repairs rejected slots and uncovered sources only; it is not asked to regenerate all successful output. Invalid slots remain unresolved. Duplicate/conflicting slots do not choose a winner by order, but independent successful repairs are retained.

An explicit no-learning disposition distinguishes publishing scaffolding or other non-learning content from a missing response. A null repair is still unresolved. A partial receipt is not a completed extraction, even when it provides useful units. Documents expose unresolved counts and Tutor preparation exposes partial status. Accepted source material can remain usable while those omissions are visible. “Complete” means every source has a protocol disposition; it does not certify exhaustive recall of every concept in a textbook.

Completed checkpoint reuse is read-only. It does not rewrite units, reset artifact links or invalidate a cached overview. A checkpoint binds the exact source revisions plus the current interpretation and receipt revisions before provider work. Its save checks that snapshot in the same short SQLite transaction as publication. A concurrent writer cannot silently overwrite newer admitted work. If source packing or extraction contracts change, obsolete overlapping receipts are retired and untouched siblings are marked for reanalysis rather than remaining falsely complete.

Retries remain explicit user actions. Startup reconciles interrupted tasks but sends no provider request. A retry of a partial matching window targets its unresolved work; a retry of a complete matching window makes no provider request. Subset preparation cannot silently expand consent to a larger source neighborhood.

## Revision safety beyond nonempty units

A unit depends on every selected evidence source, not merely its primary compatibility source. Native dependency rows and invalidation triggers remove stale interpretations and queue index eviction when any source changes. Artifact publication verifies support inside its existing transaction; edited material never gains invented citations.

Whole-document views need a stronger boundary than checking their nonempty catalogs. A source with no units, a changed no-learning disposition or an added canonical chunk can change the meaning of an overview. `DocumentSnapshot::prepared` verifies the original canonical plan and every expected prepared source projection, including empty results. An ordered, length-framed digest binds canonical content, assets, source revisions, units and receipts. Display names and progress timestamps are excluded.

The snapshot is rechecked around provider reduction and inside the final publication transaction. The derived outline, metadata and ready-state update publish atomically. A changed, added, removed or reinterpreted source prevents that stale result from committing. Native fallback summaries use whole labels, not clipped factual sentences that could lose negations or conditions.

Tutor context applies scope/privacy filters before retrieval and checks current references again. A multi-source unit is usable only when all its supporting sources survive scope filtering and context-budget eviction. This is an all-dependencies rule, not permission to retrieve private material merely because it is linked.

## Persistence, migration and privacy

Migration 14 adds evidence dependencies, analysis receipts and document views. It does not rewrite existing migrations or delete canonical documents, ordinary notes, guide edits, conversations, review events or schedules. Legacy provenance remains readable; new extraction no longer asks for provider-written quotations. An older binary cannot reverse an applied migration; preserve a compatible library backup before testing migration-bearing builds.

Whole-library backups preserve authoritative source/relationship and analysis state under the normal maintenance admission rules. Semantic vector files remain derived and rebuildable. Device-transfer sanitization strips private/derived semantic projections and rejected drafts; it does not treat full backup and device sync as interchangeable contracts. Source deletion cascades its owned dependencies while unrelated learner content retains the existing preservation rules.

Rejected drafts are bounded private application data, not diagnostic output. Logs record source IDs/locations, counts, native rejection categories, fingerprints, phases and provider usage; they exclude document text, generated drafts, prompts, credentials and model reasoning. A content hash is a diagnostic identity, not encryption or a promise of anonymity.

`structured_output_shape_mismatch` records application-schema paths, expected/actual types, missing required fields and counts of unexpected fields. It never copies unknown provider field names, values or raw Serde error strings. Inspection has depth/work/result bounds. `knowledge_envelope_repair_started` and `study_guide_repair_started` identify the bounded repair; `study_guide_topic_failed` records the topic index and source range without the topic text. A shape warning may describe safely recovered ancillary output, so inspect the subsequent admission/completion event rather than assuming every warning is fatal.

## Budgets and verification

Planning includes the semantic-analysis traversal, allowed repair work, optional global reduction and artifact generation. Source-token estimates, provider-reported usage and billing are distinct quantities. No performance or cost-reduction percentage is claimed without measurement on representative documents. Source, request, proposal, receipt and snapshot bounds prevent unbounded work, but a complete large document may still require many provider requests.

Native regression tests cover exact Unicode/PDF spans, unknown and duplicate aliases, multi-page references, sibling survival, partial/null repair, malformed repair entries, source edits during provider work, empty-result invalidation, canonical-plan completeness, competing checkpoint writers and obsolete receipt retirement. Provider-lowering-to-decoder-to-SQLite fixtures cover the shared response contract for OpenRouter, DeepSeek and llama.cpp; guide tests cover optional categories, short aliases, context serialization, output truncation and preservation of successful parallel siblings. Existing Tutor, backup, privacy, migration and platform suites remain required. Deterministic fixtures do not prove live OpenRouter model quality, provider retention behavior or physical Android lifecycle behavior.

For live diagnosis, build the verified PR head using `./scripts/ostudy-dev build`, then run `./scripts/ostudy-dev run --log-filter='warn,ostudy.app=info,ostudy.ai=trace,ostudy.documents=trace'`. Preserve terminal output and the launcher/application diagnostic files immediately after failure. Keep the same provider, model and reasoning setting for the first regression test and retry retained material rather than deleting the library. See [the command runbook](../agents/COMMAND-RUNBOOK.md) for maintained build and platform commands.
