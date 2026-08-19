# Diagnostics and debug filters

Normal application and test runs use the `info` filter with noisy dependency targets suppressed by the application taxonomy. Enable targeted diagnostics without shell command tracing:

```bash
./scripts/dev.sh --debug=database,ai
./run-app.sh --trace=qr
./scripts/check-fast.sh --debug=migrations --keep-artifacts
./scripts/check.sh --log-filter='warn,ostudy.database=debug,ostudy.ai.provider.openrouter=trace'
./scripts/android-test.sh --debug=qr --keep-artifacts
./scripts/smoke-native.sh --debug=frontend --keep-artifacts
bash ./scripts/smoke-startup-compatibility.sh --debug=database --keep-artifacts
```

Equivalent environment configuration:

```bash
OSTUDY_LOG='warn,ostudy.documents=debug' ./scripts/dev.sh
```

Targets include `app`, `database`, `ai`, `qr`, `device_sync`, `documents`, `frontend`, `filesystem`, and `diagnostics`. Target rules use longest-prefix matching. Debug/trace fields must contain only counts, durations, safe identifiers, provider/model identifiers, and normalized outcomes—never credentials, prompts, study content, OCR text, SQL parameters, or QR payloads.

## Startup and pre-public adoption

A compatibility or operational failure during native initialization must not escape through Tauri's setup hook as a Rust panic. ostudy catches startup failures at the composition boundary, hides an unusable main window when necessary, writes a structured event, presents a native dialog, and exits deliberately after the dialog is dismissed.

An exact final pre-public v26 database is adopted automatically. The expected diagnostic sequence is:

```text
target: ostudy.database.migration
event: pre_public_database_adoption_started

target: ostudy.database.migration
event: pre_public_database_adoption_completed

target: ostudy.app.startup
event: application_started
```

The adoption event fields include only the legacy/public versions, safe backup filename, and deck/card/review counts. They never include deck names, card content, review excerpts, document text, or paths containing user-entered material.

Before the first adoption write, ostudy verifies the complete immutable v26 migration/repair ledger, final schema markers, SQLite integrity, and foreign keys. It then creates a consistent SQLite backup and transactionally changes only the public-lineage metadata and provider-profile shape. All application-table row counts and critical deck, note, card, review, attribution, and AI-profile identities must match before and after the transaction.

An older, partial, checksum-mismatched, damaged, or otherwise unsupported development database uses:

```text
target: ostudy.app.startup
event: application_startup_blocked
reasonCode: pre_public_database_requires_manual_recovery
```

That event means automatic adoption was deliberately refused. The database is not reset or guessed at. The desktop dialog identifies the application-data directory and points to the non-destructive archived-data recovery helper. Starting an empty library remains possible only through the explicitly named `--confirm-empty-library` operation.

Unexpected native initialization errors use `application_startup_failed` with a bounded reason code rather than exposing arbitrary internal error details in the user-facing dialog. The original bounded native error remains available through local process/diagnostic evidence.

The full Linux gate runs `scripts/smoke-startup-compatibility.sh` against the actual standalone binary. Unlike normal WebDriver tests, this smoke deliberately seeds an exact final-v26 SQLite database containing a deck, card, review, and analytics attribution. It requires the built application to create a backup, adopt the public lineage, preserve those rows, finish startup, and avoid `application_startup_blocked`, `panic_captured`, and Tauri setup failure.

## Automated qualification baseline

The authoritative GitHub Actions job enables this low-noise first-run filter automatically:

```text
warn,ostudy=info
```

That baseline records internal lifecycle and operation outcomes while leaving unrelated dependency output at warning level. Capturing the first failing run is intentional: rerunning the complete Linux/Android gate with debug enabled is slower, can destroy the original temporary application state, and can hide an intermittent failure.

CI writes bounded captures under the runner-owned `OSTUDY_DIAGNOSTIC_ARTIFACT_ROOT`. One nested qualification invocation produces one authoritative failure capture: the innermost precise boundary claims the failure before outer wrapper traps can replace it with duplicate `initialization` records. Normal failures include the qualification step, exit status, exact commit, active filter, a recommended subsystem filter, recent launcher/application records, and a focused replay script. Native WebDriver tests copy diagnostics out of their isolated XDG data directory before that directory is deleted. A retryable WebDriver transport disconnect is recorded as a **transient** capture even when the single permitted clean-state retry succeeds. The startup compatibility smoke likewise captures its isolated native startup log before cleanup when it fails.

The Actions failure summary combines three sources:

1. the ordinary gate log and its concise compiler/test failure excerpt;
2. structured application warning/error events from captured JSONL files and Android Logcat;
3. the smallest useful local reproduction command.

This makes the initial CI failure useful without automatically rerunning a costly gate or treating a recovered flake as invisible.

## Focused failure replay

A local check run with `--keep-artifacts` writes a unique directory under:

```text
.artifacts/diagnostics/<utc>-<failed-step>.<unique-suffix>/
```

The `.artifacts/` tree is intentionally Git-ignored and excluded from clean release-source copies. Qualification also compares Git-visible worktree state at test boundaries, so diagnostics or other tooling that accidentally creates an untracked root file fails the responsible boundary rather than leaving unexplained repository dirtiness for a later command.

The directory contains `context.txt` and an executable `reproduce.sh`. Run the generated script first:

```bash
.artifacts/diagnostics/<capture>/reproduce.sh
```

The replay bypasses exact-tree check stamps, keeps bounded artifacts, and uses a step-specific diagnostic filter. Examples of the automatic narrowing are:

- migration/schema failure → `warn,ostudy.database=debug`;
- OpenRouter/document failure → `warn,ostudy.ai=debug,ostudy.documents=debug`;
- WebDriver/frontend failure → `warn,ostudy.app=debug,ostudy.frontend=debug`;
- QR/device-sync failure → `warn,ostudy.app=info,ostudy.qr=debug,ostudy.device_sync=debug`.

For Vitest failures, the Actions summary narrows the replay to the failed test file when it can identify one. For Rust test failures, it narrows to the exact failed test name. Only after the focused command passes should the broader `check-fast.sh`, `check.sh`, or Android matrix be rerun.

Escalate from `debug` to `trace` only for the affected subsystem. Avoid application-wide trace output unless a cross-subsystem ordering defect genuinely requires it.

## Retention and performance

Every encoded diagnostic event is capped at 64 KiB. If structured fields exceed that bound, ostudy retains the event metadata and replaces the fields with an explicit omission record. Free-text redaction recognizes opaque bearer/basic credentials in addition to provider-shaped keys, and tests cover both inline and whitespace-separated authorization forms.

Application JSONL files are capped at two 2 MiB generations under the application diagnostics directory. Script failure artifacts contain only bounded log tails and build context. Local captures default to `.artifacts/diagnostics/`; CI redirects them to runner-temporary storage. They do not contain the SQLite database or managed user files.

Failure-only wrapper/build evidence is retained for 14 days. Small structured diagnostic captures, including recovered transient WebDriver failures, are retained for 7 days. Clean runs with no capture do not create a diagnostic artifact.

## Exporting bounded diagnostics

Create a local archive containing environment/build context and bounded tails of the current and previous application logs:

```bash
./scripts/export-diagnostics.sh
./scripts/export-diagnostics.sh /absolute/path/ostudy-diagnostics.tar.gz
```

The resolver follows Tauri's application-data layout for the `io.github.jferments.ostudy` bundle. Development and test environments can override it with either `OSTUDY_APP_DATA_DIR` or the exact `OSTUDY_DIAGNOSTIC_LOG_PATH`. The archive excludes the learner database, cards, managed images, source documents, prompts, responses, and credentials.

## Public schema manifest maintenance

The public baseline manifest is generated by the same bundled SQLite engine used by the application. Ordinary tests leave the fixture untouched. After an intentional pre-release baseline edit, regenerate and verify it with:

```bash
OSTUDY_REGENERATE_PUBLIC_SCHEMA_MANIFEST=1 \
  cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib \
  database::tests::regenerate_public_baseline_schema_manifest -- --exact

cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib \
  database::tests::public_baseline_is_idempotent_and_matches_manifest -- --exact
```

After the public baseline ships, do not regenerate it to absorb schema changes. Add immutable forward migrations instead.