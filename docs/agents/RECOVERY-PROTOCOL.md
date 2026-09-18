# Interruption recovery and ANCHORLINE protocol

This document is the durable recovery contract for ChatGPT development on oStudy. It exists because an interrupted response, stale conversational state, or temporarily unavailable action schema must not turn into repository rediscovery, false access claims, repeated user instructions, or discarded implementation.

## Recovery cue

The phrase **ANCHORLINE RESET** means:

> Preserve useful work, verify the authoritative repository state once, stop optional investigation loops, resume the existing assigned scope, and report exact evidence.

It never means Git reset, rollback, force-push, discard changes, restart the project, reduce the requested scope, or ask the human to repeat instructions that are already present or retrievable.

Apply the same behavior whenever a development turn is clearly a resumption after an interruption, even when the cue is not used.

## One bounded resumption read

On resumption, do one bounded state reconstruction:

1. read the actual PR or current base, branch, and remote head;
2. read unresolved review/failure state and exact-head Actions evidence;
3. compare any PR-body ledger or remembered SHA with the remote head;
4. identify the next unfinished acceptance boundary or first concrete failure.

Then continue from that boundary. Do not repeat repository inventory, connector discovery, architecture surveys, or old failure analysis unless new evidence makes one of them necessary.

Conversation and memory preserve intent. GitHub and exact-SHA qualification preserve implementation facts.

## Keep an active checkpoint

During implementation retain a compact checkpoint containing:

- repository, PR, branch, expected parent SHA and parent tree SHA;
- current acceptance increment or failed boundary;
- reviewed changed paths, modes and acknowledged blob SHAs;
- candidate commit SHA when one exists;
- verified published head;
- exact qualification run/SHA and first actionable failure;
- next concrete action.

This checkpoint is progress metadata, not a source writer or branch lock. Created blobs or commits are not published until the branch ref is verified.

## Tool and access discipline

Discover each required GitHub action once in the available tool context and invoke the intended operation. A GET-only fetch action, unloaded mutation schema, missing local gh/token/toolchain, truncated tool output, queued runner, formatter failure, or stale PR ledger is not evidence that repository write access disappeared.

Distinguish:

1. action exposure;
2. successful authenticated read;
3. successful concrete mutation;
4. verified branch publication;
5. qualification on the exact published SHA.

When a real operation fails, report the operation, target, exact error, and which neighboring capabilities still work. Do not generalize a narrow failure into “GitHub is unavailable,” make a no-op access-test commit, or switch execution systems unless the human requested that change.

## Progress and response discipline

For substantial development work, surface a brief progress update after a meaningful publication or roughly every few tool operations. An update is not a request for permission to continue and does not replace the final response.

Do not withhold all useful evidence until every requested stage is complete. Publish coherent increments, verify them, and keep working through the assigned scope.

Every completed turn must produce a substantive response containing the verified head, useful changes, actual tests and tested SHAs, unrun or failed gates, and the next unfinished boundary. Never promise background work.

If the platform interrupts a response, the next turn resumes from the bounded repository read above rather than asking the human to restate the task.

## Bounded failure loop

For a failure:

1. read the concrete assertion/log and owning source;
2. change the code or the hypothesis;
3. run the smallest meaningful reproduction;
4. publish the coherent fix and verify the head;
5. qualify that SHA.

One unchanged-head retry is allowed only with evidence of a transient cause. The same fingerprint twice requires a strategy change. Do not weaken tests, add arbitrary sleeps/timeouts, toggle PR state, widen permissions, skip required gates, or make no-op commits to obtain green CI.

## Qualification claims

Workflow success and gate coverage are separate facts. Inspect the named steps and tested SHA. Skipped Linux, Android, live-provider, physical-device, accessibility, screenshot, or manual checks remain unrun even when the workflow itself is green.

A historical green ancestor does not qualify the current head except through the repository's explicitly documented qualification-reuse path.

## Scope boundary

This protocol reduces workflow failures that are under repository/agent control. It does not prove that ChatGPT platform-level response interruption, browser hangs, or service failures have been repaired. Preserve enough repository state that those interruptions do not force the human to become the recovery mechanism.
