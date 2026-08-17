# Diagnostics and debug filters

Normal application and test runs use the `info` filter with noisy dependency targets suppressed by the application taxonomy. Enable targeted diagnostics without shell command tracing:

```bash
./scripts/dev.sh --debug=database,ai
./run-app.sh --trace=qr
./scripts/check-fast.sh --debug=migrations --keep-artifacts
./scripts/check.sh --log-filter='warn,ostudy.database=debug,ostudy.ai.provider.openrouter=trace'
./scripts/android-test.sh --debug=qr --keep-artifacts
```

Equivalent environment configuration:

```bash
OSTUDY_LOG='warn,ostudy.documents=debug' ./scripts/dev.sh
```

Targets include `app`, `database`, `ai`, `qr`, `device_sync`, `documents`, `frontend`, `filesystem`, and `diagnostics`. Target rules use longest-prefix matching. Debug/trace fields must contain only counts, durations, safe identifiers, provider/model identifiers, and normalized outcomes—never credentials, prompts, study content, OCR text, SQL parameters, or QR payloads.

## Retention and performance

Application JSONL files are capped at two 2 MiB generations under the application diagnostics directory. Script failure artifacts contain only bounded log tails and build context under `.artifacts/diagnostics/`; they do not contain the SQLite database or managed user files.
