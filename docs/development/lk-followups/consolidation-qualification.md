# Final data consolidation, cleanup and whole-product qualification

## Status and authority

Planning-only successor to the PR #103 split. Intended delivery slot: **PR #106**; GitHub has not allocated that number to this plan. Proposed implementation branch: `agent/lk-consolidation-qualification`. Prepared 19 September 2026. Publication adds only this new Markdown file from verified `main` commit `30ae169fbe1b4c70add0ed0f6db2b847060c70a0`.

No product source, migration, workflow, personal library, PR #103 metadata or active run is changed by this plan. This is not an LK-04 test build. Product implementation and human data adoption require their later assignments; there is no merge authorization.

The binding design provenance is the original [master roadmap](https://github.com/jferments/ostudy/blob/a29070fa3bf031a4dc172d945d72bbccf22e3171/docs/development/shared-knowledge-lessons/README.md), [LK-13/LK-14 stages](https://github.com/jferments/ostudy/blob/a29070fa3bf031a4dc172d945d72bbccf22e3171/docs/development/shared-knowledge-lessons/02-learning-experience.md), and [data/acceptance contract](https://github.com/jferments/ostudy/blob/a29070fa3bf031a4dc172d945d72bbccf22e3171/docs/development/shared-knowledge-lessons/03-data-and-validation.md). The observed PR #103 head is provenance, not proof of its stage completion. Current repository instructions supersede historical execution arrangements.

## 1. Scope, order and the LK-13 correction

The delivery sequence is #103 through LK-04, #104 for LK-05–09, #105 for LK-10–12 plus feature data readiness, then this final cleanup. Each predecessor must be independently working, qualified, human-reviewed and merged. Final qualification is an additional integrated gate, not a substitute for predecessor correctness.

Original LK-13 already owns final schema baseline, actual-library adoption and old-reader archival. Therefore this cleanup is **LK-14 plus transferred LK-13 consolidation responsibilities**, not literally only the old LK-14 text. Preserve the original identifiers:

* LK-13-P stays with the feature PR: additive migrations, feature readers, backup/transfer support, fixtures and final-contract inventory.
* LK-13-C belongs here: curated baseline, preservation-proven conversion, local actual-library evidence and retirement of superseded runtime machinery.
* LK-14 belongs here in full: code/docs/workflow audit, immutable-candidate qualification, combined scenario and handoff.

Original AC-14 closes only after LK-13-C, including its human evidence; original AC-15 owns final qualification. Do not silently mark LK-13 complete when only preparation exists.

This is not a blanket rewrite or deletion project. Preserve the app's current behavior and learner data. Fix concrete workflow defects supported by evidence from #103–#105; do not introduce a new orchestrator, source publisher, hosted service or broad dependency upgrade.

## 2. Activation and bounded inventory

After #105 merges, record exact merged commits for #103–#105 and the actual `main` head. Integrate that head into this existing planning branch using a reviewed normal non-force merge. Do not replace its tree with an older feature tree, force-push, or rely on an obsolete stacked branch after squash merges. Retarget any dormant planning PR to `main` only when implementation is explicitly activated and shared runner use is coordinated.

Read predecessor handoff inventories and immediate owning code/tests, then create one current contract map. Identify the actual final schema/lineage/payload versions from source and installed-input inspection where authorized; never reserve or guess a final version number in advance. If a late feature changes durable contracts, update the inventory and affected tests before freezing consolidation.

| Area | Owners to inspect/change | Required result |
| --- | --- | --- |
| Entry/schema | `src-tauri/src/database.rs`, `database/`, `data_repairs.rs`, `startup.rs`, `src-tauri/migrations/` | Explicit current baseline and known-input normalization; no scattered legacy feature readers |
| Backup/activation | `library_backup/`, managed assets, external-file/staging owners | Coherent backup, staged conversion, crash-safe activation and tested recovery |
| Feature payloads | Cards/questions/attempts, evidence/knowledge, collections, guides, Tutor, lessons/speech | Current validators, stable identities and explicit conversion mapping |
| Interchange | `.studydeck`, authoring JSON, Anki/APKG, `device_sync/`, backup readers | Separate supported contracts; no universal format or lossy substitute |
| Cleanup | Feature controllers, IPC, domain services, styles, tests and package references | Remove only proven dead owners, duplicate implementations and obsolete runtime adapters |
| Documentation | `docs/README.md`, user/development/architecture/reference and agent guides | Current claims, correct commands, clear history and cross-PR traceability |
| Workflow | Existing scripts and `.github/workflows/` only for demonstrated defects | Bounded recovery, honest qualification, noninterference and self-hosted read-only execution |

Paths name inspected design owners; additions such as `scripts/convert-library-current.sh`, a baseline fixture or historical archive are proposed until their necessity and existing equivalents are checked. No speculative command is presented as usable now.

## 3. Preservation contract

Preserve semantic values and identities, not just row counts. Transforming an explicitly versioned representation may change serialized bytes; define a canonical before/after semantic projection for those fields and require byte/hash identity for fields that are not transformed. Document every allowed difference, including newly derived indexes, instead of accepting unexplained drift.

| Retained graph | Invariants |
| --- | --- |
| Cards and learning | Note/card IDs, Basic/reverse/Cloze/objective definitions, variants, source/key relationships, tags/templates, rich content/assets, schedules/due values, reviews, attempts, assistance and overrides |
| Authoring | Unsaved durable drafts, learner edits, validation conflicts, guide overlays, annotations and explicit retained revisions |
| Documents/knowledge | Original files, evidence payloads and owned occurrences, image ordering/coordinates, reversible filter decisions, support links and honest partial/empty/unresolved receipts |
| Collections/bundles | Folder/range membership, snapshots, synthesis links, target/deck identities, successful jobs and deletion suppression |
| Tutor | Conversation/project/turn identity, text and historical source/citation snapshots, archive/privacy/default selections |
| Lessons | Plans, scene/beat/script/question identities, progress/response state, retained revisions and disclosure choices |
| Narration | Paid audio bytes, manifests, actual producer/configuration metadata, receipts, hash/duration integrity and explicitly retained prior renditions |

Ownership hashes are not permission. A matching blob must not merge separate document ownership or widen allowed sources. Derived search/vector indexes may rebuild only under an explicit policy; migration performs no extraction, LLM, speech or other provider calls. Core data is not an expendable cache.

## 4. Implementation roadmap

### 106-A — Consolidation specification and preservation fixtures

Create a bounded inventory of current tables, columns, defaults, constraints, indexes, triggers, FTS/tokenizer configuration, repair ledgers, payload readers and managed-file kinds. Classify each compatibility path as live required, ingress normalization, historical recovery, or proven dead. Record why it exists and its last caller/fixture.

Extend the synthetic full-library fixture from LK-02/LK-03 with final #104/#105 graphs. Include deliberately edited/suppressed content, all question kinds, in-progress and checked attempts, unsupported/corrupt siblings, filtered screenshot regions, private Tutor history, partially generated bundles, lesson revision changes and paid prior-voice clips. Make the fixture deterministic and content-free of personal data.

Define semantic preservation receipts and an explicit transformation allowlist. Test existing current-format reopen, complete backup, restore rollback, committed restore and post-restore decoding before changing baseline machinery. A preexisting failure gets its own concrete diagnosis; never weaken preservation assertions to clear it.

Exit: every retained category has a decoder/value/file invariant and all candidate legacy paths have an owner and classification.

### 106-B — Curated fresh baseline and equivalence proof (LK-13.1)

Construct a current-schema baseline from the final contracts, with explicit lineage/baseline identity where needed. Do not concatenate historical SQL, edit released migration bytes/checksums, relabel an older database, or assume matching `user_version` proves equivalence.

Build two isolated databases: one through each supported known migration/conversion route, and one from the curated baseline. Compare normalized schema structure including column affinities/defaults/nullability, keys, constraints, indexes/collations, triggers and FTS definitions. Also run behavioral assertions: uniqueness, foreign keys, delete/update actions, search/tokenization, transaction semantics and current decoders. SQL string equality alone is insufficient.

Keep current startup/recovery paths live while this candidate is under test. Establish new-baseline admission separately from the old known-input converter. Ordinary feature services consume one current representation; they do not gain dual writes or legacy modes.

Exit: schema structure and behavior agree; current-to-current admission is a verified no-op; unknown/future/tampered/corrupt input fails closed.

### 106-C — Staged converter, activation and recovery (LK-13.2)

Implement or extend one maintained local conversion wrapper and its native service. It resolves storage through existing application paths and requires the appropriate exclusive maintenance boundary. It records exact input lineage/ledger/repair state and converter SHA without exporting private contents.

The procedure is: admit input read-only; obtain and verify a coherent complete backup; prove sufficient space for original/backup/staging/output; restore an isolated copy; capture preservation receipts; run known forward conversion in staging; verify target schema, semantic receipts and managed-file references; activate through existing recoverable swap/startup discipline; reopen and verify; retain the prior coherent library for recovery.

A raw copy of a live SQLite file is not the backup procedure. Preserve original bytes and sidecar behavior on rejected input. Inject faults before/after journal/transaction/file-move/activation boundaries, including process interruption, disk-full, unreadable asset, corrupt audio, duplicate invocation and successful mutation followed by refresh failure.

An older executable cannot undo a newer schema. Recovery uses a compatible binary and the verified matching backup through the established restore path. No reset, uninstall, clearing app data, lossy deck export/import or guessed downgrade.

Exit: deterministic synthetic conversions and recovery pass; original remains recoverable after every injected interruption; no provider calls or partial activation are possible.

### 106-D — Authorized actual-library acceptance (LK-13.3)

Provide precise local commands and safeguards only after the converter exists and is qualified. Personal data stays on the user's machine. Implementation access to GitHub does not authorize inspecting or migrating their installed library.

The human gate records: converter/build SHA; input application identity and schema/lineage/ledger summary; verified complete-backup and asset-manifest identities; canonical preservation digest and allowed transformations; output identity; successful reopen; representative behavioral checks; creation and isolated restoration of a new current backup. Record bounded summaries, not document text, credentials, raw answers or database uploads.

Representative checks include rich cards/media and resumed Practice, schedules/analytics, edited guide, source/filter inspection, folder-scoped Tutor, lesson resume and retained offline narration. Compare actual values and hashes as well as counts. Synthetic tests cannot claim this gate passed.

Until explicit local evidence exists, mark retirement **human required** and retain its necessary converter. Other safe cleanup can continue, but this PR cannot claim full LK-13 completion or delete the only historical recovery route. An unavailable human gate is reported, not disguised as an implementation failure or silently waived.

Exit: the user has reviewed a privacy-safe preservation/recovery receipt for each known retained development library within scope, or the gate remains explicitly blocked with working conversion paths retained.

### 106-E — Archive superseded runtime machinery (LK-13.4–13.5)

Only after the preceding gate, archive superseded SQL, converters and fixtures with provenance, original source SHA, immutable hashes and recovery instructions. Keep a deliberate offline historical-backup conversion route. Preserve backup files themselves. Archival is not destruction.

Remove obsolete runtime consumers, version branches and dual representations in a coordinated change. Trace normal startup, compilation, packaging, resources, tests and indirect references to prove archived code is not still loaded. Keep a separately executable recovery-validation suite even when archived fixtures leave routine startup/package paths. Future schema changes still require small forward transitions from the new baseline.

Repeat complete-library backup/restore across the final graph, including private local records and paid audio. Define content-only transfer and trusted-device synchronization separately. Content-only import has fresh schedules and no donor reviews/private Tutor history. A receiver either negotiates supported lesson/media versions explicitly or rejects them before any partial transfer.

Exit: current runtime has one representation, fresh and converted libraries agree, historical backups have a tested explicit recovery route, and archival has not weakened data-preservation coverage.

### 106-F — Evidence-based code and documentation cleanup (LK-14.1)

Use a deletion ledger: candidate owner; production and indirect callers; serialized entry points; platform fallback need; replacement; exclusive styles/dependencies/tests; regression demonstrating behavior preservation. Search dynamic registries, IPC registration and saved-route/checkpoint readers before declaring something unused.

Remove demonstrated duplicate legacy adapters, obsolete layouts and unreferenced styles/dependencies together with their exclusive dead scaffolding. Preserve supported imports, meaningful fixtures, platform fallbacks, optional capabilities and regression tests. Do not collapse Basic/Cloze/objective question kinds or user/generated content origins as though they were old schema versions. No broad formatting or dependency churn.

Source-check user guides, architecture ownership, AI/provider capability claims, current-format references, build/install/debug commands, backup/restore warnings and stage status. Resolve contradictory prose in canonical sections; archive historical decisions with clear labels. The product README remains a short introduction, not a commit diary. Update documentation indexes and links together.

Exit: each removal has ownership evidence, maintained docs describe actual source, and product behavior/preservation remains covered by focused regressions.

### 106-G — Workflow repair without disrupting active work

Collect concrete failures from #103–#105: stale head/body evidence, repeated broad rereads, empty test selections, unsafe full-file publication, unnecessary qualification invalidation, duplicate retries, shared-runner conflicts or missing handoff evidence. Separate product, test, environment, connector, human-only and ChatGPT-response failures. Repository changes cannot certify a platform-level interruption is fixed.

For each proposed workflow change, record observed failure/run/SHA, smallest owning component, regression test and expected behavior. Preserve GitHub-direct source publication, one writer per branch, verified blob/parent/tree/ref identity, no force pushes, and self-hosted source-read-only Actions. PR metadata checks remain distinct from product gates. No no-op commits, state toggles, sleeps, weakened assertions or infrastructure redesign to obtain green.

Test branch/PR-specific concurrency behavior so a separate planning or feature PR cannot cancel another branch's qualification. Audit docs-only scheduling costs separately from expensive-gate reuse: a small diff does not itself stop a workflow from scheduling. Any change to triggers, permissions, concurrency groups, caches, services or shared execution must be reviewed for active-run impact first. If it could disrupt another writer/run, ask the user before acting. Service administration is not authorized by this plan.

Improve the compact checkpoint and final handoff where evidence warrants: exact remote head, active stage, expected parent/tree, files, failed command, tested SHA/run IDs, next action and human blockers. Track observed recovery rereads, repeated unchanged failures, unnecessary full runs and time to a published useful increment only when logs support them; do not invent before/after speedups or collect personal telemetry.

Exit: concrete regressions cover agreed repairs; current active runs were not canceled or commandeered; no claim is made about OpenAI's internal scheduling or motives.

### 106-H — Freeze, qualify, exercise and hand off (LK-14.2–14.5)

Complete broad source/diff/docs/compatibility/cleanup audit and prepare the handoff before freezing the exact candidate. Run maintained static/fast, full Linux, Android compile/package and emulator gates. Match named step results to the actual commit. Explain allowed fingerprint-based reuse with the original producing SHA; an ancestor's green result alone is not current qualification. A true blocking change gets a new candidate and relevant gates; optional late cleanup goes to follow-up rather than continually invalidating tests.

Exercise the combined Week 1 scenario: import readings, sparse slides, notes/assignment and overlapping screenshots; inspect/restore filters; combine a collection; generate a coherent bundle; use folder/project Tutor; inspect guide/PDF; read and resume responsive lessons/checks; synthesize the selected verified voice once; replay offline; change desired voice without immediate spending; deliberately generate replacements and switch back; export PPTX notes/saved audio; complete backup/restore; reuse an already prepared source in Week 2.

Measure fake/authorized-live request counts at each boundary. The scenario must preserve IDs, edits, schedules, responses, scope exclusions, suppression and retained files. Use representative held-out source review for educational completeness/faithfulness, not just deterministic contract assertions.

Record actual device/application/OS/version for visual/accessibility checks, physical Android audio/Back/Bluetooth, PDF layout/reading order and PowerPoint/Impress compatibility. Missing environments stay unrun. A ZIP parser, emulator pass or provider mock cannot certify these human observations.

Exit: immutable-candidate evidence, explicit human gates and known limitations, exact local build/run/debug/restore instructions, compatibility/recovery notes and human review. Keep the PR draft while mandatory acceptance is unresolved. No automatic merge.

## 5. Acceptance and test plan

All implementation criteria start **not started**; writing this roadmap does not satisfy product acceptance.

| Criterion | Required evidence |
| --- | --- |
| AC-01 / original AC-14 | Final contract inventory, transformation allowlist and complete synthetic preservation graph |
| AC-02 / original AC-14 | Fresh-baseline versus migrated schema structure and behavior, current no-op, tamper/future/corrupt rejection |
| AC-03 / original AC-14 | Staged conversion and activation fault matrix, read-only refusal, compatible backup recovery and zero-provider counters |
| AC-04 / original AC-14 | Privacy-safe actual-library adoption/reopen/restore receipt; human required, never inferred from CI |
| AC-05 / original AC-14 | Proven runtime retirement only after adoption, immutable archive provenance and exercised historical-backup recovery |
| AC-06 / original AC-14 | Typed-value/asset preservation through complete restore; distinct content transfer/sync admission with privacy tests |
| AC-07 / original AC-15 | Evidence-backed deletion ledger and focused behavior regressions; canonical docs/index/link/command audit |
| AC-08 / original AC-15 | Workflow failure-to-regression mapping, branch isolation, nonempty test selection and correct exact-head evidence |
| AC-09 / original AC-15 | Frozen-head static/fast/full Linux/Android package/emulator results with fresh/reuse attribution |
| AC-10 / original AC-15 | Combined scenario and separately observed source quality, PDF, narration, physical-device and office-app gates |

Extend current `database/*entry*tests`, `library_backup/current_format_tests`, native migration/backup/transfer tests, focused feature lifecycle suites, script policy tests and Android instrumentation. Proposed additions may include baseline-equivalence tests, converter fault-injection fixtures and archived-recovery validation. Select names and wrappers from the then-current repository rather than claiming these files already exist.

Qualify migrations using synthetic isolated data by default. No CI job needs the user's library or documents. Assert exact semantic digests, managed-file hashes, foreign-key integrity, supported decoders and explicit admissible differences. Preserve first-result/review semantics and source ownership across conversions.

## 6. Final checkpoint and scope control

At each coherent publication, record PR/branch, expected parent/tree and verified head, changed owners, acceptance IDs, first failure, actual checks/tested SHAs, next increment and human blockers. One bounded reconstruction after interruption, then resume the concrete boundary. No repeated whole-tree inventories or alternate tool deployments.

Planning branches are isolated Markdown proposals. A future dormant PR may use a frozen non-main planning base to avoid triggering the current main-targeted workflows, but that is not CI success and must be disclosed. Do not edit workflows or use skip tricks merely to park a plan. Activate against current merged `main`, verify the resulting diff and run the then-applicable gates before implementation/review readiness.

The final handoff distinguishes implementation, publication, automated qualification, human acceptance and merge. It names exact candidate SHA, preservation/conversion receipts, archived recovery location, rollback constraints, unrun environments and writer release. Optional residual improvements remain explicit follow-ups; mandatory preservation or acceptance is not relabeled optional to claim completion.
