# Streamed PDF generation, cancellation, and deletion testing

This checklist covers the PR #38 follow-up that changed long document AI requests from fixed wall-clock timeouts to cancellable streaming requests.

## Automated gates

From the PR branch run:

```bash
git fetch origin
git switch agent/stage-4-3-4-6-pdf-flashcard-generation
git pull --ff-only
pnpm install
./scripts/check-fast.sh
./scripts/check.sh
```

The automated suite verifies typed IPC parity, SSE parsing/bounds, task-local cancellation isolation, Documents Stop/Delete controls, destructive confirmation, Default AI model wording, dependency pins, Rust/React builds, interrupted-delete reconciliation, and native smoke tests.

## Long-running llama.cpp / Unsloth acceptance

1. In Settings → AI/LLM, select the llama.cpp / Unsloth profile as **Default AI model** for Flashcard generation.
2. Set that profile's request timeout to a deliberately short but reasonable value, such as 30 seconds.
3. Upload a PDF/model combination known to require substantially longer than 30 seconds to complete an outline or card-generation call.
4. Confirm the server accepts the request and returns successful HTTP response headers within the configured timeout.
5. Confirm ostudy does **not** fail merely because the first token or complete inference arrives later than 30 seconds.
6. Leave the job running until it completes and confirm the generated deck commits normally.

The document transport does not require llama.cpp-specific prompt-progress or ping request extensions. This intentionally keeps the request compatible with older llama-server / Unsloth deployments that support OpenAI-style streamed Chat Completions but reject newer optional request fields.

If a llama.cpp server rejects the normal structured request with HTTP 400/422, ostudy performs one same-provider/same-model compatibility retry using JSON-object output plus the exact schema in the system instructions. Strict local deserialization and normalization remain authoritative; this is not provider fallback.

## Initial-response timeout acceptance

Test a profile that cannot acknowledge the request within its configured timeout—for example, an unreachable endpoint or a server that accepts the socket but never returns response headers.

Confirm:

- generation fails with a concise initial-response/connection error;
- no partial deck commits;
- the managed PDF remains in Documents;
- **Retry generation** remains available.

The configured timeout covers request establishment and response headers. After successful response headers, it is **not** a first-token timeout and is not a maximum total inference duration.

## Stop generation acceptance

1. Start a PDF generation that takes long enough to observe the Processing state.
2. Confirm **Stop generation** is visible only while that document is processing.
3. Select **Stop generation**.
4. Confirm the UI changes to **Stopping…** and does not offer Delete while cancellation is in progress.
5. Confirm the active provider response stream closes and the command returns without waiting for model completion.
6. Confirm the document becomes stopped/retryable and no partial deck is committed.
7. Retry the retained document and confirm a new request can start normally.

If two documents are generating concurrently, stopping one must not cancel the other.

## DeepSeek streaming acceptance

Run a representative text-heavy PDF with DeepSeek selected as the Default AI model.

Confirm:

- document outline/generation/curation use streamed responses;
- provider keep-alive SSE comments do not become generated content;
- total inference and time-to-first-token may exceed the configured timeout after successful response headers;
- Stop generation returns control to ostudy without provider fallback;
- a later Retry uses the same explicitly configured profile unless the user changes the default.

## Open PDF acceptance

From Documents select **Open PDF** for a retained document.

Confirm the operating system opens the managed source PDF and no `opener.open_path not allowed` permission error appears. The frontend should not receive or open an arbitrary filesystem path; opening is performed by the native `open_document_source` command.

## Permanent deletion and detached text citation

1. Generate a deck successfully from a PDF.
2. Inspect the generated deck description and note its source filename.
3. In Documents choose **Delete PDF**.
4. Confirm a destructive dialog explains that the source PDF, rendered pages, extracted structure, outline, and generation history for this document will be deleted while generated study content remains.
5. Confirm deletion.
6. Verify the PDF disappears from Documents and **Open PDF** is no longer available for it.
7. Verify its managed `documents/<id>/` source/render directory is removed.
8. Verify the generated deck and cards still exist.
9. Verify the deck description now contains a text citation in the form `Source PDF (deleted from Documents): <title> (<filename>), <N> PDF pages.` or already contains an equivalent user-edited textual source reference.
10. Verify ordinary practice, scheduling, search, and analytics for the surviving cards still work.

Deletion must be disabled while generation is running. If the deck description is too long to preserve any textual source reference safely, deletion should fail rather than truncate user-authored description text or silently lose source identity.

### Interrupted deletion recovery

Permanent deletion deliberately has a filesystem/SQLite boundary: the managed document directory is renamed into application-controlled delete staging before the database transaction commits. The automated suite covers the pre-commit crash case. On startup, if the durable document row still exists and its normal source directory is missing, ostudy restores the staged directory **before** ordinary document reconciliation can mark the source missing. If the document row is already gone, delete-staging is unreachable residue and startup removes it.

For a manual fault-injection build, terminate ostudy after the directory rename but before the database delete commits, then restart. The document and source PDF should recover rather than disappear. The opposite case—database delete committed but final staged-directory cleanup interrupted—should keep the document deleted and remove the unreachable staging residue on restart.

## Archive versus delete

Confirm **Archive** remains reversible and preserves the managed PDF, all derivatives, and generation provenance. **Delete PDF** is permanent and removes document-only data while preserving generated cards with a detached text citation.

## Backup behavior

- Before deletion, a complete `.ostudybackup` includes the managed source PDF and rendered pages.
- After permanent deletion, a new backup must not contain those deleted files/document rows, but it must contain the surviving generated deck description with its text source citation.
- Restore each case into a disposable library and verify the corresponding retained/deleted behavior.
