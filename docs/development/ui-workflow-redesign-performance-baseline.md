# UI workflow redesign Stage 0 performance baseline

This document defines the measurement and evidence contract for the two Stage 0D items in [ADR 0049's roadmap](ui-workflow-redesign-roadmap.md#stage-0--baseline-characterization-and-contracts). It is baseline evidence, not a claim that the current interface is optimized. Stage 4 owns drag optimization, Stage 12 owns broad responsiveness work, and Stage 13 retains human perceived-performance acceptance.

## Privacy and build boundary

Performance collection is available only when `OSTUDY_UI_PERFORMANCE_CAPTURE=1` builds a separate Tauri artifact with the installed `react-dom/profiling` renderer. Ordinary builds use `react-dom/client`, do not install `window.__OSTUDY_UI_PERFORMANCE__`, and do not collect measurements. The capture bridge accepts only enumerated scenario IDs, DOM element handles, profiler boundary IDs, and bounded numeric timing/count data. It does not serialize the DOM or retain card text, document text, filenames, prompts, provider responses, credentials, identifiers, screenshots, or raw browser traces. The capture bridge performs no network I/O. In Actions, the workflow uploads only the explicitly bounded synthetic manifest, deck-drag measurement, and captured log artifacts described below.

The explicit capture operation creates a temporary XDG data/config/cache/state/runtime home, generates a validated synthetic complete-library backup through cfg(test)-only Rust support, installs it as a pending restore inside that temporary application-data directory, builds into a separate temporary Cargo target, and deletes the complete boundary on exit. On its private Xvfb display, the launcher pins X.Org's fake vblank clock to 60 Hz, starts an isolated Openbox process, verifies the EWMH window-manager selection, and runs candidate WebKitGTK with its documented software-compositing mode because Xvfb exposes no DRI3 device; capture fails closed instead of timing a throttled, unmanaged, or unusable accelerated surface. It never resolves or opens the learner's normal ostudy data, documents, managed assets, credentials, or settings.

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

- `tests/performance-baselines/ui-workflow-redesign-stage-0/manifest.json` records the exact harness source SHA, application binary hash, profiling identity, versions, 1440×900 CSS viewport, Light theme, 100% scale, full motion, rich materials, fine pointer, device-pixel ratio, and the 1920×1200×24 Xvfb display at 96 DPI with a pinned 60 Hz fake vblank clock.
- `tests/performance-baselines/ui-workflow-redesign-stage-0/budgets.json` contains the derived budgets, formulas, source statistics, rounding rule, comparison operator, and rationale.
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

Normal `verify` is source-sensitive: it compares every measurement-sensitive
file in the current tree with the manifest's recorded Stage 0 source and fails
when those bytes drift. A separate structural check verifies the immutable
reference at its own recorded identity without impersonating that source:

```bash
./scripts/ui-performance-baselines.sh verify-reference
```

Explicit capture on the clean reviewed harness checkpoint (native build, Xvfb, Openbox plus `xprop`, WebKitWebDriver, and `tauri-driver` required):

```bash
./scripts/ui-performance-baselines.sh capture
```

Read-only comparison of a compatible candidate evidence directory:

```bash
./scripts/ui-performance-baselines.sh compare /absolute/path/to/candidate
```

Stage 4A adds safer candidate-only entry points without changing the committed
reference evidence:

```bash
./scripts/ui-performance-baselines.sh capture-candidate /absolute/new/output-directory
./scripts/ui-performance-baselines.sh compare-candidate /absolute/path/to/candidate
```

`capture-candidate` requires a clean committed HEAD and a new absolute output
path outside the repository. It rejects symlink aliases, the committed
reference tree, and existing destinations; uses the distinct
`ui-workflow-redesign-stage-4a-deck-drag-large-library` evidence identity and
large-library fixture; captures only `deck-drag-v1` with three warmups and
twelve samples; verifies a sibling staging tree; and installs it with an atomic
no-replace rename. It never invokes another performance scenario, updates the
roadmap, or changes committed reference evidence. `compare-candidate` is
read-only, requires compatible source and environment identity, and evaluates
only the committed drag limits for avoidable React commits, visible feedback,
and maximum pointer-frame work. It returns nonzero when any of those three
limits fails.

The host-side CI wrapper is:

```bash
./scripts/check-ui-performance-candidate.sh /absolute/new/output-directory
```

It runs `capture-candidate` and then `compare-candidate` in that order, rejects
an existing destination instead of overwriting it, propagates either failure,
and retains installed evidence when comparison fails. The authoritative `Check`
workflow runs this wrapper only after successful Linux full qualification for
`linux-full` and `all-full`, using a runner-temporary directory named for the
exact `QUALIFIED_SHA`. Its machine-readable result includes the performance-step
outcome, and its bounded artifacts retain the log tail, raw manifest, and drag
measurement; a failure artifact also retains the complete capture/comparison
log. Native capture for a new source head must come from that CI step—an older
user-run candidate cannot qualify a changed head.

Comparison retains the raw `platform.platform()` and Python version strings as
provenance. Its fail-closed parser accepts only the supported canonical Linux
shape. Compatibility requires the same kernel major/minor ABI while allowing
kernel patch/package drift, and it allows only Python patch drift. It also
requires exact OS family, architecture, libc family plus major/minor ABI,
Python major/minor, device-pixel ratio, viewport/display, fixed UI metadata,
captured driver-command output, profiling identity, source and application
hashes, scenario/sample structure, and budget contracts.

Before timed samples, Stage 4A verifies that the timing session is visible and
samples seven consecutive animation-frame intervals; a median above 50 ms fails as
a throttled rendering surface before any product measurement is accepted. It then
starts a separate disposable driver/application session, opens the isolated Xvfb
display through XTEST,
calibrates X-root coordinates against a browser-observed trusted event, injects
native motion/button-down/threshold-crossing motion/Escape/button-up, and requires
WebKit to report `isTrusted` mouse Pointer Events with the expected target,
coordinates, primary-pointer continuity, and button state. The real product
preview must appear and clear on trusted Escape without a drop, the exact
synthetic organization structure must remain unchanged, and cleanup
unconditionally releases button 1. This is the automated Linux
mouse-input smoke. Its process is stopped before the measurement driver and
application start, and its exact attestation is required in the candidate
manifest, so native-state residue cannot enter the three warmups or twelve
timed samples.

The Stage 4A candidate timing path then dispatches deterministic, standards-shaped
DOM `PointerEvent` objects at the verified noninteractive deck-row point. The
tested Linux/WebKitGTK driver/runtime built and launched the candidate successfully but
did not translate either W3C touch actions or distinct W3C fine-mouse actions
into the React Pointer Events consumed by the new adapter. Candidate timing
therefore does not depend on WebKitWebDriver pointer-action translation: it
dispatches pointerdown before arming, dispatches a same-target mouse pointermove
strictly beyond the 8px activation threshold after arming, proves real preview
appearance, finalizes the visible-frame timing while that preview remains mounted,
and then proves cancellation cleanup. Activation dispatch, preview readiness, and
`bridge.finish()` execute inside one bounded asynchronous browser command, so
WebKit can service the response frame without an idle transition between automation
commands. This ordering excludes both cross-command scheduling and cleanup latency
from the visible-response metric. This
deterministic DOM-event performance path exercises the production React drag
adapter, preview, cancellation, Profiler, User Timing, and pointer-frame
instrumentation. Those samples do not themselves prove operating-system input
translation; the preceding trusted XTEST smoke covers the supported Linux mouse
path. Physical touch/pen behavior and subjective interaction feel remain manual
acceptance boundaries.

Verification and comparison never rewrite evidence. Capture stages all JSON in a sibling temporary directory, strictly verifies it, replaces the canonical tree only after success, and then checks exactly the two remaining Stage 4 roadmap rows. The verifier rejects duplicate JSON keys and entries, missing/extra scenarios or files, non-finite/malformed values, applicability errors, stale ordinals/budgets/versions/source SHA, noncanonical paths, hash mismatches, and incompatible reference-runtime metadata.

## Limitations

The committed distribution was captured on the pinned codex-dev
Linux/WebKit/Xvfb/EWMH reference environment. Exact-head Actions applies its three
drag maxima as absolute SLOs on the separately maintained `ostudy-unified`
runner profile after guest preflight; this is not represented as a same-host
statistical comparison. The current manifest verifies runtime ABI, display,
viewport, UI, profiling, source, application, and captured driver-command
identities, but does not encode hardware identity. Results from other machines
are therefore diagnostic rather than merge evidence. The automated evidence
does not establish human-perceived smoothness, physical touch/pen behavior, or
Stage 12 final performance acceptance.
