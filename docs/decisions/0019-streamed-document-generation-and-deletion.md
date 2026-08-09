# ADR 0019: Streamed document AI, explicit cancellation, and deletable managed PDFs

- **Status:** Accepted
- **Date:** 2026-08-08
- **Scope:** PR #38 follow-up

## Context

Real llama.cpp / Unsloth testing showed that a provider profile's request timeout was being applied to the complete HTTP response. A local model can legitimately need substantially longer than that to finish an outline or flashcard batch even though the request was accepted and inference is progressing. Treating one timeout value as both connection-health detection and a generation deadline caused healthy long-running work to fail.

The Documents UI also needed an explicit way to stop a slow request, native PDF opening was blocked because the WebView did not have generic path-opening permission, and archive-only document lifetime was insufficient for users who want to reclaim managed storage permanently.

## Decision

### Stream long-running document completions

Provider discovery and explicit connection/capability probes keep their existing bounded non-stream requests. Document-intelligence Chat Completions run as Server-Sent Events.

For document tasks only, `requestTimeoutMs` is interpreted as the maximum time to establish the request and receive the first provider response-body activity. After that first activity there is no ostudy wall-clock generation deadline.

The llama.cpp request enables prompt progress and periodic SSE pings where the server supports those completion options. DeepSeek's streamed API can provide its own keep-alive comments while queued. Either provider's first streamed bytes count as acknowledgment that the request is alive; progress/keep-alive events do not become model content.

Streamed `choices[].delta.content` is accumulated behind the native AI gateway into the same bounded provider-neutral completion shape already consumed by document intelligence. Wire bytes and final generated content remain independently bounded. Provider response bodies are still not logged or persisted.

### Cancel by closing the exact request stream

There is no ostudy-wide Stop button and no guessed provider job-status API. Each running document task registers one process-local cancellation signal under its document ID. The native gateway waits on that signal while reading the provider stream.

When the user selects **Stop generation**, ostudy cancels only that document task and drops its response stream. The managed PDF remains available. SQLite records the document as stopped/retryable and converts the current generation job to `canceled` when a job row already exists.

Restart remains a separate recovery case: process-local cancellation handles are not durable, and startup reconciliation never resumes network transmission automatically.

### Open managed PDFs only from Rust

The frontend no longer requests a managed source path and no longer calls the generic opener path command. `open_document_source` accepts only a validated document ID, resolves and validates the managed source path in Rust, and invokes the native opener plugin.

This removes the need to grant the WebView `opener:allow-open-path`, which would be a broader local-filesystem capability than the Documents feature requires.

### Support permanent document deletion

Archive remains a reversible organizational action. **Delete PDF** is a distinct destructive action with confirmation.

Deletion is refused while that document has an active generation task. Otherwise ostudy:

1. validates document ID, revision, lifecycle, and the exact managed source path;
2. renames the managed document directory into application-controlled delete staging;
3. starts an immediate SQLite transaction;
4. finds generated decks linked through `document_decks`;
5. converts the generated source reference into a durable text citation in each surviving deck description;
6. deletes the document row, allowing document-only pages, semantics, outlines, jobs, and provenance to cascade;
7. commits the database transaction;
8. removes the staged source/render directory.

A database failure renames the managed directory back. Cleanup residue after a committed deletion is unreachable from the database and is removed by startup reconciliation.

The current Stage 4.3–4.6 generated-card model has document-to-deck provenance rather than a durable per-note file citation table. The preserved citation therefore lives in the generated deck's normal text description. If a later stage introduces note-level source references, permanent deletion should convert those references to detached text citations before the source document is removed as well.

### Settings placement

The Flashcard generation default is presented at the top-left of the AI/LLM tab immediately before the provider-profile list and is labeled **Default AI model**. It displays both the profile name and API model identity when available. The old Default task profile card is no longer shown.

The settings copy also explains that the profile timeout is initial-response protection for long document jobs, not a maximum generation duration. There is still no automatic local-to-cloud provider fallback.

## Consequences

- Slow but healthy local inference can finish without arbitrarily increasing a total request deadline.
- A provider that never connects or never emits initial streamed activity still fails within the configured bound.
- Cancellation is immediate from ostudy's perspective and provider-neutral at the HTTP stream boundary. A cloud provider may have its own accounting semantics after client disconnect; ostudy does not claim undocumented server-side cancellation guarantees.
- The user can reclaim source-PDF/render storage without deleting generated cards.
- Surviving generated decks retain a human-readable source reference even after the managed file is gone.
- The WebView receives less filesystem authority than a generic opener permission would require.
- Process-local cancellation is intentionally unavailable after app restart; durable interruption recovery remains SQLite/startup reconciliation.

## Validation

Automated coverage must include:

- SSE keep-alive/progress events that do not contaminate generated content;
- split streamed JSON events and bounded accumulated content;
- task-scoped cancellation isolation;
- typed IPC for import identity, retry, stop, native open, delete, and archive;
- Documents UI Stop/Delete states and destructive confirmation;
- source-citation conversion before deletion;
- provider timeout copy and Default AI model UI wording;
- existing secret/redaction/no-fallback policy checks;
- full Rust/React/Tauri qualification on the self-hosted runner.

Manual provider acceptance should include one llama.cpp / Unsloth generation that exceeds the configured initial-response timeout in total wall-clock duration but starts streaming before that timeout, a user-canceled generation, DeepSeek streaming, native **Open PDF**, and deletion of a generated source followed by verification that the deck remains with its text source citation.
