# PR #106 roadmap — integrated qualification, residual cleanup, and workflow hardening

Status: planning-only successor to PR #105. Owned stage: **LK-14**, plus evidence-backed residual schema/code/documentation/workflow cleanup discovered across PRs #103–#105. This roadmap supersedes the earlier draft on this branch where it assigned final baseline consolidation to PR #106.

**Important boundary:** PR #105 / LK-13 owns the one final current-format baseline, isolated conversion proof, actual-library adoption gate, integrated backup/transfer verification, and gated retirement/archive of obsolete migrations/readers. PR #106 audits that result, fixes demonstrated residual defects, removes remaining proven-dead dependencies, and performs whole-product qualification. It must not perform a second baseline consolidation merely to simplify cleanup.

Activation rule: implement only after PR #103, #104, and #105 are reviewed and merged. Begin from the actual merged `main`, verify the predecessor merge SHA, final schema/baseline identity, archive/recovery route, acceptance evidence, and unresolved issue register once, then work only from concrete gaps.

## Product result and limits

The complete shared-knowledge workflow is exercised as one product: retained documents and image collections, folders/ranges, cross-document synthesis, automatic study bundles, rich guide/PDF, Tutor organization, responsive lessons/checks, retained narration, local PowerPoint, and recoverable library state.

The final tree is audited for contradictory docs, demonstrably obsolete code, stale schema/runtime adapters, and workflow problems actually observed during #103–#105. Fixes are narrow and regression-backed. This is not a new feature tranche, a repository rewrite, or an unbounded “clean everything” campaign.

No merge is automatic. No assertion, permission, timeout, state toggle, no-op commit, or recovery path is weakened merely to obtain green CI.

## Inputs from predecessor PRs

PR #106 receives compact, authoritative handoffs rather than replaying entire implementation histories:

- exact merge SHAs for #103–#105;
- current schema/baseline identity and registered forward migrations;
- active payload/read/write versions and managed-file kinds;
- actual-library adoption/preservation receipt from LK-13 or an explicit unresolved human gate;
- historical-backup recovery/archive entry point and provenance;
- current SourceSetSnapshot, synthesis, StudyTarget, guide, Tutor, lesson, narration, and export contracts;
- actual test entry points and last exact-head qualification evidence;
- bounded known-issue/manual-acceptance register.

If a required LK-13 result is missing, keep that dependency explicit. Do not silently relabel unfinished consolidation as LK-14 cleanup.

## LK-14.1 — source, data, documentation, and cleanup audit before freeze

### Evidence-backed issue register

Every finding must name: exact source SHA, user-visible or invariant impact, owning paths, concrete source/caller evidence or reproduction, affected acceptance boundary, smallest regression, and disposition.

Classify findings as:

- blocking product/data/privacy defect;
- bounded maintenance required for acceptance;
- intentional compatibility;
- live platform fallback;
- human-only/manual acceptance;
- nonblocking follow-up.

A suggestive filename, historical label, unused-looking parameter, or negative search result is not enough to delete code. Trace active composition roots, IPC registration, dynamic imports, serialized checkpoints, platform capability gates, package manifests, restore paths, and tests.

Add or identify a regression before changing behavior where feasible. Fix defects at their owner; avoid broad formatting/dependency churn.

### Verify LK-13 consolidation without repeating it

Audit that startup and complete restore use the intended current admission boundary, that fresh-baseline and migrated libraries remain equivalent in structure/behavior, and that managed-file preservation receipts still cover all retained data.

Verify ordinary runtime/startup/package paths no longer depend on archived historical migrations/readers, while the explicit historical-backup recovery route remains usable. Historical migration bytes/checksums and archive provenance must not be rewritten to create a cleaner story.

If a **newly demonstrated** post-baseline schema defect requires change, add a small forward migration/repair with preservation and fault tests. Do not rebuild or squash the baseline a second time. Any repair that changes actual-library assumptions must identify whether renewed local adoption evidence is required.

Tests include current no-op admission, future/unknown/tampered/corrupt read-only refusal, restore/startup parity, archive recovery, transactional forward repair, typed-value preservation, foreign-key/FTS behavior, and managed asset/audio hashes.

### Proven-dead code removal

Use a deletion ledger: candidate owner, current direct/indirect callers, serialized entry points, platform fallback need, replacement, exclusive styles/dependencies/tests, and regression proving behavior preservation.

Remove only code with no current runtime, compile-time, dynamic, test, packaging, platform, or supported compatibility consumer. Preserve meaningful historical fixtures, supported import readers, offline recovery tools, and platform fallbacks.

An awkward live module is refactoring scope, not “dead code.” Do not collapse distinct Basic/Cloze/objective content, human/generated origins, or privacy boundaries just because types look similar.

### Documentation reconciliation

Source-check and reconcile canonical user/development/architecture/reference documentation for:

- navigation and visible actions;
- document/image upload and generation lifecycle;
- folder/range/Tutor source semantics;
- guide/PDF behavior and limitations;
- lesson/check answer privacy;
- narration route/cost/retention behavior;
- PPTX disclosure/interoperability;
- current baseline, backup/restore, sync/transfer;
- build/install/debug/diagnostic commands;
- manual acceptance limitations.

Edit canonical owners rather than appending contradictory PR diaries. Keep historical decisions clearly historical and update indexes/links together.

## Workflow repairs collected from PRs #103–#105

Workflow work in this PR is allowed only for concrete observed failures with an owning regression.

### Planning-only draft classification

Current policy runs self-hosted Check on draft PR events to `main`, selecting static/fast for drafts, and PR Contract independently. That is correct for product-bearing drafts but expensive for planning-only Markdown.

A proposed improvement is a **complete changed-file classifier** with a narrow allowlist for roadmap-only Markdown. Planning-only changes would receive focused docs/metadata validation while product-bearing drafts retain static/fast. Ready candidates and explicit full qualification retain full/platform requirements.

Default to the stronger gate if:
- source/tests/assets/scripts/workflows/config/lockfiles are present;
- agent-policy or security-sensitive docs are changed;
- a rename/delete crosses the allowlist;
- changed-file enumeration is incomplete;
- scope is mixed or ambiguous.

Do not depend on titles, labels, commit messages, broad `paths-ignore`, or a fabricated “tests passed” status. Required-check identity and applicability must remain truthful.

### Qualification evidence and failure handling

Preserve exact tested SHA, selected gate, named step outcomes, fresh execution versus permitted reuse, bounded diagnostics, first actionable failure, and focused reproduction.

A green workflow does not mean skipped Linux/Android/manual gates passed. An older ancestor is historical evidence unless the explicit fingerprint-reuse policy applies and is reported.

Keep formatting, compile, test-contract, runner/environment, connector, provider, and human-only failures distinct. One unchanged-head retry requires transient evidence; identical failure twice requires a changed hypothesis.

### Cross-PR concurrency and interruption recovery

Verify that automatic/manual concurrency keys cannot cause an unrelated planning/feature PR to cancel another branch's qualification. Shared runner capacity is still finite even with separate cancellation groups.

Retain GitHub-direct one-writer publication, expected-parent Git data, independent blob acknowledgement, off-ref diff inspection, non-force ref updates, exact-SHA qualification, and the ANCHORLINE bounded-resumption protocol.

Repository workflow changes cannot prove or claim that ChatGPT platform response interruption/throttling has been fixed. The goal is to make interruptions cheaper to recover from by preserving exact publication state and not forcing repeated repository reconstruction.

## LK-14.2 — freeze and exact-head terminal qualification

Complete the broad source/diff/docs/data/cleanup audit **before** terminal qualification begins. Resolve all blocking findings, prepare the handoff, and freeze one exact candidate.

Then run and record, against the frozen SHA:

- maintained static qualification;
- fast qualification;
- authoritative full Linux qualification;
- Android compile/package;
- Android emulator smoke;
- any required repository contract checks.

Qualification reuse, if allowed by the then-current conservative product/toolchain fingerprint, must identify the earlier producing SHA and the exact reused tier. Source, tests, scripts, workflows, config, assets, lockfiles, and other invalidating paths require fresh applicable execution.

Once expensive gates are running, do not reopen broad cleanup for optional wording/refactor opportunities. Diagnose actual failures; nonblocking late findings become follow-up work.

**Exit / automated AC-15 boundary:** exact-source repository/platform evidence is complete and accurately scoped.

## LK-14.3 — combined Week 1 acceptance scenario

Exercise the entire retained workflow on one immutable candidate:

1. Import a reading, sparse slides, notes/assignment, and overlapping screenshots/images.
2. Verify originals/evidence are retained and image overlap does not collapse changed values.
3. Create/use a Week 1 folder with bounded ranges and exclusions.
4. Generate one coherent bundle without multiplying decks.
5. Preserve edited cards/schedules and guide overlays through a source update.
6. Ask folder/project-scoped Tutor questions and prove excluded material cannot leak through dependencies.
7. Read the semantic guide and export PDF offline.
8. Open/resume a responsive lesson, answer a check, and confirm no unintended FSRS review.
9. Synthesize the selected verified voice once under explicit authorization.
10. Replay offline, seek/change local speed with zero TTS calls, switch desired voice without immediate whole-library spending, and switch back to retained compatible audio.
11. Export editable PPTX with correct notes/disclosure and already-saved audio without provider requests.
12. Create/verify a complete backup, exercise staged restore/reopen, and confirm retained audio/media integrity.
13. Add the already prepared reading to Week 2 and confirm no document re-extraction/reinterpretation is triggered.

Use explicit request counters where deterministic and permitted. Preserve IDs, edits, schedules, responses, source scopes, suppression state, and managed files.

This scenario supplements—does not replace—the stage-specific regressions in prior PRs.

## LK-14.4 — human/manual acceptance

Record actual source SHA, platform/application version, and observed result for every manual gate that is performed. Missing environments remain explicitly unrun.

Human gates include, as applicable:

- held-out source extraction/teaching quality and factual faithfulness;
- PDF typography, page breaks, reading order, and math/glyph appearance;
- responsive/accessible desktop and Android layouts;
- physical Android Back/audio focus/interruption/Bluetooth/background behavior;
- live speech pronunciation and cost/request behavior;
- Microsoft PowerPoint and/or LibreOffice Impress editability/notes/audio;
- final actual-library preservation/recovery if an LK-13 repair invalidated prior evidence.

Mocks, parsers, emulator success, or structurally valid ZIP/PDF files do not certify these observations.

## LK-14.5 — final handoff

The final handoff includes:

- exact base, branch, remote head, and last expected parent/tree;
- changed owners and cleanup ledger;
- current schema/baseline/archive identities and rollback constraints;
- acceptance IDs and remaining non-goals;
- actual test runs, exact SHAs, named gate outcomes, and permitted reuse;
- failed/unrun manual/platform/provider gates;
- local safe checkout/build/run/debug/backup/restore instructions;
- known limitations/follow-up items;
- writer release.

No automatic merge. The human decides whether observed evidence is sufficient.

## Delivery sequence

1. **106-A:** bounded audit + issue/deletion/documentation register.
2. **106-B:** fix blocking integrated product/data defects with focused regressions.
3. **106-C:** residual schema/recovery cleanup only where evidence requires it.
4. **106-D:** proven-dead code and canonical documentation cleanup.
5. **106-E:** evidence-backed workflow repairs, including planning-only draft policy if still desired.
6. **106-F:** freeze exact candidate and run terminal repository/platform gates.
7. **106-G:** combined Week 1 scenario and manual acceptance.
8. **106-H:** final evidence reconciliation and handoff.

Proposed PR title: **chore(release): integrated qualification, residual cleanup and workflow hardening**.
