# ADR 0020: Generated-card direction, large-document normalization, additional passes, and practice exit

- **Status:** Accepted
- **Date:** 2026-08-09
- **Scope:** PR #38 follow-up after real-provider/manual PDF testing

## Context

Manual acceptance of the Stage 4 document workflow exposed four product/architecture problems that deterministic provider fixtures did not reproduce by themselves:

1. structurally valid generated cards could still be directionally poor when ordinary question/answer cards were marked reversible;
2. some llama.cpp / Unsloth server builds rejected otherwise valid document requests containing newer optional structured-output/template extensions;
3. a large, structurally complex PDF could reach the persistence boundary with third-party semantic metadata that exceeded ostudy's stricter SQLite limits;
4. users need both a deliberate second pass for more coverage and a safe way to leave an active Practice session without silently abandoning it.

## Decision

### Generated-card direction is owned by Rust

The model no longer chooses whether an ordinary Basic card is reversible.

- `basic`, `multiple_choice`, and `critical_thinking` are always one-way question → answer cards.
- `definition` is reserved for a straightforward term/definition pair and normalizes to a reversible Basic card.
- `cloze` retains its own canonical Cloze behavior.

The generation prompt states the same rule for content quality, but the durable guarantee is native normalization: Basic output is always written with `reverseEnabled=false`, while Definition output is converted to `front=term`, `back=definition`, `reverseEnabled=true`.

This prevents a provider from producing a readable JSON response that accidentally turns an explanatory answer into the practice prompt.

### Document timeout ends at provider acknowledgment

For long document tasks the configured request timeout protects request establishment and receipt of HTTP response headers. Successful response headers are the provider-acknowledgment boundary. After that point there is no ostudy wall-clock deadline for first token or completion; the task ends on completion, explicit user cancellation, transport failure, or a response-size/contract limit.

Document requests use ordinary streamed Chat Completions and do not require optional llama.cpp prompt-progress or SSE-ping request fields. This broadens compatibility with llama-server / Unsloth builds while preserving bounded streaming and exact-document cancellation.

For llama.cpp only, if the normal JSON-Schema request is rejected with an HTTP bad-request response, ostudy performs one same-provider/same-model compatibility retry using JSON-object output, removes optional chat-template kwargs, and places the exact expected schema in the trusted system instructions. Strict local parsing and normalization remain authoritative. This is not provider fallback.

### Third-party PDF metadata is normalized before SQLite

`oxidize-pdf` output is treated as untrusted structured input at the persistence boundary. Before durable insertion ostudy now bounds or sanitizes:

- parent headings;
- heading-path depth and item lengths;
- font names;
- chunk bounding-box counts;
- chunk element-type counts and lengths;
- heading context;
- non-finite geometry, confidence, and font-size values;
- negative box widths/heights.

This keeps complex textbook metadata inside migration 0018's strict column/JSON limits instead of surfacing an opaque `database_error` for otherwise usable source material.

### Generate more cards appends to the same generated deck

A ready retained document with an active generated deck exposes **Generate more cards**.

The additional pass:

1. loads the existing persisted outline and semantic chunks;
2. builds a bounded summary of current active card question/answer coverage;
3. adds that coverage as a trusted second-pass constraint;
4. re-runs the semantic generation windows against the same document;
5. asks for important concepts/mechanisms/distinctions/applications not already meaningfully tested and rejects paraphrase-for-volume behavior;
6. globally curates the new candidates;
7. appends through the canonical generated-import path to the exact existing deck;
8. retains ordinary duplicate detection, tags, safe Markdown/Cloze validation, fresh schedules, and transactional persistence.

The destination is checked both by expected deck ID and by unique active deck name before append so a same-name user deck cannot receive the cards accidentally.

A second pass has its own durable generation job/window provenance. Failure or cancellation leaves the previous generated deck unchanged and returns the document to a usable ready state.

### Processing phases stay lightweight

Documents continues to use the durable `processing` lifecycle state but supplements it with a small muted phase label derived from durable progress:

- `Extracting text / rendering pages` before page metadata exists;
- `Rendering pages` when only some renders are recorded;
- `Building outline / generating cards` once local page processing is complete;
- `Generating additional cards` for a second pass;
- `Stopping` while an exact-document cancellation is being requested.

The phase label is intentionally descriptive rather than pretending to provide per-page progress that the current blocking local extraction/render transaction cannot yet report accurately.

### Leaving active Practice requires explicit termination

When a Practice session is active, navigation to another main section asks **Terminate this practice session?**.

If the learner says No/Cancel, navigation is aborted. If they confirm, the Practice screen invokes the existing durable session-completion command first; only after that succeeds does navigation continue.

Already submitted reviews remain authoritative and continue to affect schedules/history. The navigation-termination path deliberately does not transition the Practice UI into its normal completion screen, so post-review analytics are not opened automatically. A review currently in an uncertain/pending retry state blocks termination rather than risking ambiguous review persistence.

## Consequences

- Provider prompts still guide quality, but card direction is no longer left to model judgment.
- Older llama.cpp-compatible servers have a safer structured-output compatibility path without weakening local validation or switching providers.
- Large structured PDFs are less likely to fail at the database boundary because parser metadata is normalized to ostudy-owned limits.
- Users can request progressively broader coverage from a retained chapter without creating replacement decks.
- The Documents UI communicates long-running phases without inventing false precision.
- Practice navigation no longer silently abandons an active session, while already completed reviews remain durable and navigation termination avoids the normal analytics transition.

## Validation

Automated coverage includes:

- Basic versus Definition reversibility normalization;
- Basic schema exclusion of model-controlled `reverseEnabled`;
- semantic metadata finite/bounded normalization;
- second-pass coverage planning and image sampling;
- additional-pass typed IPC and UI behavior;
- processing-phase UI behavior;
- active-practice navigation confirmation and safe termination;
- existing cancellation, backup, deletion, PDF extraction/render, canonical authoring, and native smoke suites.
