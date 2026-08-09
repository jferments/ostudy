# Document platform stabilization testing

This guide qualifies the Stage 4 architecture changes in PR #39 before additional providers or document formats are introduced.

## Local automated gates

During document-platform iteration:

```bash
./scripts/check-changed.sh
```

Run the focused cross-layer document contract directly when changing ingestion, semantic planning, checkpoints, budget logic, migrations, or generation persistence:

```bash
./scripts/check-document.sh
```

Before requesting merge review:

```bash
./scripts/check-fast.sh
./scripts/check.sh
```

Ordinary pull-request CI is read-only. Do not rely on CI to modify rustfmt output or `Cargo.lock`; if canonical generated Rust candidates are required, use the explicit workflow-dispatch action and review the resulting commit.

## Invariant regressions

The automated document lane must prove all of the following:

1. A source descriptor rejects zero source units.
2. A source locator rejects zero/out-of-range/duplicate source-unit numbers.
3. Semantic windows are globally numbered `0..N-1`, including multiple windows inside one partition.
4. The 11-page/single-partition/two-window plan persists two distinct `document_generation_windows` rows.
5. Invalid window identity is rejected before a generation job row is inserted.
6. Provider-reported context can only shrink conservative source windows.
7. A provider context too small for schema/task/output reserves fails locally before traffic.
8. Checkpoint contract identity changes with semantic model/capability/planning/source changes but not credential rotation alone.
9. Checkpoint cache write/read failures are non-canonical and degrade to recomputation.
10. Migration 20 upgrades old schemas while preserving foreign-key integrity.
11. Durable provider adapter keys are no longer closed SQLite enums.
12. PDF ingestion explicitly records `source_format=pdf` and `source_unit_kind=page`.

## Empirical usage benchmark

The committed baseline corpus is:

```text
src-tauri/tests/fixtures/document-generation/usage-baselines.json
```

It stores only observed aggregate acceptance facts and the required benchmark matrix. Do not add estimated values as if they were observations.

After a real-provider acceptance run, analyze one or more diagnostic logs:

```bash
node scripts/analyze-document-usage.mjs /path/to/ostudy-diagnostic.log
```

Machine-readable form:

```bash
node scripts/analyze-document-usage.mjs --json /path/to/ostudy-diagnostic.log
```

For every benchmark run, record externally in the acceptance notes:

- provider and exact model identifier;
- configured/reported context limit if available;
- source format;
- source-unit count;
- text-only versus multimodal;
- planner source-token estimate;
- outline/generation window counts;
- planner aggregate estimate and safety ceiling;
- actual aggregate prompt/completion tokens reported by the provider;
- reasoning/cache token details when available;
- success/failure and the exact safe failure code.

### Required matrix before changing planner coefficients

At minimum collect repeatable observations for:

- small text-only source;
- medium text-only source;
- large text-only source;
- small multimodal source;
- medium multimodal source.

Prefer multiple runs for a provider/model before modifying calibration defaults. Treat provider-declared context as a hard constraint regardless of empirical observations.

## PR #39 real-provider acceptance order

Run expensive tests only after cheaper representative tests pass.

### 1. 11-page llama.cpp / Unsloth regression

Use the exact source/profile that previously produced the generic local database error.

Expected:

- no duplicate-window database error;
- at least two generation windows are allowed when source/window bounds require them;
- window identities remain unique;
- generation proceeds through canonical authoring;
- diagnostic output contains no `document_generation_window_insert_failed` event.

### 2. Small DeepSeek baseline

Re-run the ~11-page text source previously observed around ~45k aggregate tokens.

Compare:

- planner estimate;
- actual aggregate token usage;
- number of provider calls;
- any cache-hit behavior;
- final card count/quality.

A large unexplained regression should block the expensive large-source test.

### 3. Medium local-model source

Re-run the representative ~48-page llama.cpp/Unsloth source.

Verify:

- conservative request windows;
- current model-context capability is honored if available;
- no runaway retry loop;
- interruption can be retried without redoing validated retained work unnecessarily.

### 4. Large DeepSeek source

Only after steps 1-3 pass, re-run the ~138-page source that previously consumed roughly 1.45M aggregate tokens before late truncation.

Expected behavior:

- either the planner rejects the job locally under the configured safety ceiling, or the bounded/resumable run completes without the previous token explosion;
- completed outline/generation windows can be reused after interruption;
- no recursive compact-retry loop;
- diagnostics provide partition/window/source-unit context without raw source text.

### 5. Generate More

On a successfully generated document:

- Generate More appends to the exact existing deck;
- it reuses the retained final outline;
- it uses the same shared generation executor/state transition rules as initial generation;
- it does not create a replacement deck;
- failure/cancellation leaves existing cards intact.

## Lifecycle regression checks

Re-test:

- Stop generation;
- Retry generation;
- Open source PDF;
- Archive/restore document;
- generated-deck deletion followed by regeneration;
- permanent PDF deletion with preserved deck citation;
- interrupted PDF deletion recovery;
- complete backup/restore with source and rendered derivatives;
- Practice navigation from generated deck.

## Merge criteria

PR #39 should remain draft until the focused/fast gates pass. Mark it ready only when the code is stable enough for the full qualification gate. Do not merge until the 11-page llama.cpp/Unsloth regression and representative DeepSeek/local acceptance tests pass in addition to automated qualification.
