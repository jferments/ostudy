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

## Startup and historical pre-public recovery

Native initialization still fails closed rather than escaping through Tauri setup as a Rust panic. The current runtime accepts the supported public lineage and does **not** auto-convert the historical final-v26 development lineage.

An old/non-public database uses:

```text
target: ostudy.app.startup
event: application_startup_blocked
reasonCode: pre_public_database_requires_manual_recovery
```

The database is left unchanged. On desktop, the dialog points to the explicit source-checkout recovery entry point:

```bash
bash ./scripts/transition-pre-public-data.sh \
  --data-dir "/absolute/path/to/io.github.jferments.ostudy" \
  --confirm
```

For a separate timestamped v26 archive, add `--source-backup "/absolute/path/to/archive"`. The recovery utility copies the source before SQLite opens it, verifies the exact historical migration/repair contract, converts only a staged copy to the immutable `ostudy-public-v1` baseline, compares learner-table counts and critical identities, runs integrity/foreign-key checks, archives the previous active directory, and leaves the preserved source untouched. Current startup then uses only the ordinary public migration chain.

The full Linux gate exercises both halves: the built application must refuse a seeded exact-v26 database without changing it; the explicit recovery tool must convert an isolated copy; then the same built application must migrate that recovered public-v1 copy through the current schema and finish startup while preserving representative deck/card/review/analytics state.

Unexpected native initialization errors continue to use `application_startup_failed` with a bounded reason code rather than exposing arbitrary internal error details.

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

For a local crash, start with the zero-placeholder summary:

```bash
./scripts/diagnose-last-crash.py
./scripts/diagnose-last-crash.py --json
```

It resolves the active launcher log, application log and library automatically, interprets negative subprocess exit codes as Unix signals, reports launcher-recorded resource pressure when available, correlates application events and the nearest retained generation operation to the selected crash window, and best-effort checks systemd coredump/kernel evidence. It selects the most recent abnormal launcher exit rather than blindly selecting the most recent launch, so reopening oStudy after a crash does not erase the useful diagnostic target. Older launcher logs remain useful: a legacy negative `exit_code` is still decoded even though resource sampling and the child PID were not recorded yet.

Create a shareable local archive with:

```bash
./scripts/export-diagnostics.sh
```

The archive now includes `last-crash.txt` and `latest-generation-usage.txt` automatically in addition to environment/build context and bounded tails of the current and previous application logs. An explicit output path remains optional.

Repository diagnostics default to the desktop development profile, `io.github.jferments.ostudy.dev`, so developer tooling does not inspect the production learner library by default. Development and test environments can override the application-data location with `OSTUDY_APP_DATA_DIR` or the exact log with `OSTUDY_DIAGNOSTIC_LOG_PATH`. Production support tooling must select production state deliberately rather than inheriting the developer default. The archive excludes the learner database, cards, managed images, source documents, prompts, responses, and credentials.

## Public schema manifest maintenance

The public baseline manifest is generated by the same bundled SQLite engine used by the application. Ordinary tests leave the fixture untouched. After an intentional pre-release baseline edit, regenerate and verify it with:

```bash
OSTUDY_REGENERATE_PUBLIC_SCHEMA_MANIFEST=1 \
  cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib \
  database::tests::regenerate_public_baseline_schema_manifest -- --exact

cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features --lib \
  database::tests::latest_schema_is_idempotent_and_public_baseline_fixture_is_immutable -- --exact
```

After the public baseline ships, do not regenerate it to absorb schema changes. Add immutable forward migrations instead.