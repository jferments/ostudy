# Readable learning materials and trustworthy tooling

Status: implementation in progress after PR #113. Base: `7283216a08e90361271395e7d70a8d38de127aa2`; branch: `agent/readable-guides-and-tooling`. This is the September 27 follow-up requested after manual acceptance found improved flashcards but unacceptable study guides and matching drag feedback. Automated qualification of #113 did not establish educational or physical-pointer acceptance.

## Accepted scope

The scope is one requested PR with coherent published increments, not permission for unrelated refactoring. Preserve canonical rendered-page visual evidence and the current concept-first flashcard contract. Input-token reduction is not a reason to omit tables, diagrams, page images, qualifications or source coverage. Native admission proves shape and traceability, not factual entailment; reference evaluation must test that distinction.

| Acceptance | Required outcome |
| --- | --- |
| AC-01 | Matching pointer gestures suppress native text/image selection, show opaque readable moving tiles and a background activation state on the destination, and preserve scoped hit testing, cancellation, stable assignments and keyboard/click/touch alternatives. Test real browser styles/selection separately from component state. |
| AC-02 | Study guides read as concise, coherent subject overviews: a correctly capitalized human title, a narrative introduction, major-topic contents, and logically ordered explanatory sections. Do not dump knowledge-unit labels into learning objectives or treat every fact/drug as a peer chapter. Preserve all authorized evidence, support and original material; use supplemental lists/tables only where they teach better than prose. |
| AC-03 | Default document interpretation/card/guide reasoning is Off; Tutor defaults to Low with Medium available. Distinguish requested, resolved and transmitted reasoning; preserve deliberate user choices and do not silently enable hidden reasoning to repair a response. |
| AC-04 | First successful OpenRouter credential setup proposes/applies `deepseek/deepseek-v4.1-flash` for generation/Tutor and `qwen/qwen3-embedding-4b` for embeddings through the native defaults owner. Verify model/capability availability normally, do not fabricate probe success, overwrite existing assignments or transmit learner material merely because a key was saved. Credential replacement/retry is idempotent. |
| AC-05 | Document/generation diagnostics expose bounded stage/attempt timing, request evidence/image counts, requested/effective configuration and reported usage without private text, images, credentials or reasoning bodies. Unknown usage remains unknown; failure/cancellation cannot erase already observed usage or double count it. Keep Stop and useful cost/progress metrics visible. |
| AC-06 | WebDriver transport activation is explicit; retry safety belongs to identified read operations rather than script substrings or global urllib monkey-patching. Lost mutation replies and assertions are not replayed. Replace identified prose/formatting-pinning tests with meaningful behavioral/structured checks, including negative cases. |
| AC-07 | Native-owned deterministic migration metadata is exported for maintenance consumers with check-only drift verification. Keep release/public/repair identities separate, released SQL and checksums immutable, independent admission and compatibility fixtures intact, and tools usable without building the desktop app. |
| AC-08 | Maintained shell/Python scripts gain pinned correctness-oriented ShellCheck/Ruff checks and focused type checks for safety-critical Python. Existing tests remain; add source-grounded synthetic/reference regressions for learning structure and recovery. No paid or private-source evaluation runs implicitly in CI. |
| AC-09 | Maintained documentation and local handoff agree with implementation. Qualify the exact frozen head through required Static/Fast, Linux full/package and Android compile/emulator gates; report fresh versus reused evidence and unrun live-provider/physical/educational acceptance separately. No automatic merge. |

## Implementation boundaries

Study-guide planning and authoring must be separate from the shared flashcard selection policy. Organize the complete retained topic catalog before authoring, retain deterministic identities/order and support selections, and invalidate only incompatible derived guide checkpoints. Previously generated guides remain readable; regenerating a guide must not rewrite learner cards or reviews. Large documents require bounded grouping/authoring, not topic omission, arbitrary first-N sampling, or a demand to split the source manually.

First-key defaults are an onboarding decision, not a hard-coded capability certificate. The exact public model identifiers were verified against OpenRouter's model pages on September 27, 2026. Model availability remains a runtime discovery/probe concern. Saving a key does not authorize a fallback provider or automatic library-wide embedding upload.

Tooling changes should remove incidental source-layout dependencies without making another status service or general test framework. Generated metadata is compared during qualification; Actions never write maintained source. All destructive filesystem tests use disposable data.

## Validation and manual acceptance

Use existing native, component, script and WebDriver owners with deterministic fixtures. Important negative cases include mutation-before-disconnect, native text selection during a captured-pointer drag, guide contents that flatten leaf facts, omitted topic coverage, migration metadata drift, first-key replay/credential replacement and partial usage on failed requests.

Manual acceptance remains required for readable output on the user's cardiovascular-drugs source, actual selected-provider reasoning behavior, desktop pointer ergonomics, physical touch and exported/readable guide appearance. Do not infer these from a green workflow or valid JSON. Preserve the full visual evidence policy while evaluating reasoning Off; reasoning Off is the requested default, not a claim of equivalent quality on every model.

Follow [GitHub-direct development](../agents/GITHUB-DIRECT-DEVELOPMENT.md), [testing and CI](testing-and-ci.md), and [local testing and handoff](local-testing-and-handoff.md). Keep useful publications recoverable, freeze the final candidate before terminal qualification, and do not merge without a new explicit instruction.
