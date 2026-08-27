# UI workflow redesign Stage 0 performance baseline

This document defines the measurement and evidence contract for the two Stage 0D items in [ADR 0049's roadmap](ui-workflow-redesign-roadmap.md#stage-0--baseline-characterization-and-contracts). It is baseline evidence, not a claim that the current interface is optimized. Stage 4 owns drag optimization, Stage 12 owns broad responsiveness work, and Stage 13 retains human perceived-performance acceptance.

## Privacy and build boundary

Performance collection is available only when `OSTUDY_UI_PERFORMANCE_CAPTURE=1` builds a separate Tauri artifact with the installed `react-dom/profiling` renderer. Ordinary builds use `react-dom/client`, do not install `window.__OSTUDY_UI_PERFORMANCE__`, and do not collect measurements. The capture bridge accepts only enumerated scenario IDs, DOM element handles, profiler boundary IDs, and bounded numeric timing/count data. It does not serialize the DOM or retain card text, document text, filenames, prompts, provider responses, credentials, identifiers, screenshots, or raw browser traces. Nothing is transmitted over the network.

The explicit capture operation creates a temporary XDG data/config/cache/state/runtime home, generates a validated synthetic complete-library backup through cfg(test)-only Rust support, installs it as a pending restore inside that temporary application-data directory, builds into a separate temporary Cargo target, and deletes the complete boundary on exit. It never resolves or opens the learner's normal ostudy data, documents, managed assets, credentials, or settings.

## Scenario contract

The instrumentation version is `ui-performance-v1`, the fixture version is `ui-performance-fixture-v1`, and all scenarios are independently versioned:

| Scenario | Interaction | Explicit response/ready condition | Pointer metrics |
| --- | --- | --- | --- |
| `navigation-hover-v1` | Move a fine mouse pointer to an inactive wide-sidebar Analytics item. | The target matches native `:hover`; the first post-event animation frame is recorded. | Required. |
| `document-list-interaction-v1` | Toggle Show archived documents. | The deterministic list contains both active and archived synthetic documents; reset hides archived documents. | Not applicable. |
| `deck-drag-v1` | Use W3C touch-pointer actions to activate a deck drag, move outside every target, and cancel. | Drag feedback has cleared and no durable movement is submitted. | Required. |
| `deck-selection-v1` | Toggle one Practice setup deck checkbox. | Workload reconciliation is no longer updating and selection copy has changed; reset toggles the checkbox back. | Not applicable. |
| `study-guide-open-v1` | Open the synthetic active guide. | The shared dialog is visible and its loaded-guide state has replaced the loading state; reset closes the dialog. | Not applicable. |
| `active-practice-progress-v1` | Reveal and rate the next card in one deterministic 16-card scheduled session. | The next sequential studied count is visible. Warmups and recorded samples intentionally consume the same session in order. | Not applicable. |

Every scenario uses three warmups followed by twelve recorded samples. Readiness uses bounded W3C element polling, DOM state, User Timing, React commit callbacks, and animation frames; an arbitrary sleep is not a scenario condition. Missing profiling callbacks, unavailable required metrics, measurement overflow, or unsupported touch-pointer actions fail capture rather than being replaced with fabricated values.

## Evidence and budgets

The explicit capture operation writes only this canonical evidence tree:

- [`manifest.json`](../../tests/performance-baselines/ui-workflow-redesign-stage-0/manifest.json) records the exact harness source SHA, application binary hash, profiling identity, versions, 1440×900 CSS viewport, Light theme, 100% scale, full motion, rich materials, fine pointer, device-pixel ratio, and the 1920×1200×24 Xvfb display at 96 DPI.
- [`budgets.json`](../../tests/performance-baselines/ui-workflow-redesign-stage-0/budgets.json) contains the derived budgets, formulas, source statistics, rounding rule, comparison operator, and rationale.
- `measurements/<scenario-id>.json` contains twelve numeric samples for exactly one scenario. Each sample records interaction-to-visible and interaction-to-ready User Timing, React unique commit count and render duration, unexpected-boundary/avoidable commit count, and either available pointer-frame evidence or an explicit `notApplicable` state.

Budgets are derived mechanically from the captured distribution:

- hover/drag visible latency = observed maximum + one median measured refresh interval;
- hover/drag maximum pointer-frame work = observed maximum + one sample standard deviation;
- avoidable React commit count = observed maximum, with no noise headroom because commits are discrete work.

Values are rounded to three decimal places. Verification recomputes every budget and rejects hand-edited or stale output. These Stage 0 limits provide a later comparison point; they are not aspirational targets and capture does not change product behavior to make them smaller.

## Commands and mutation policy

Read-only verification:

```bash
./scripts/ui-performance-baselines.sh verify
```

Explicit capture on the clean reviewed harness checkpoint (native build, Xvfb, WebKitWebDriver, and `tauri-driver` required):

```bash
./scripts/ui-performance-baselines.sh capture
```

Read-only comparison of a compatible candidate evidence directory:

```bash
./scripts/ui-performance-baselines.sh compare /absolute/path/to/candidate
```

Verification and comparison never rewrite evidence. Capture stages all JSON in a sibling temporary directory, strictly verifies it, replaces the canonical tree only after success, and then checks exactly the two open Stage 0D roadmap rows. The verifier rejects duplicate JSON keys and entries, missing/extra scenarios or files, non-finite/malformed values, applicability errors, stale ordinals/budgets/versions/source SHA, noncanonical paths, hash mismatches, and incompatible reference-runtime metadata.

## Limitations

The committed distribution describes only the pinned codex-dev Linux/WebKit/Xvfb reference environment. Measurements from other runtimes are descriptive and are rejected for automatic budget comparison. The evidence does not establish human-perceived smoothness, physical-device behavior, Stage 4 drag improvements, or Stage 12 final performance acceptance.
