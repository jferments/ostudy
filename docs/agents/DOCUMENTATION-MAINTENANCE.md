# Documentation maintenance and stale-code review

Documentation is part of the product contract. It must describe the application that exists at the reviewed source revision, not an accumulation of promises, historical implementation steps, and remembered commands.

## Ownership of each document

| Reader's question | Canonical home |
| --- | --- |
| What is ostudy and why use it? | Root README |
| How do I complete a study task? | `docs/user-guide/` |
| How do I install or build for this platform? | `docs/installation/` |
| Who owns behavior and why was it designed this way? | `docs/architecture/` |
| How do I contribute, test, package, or diagnose? | `docs/development/` |
| How must an automated maintainer operate? | `AGENTS.md` and `docs/agents/` |
| What exact file format must another tool produce? | `docs/reference/` and versioned schemas/examples |
| Why was a durable cross-module decision made? | `docs/decisions/` |
| What did a previous stage or dated audit establish? | `docs/history/`, or an explicitly labeled retained historical record |

Keep the README an introduction with a short start path and links. Do not add internal IPC names, CI incident transcripts, debug flags, migration inventories, PR checklists, or narrow renderer implementation details to it. Put durable explanations in their owning guide, not another appendix labeled with a PR number.

## Audit from source to prose

Read the current branch/PR head and record the source revision. Compare recent requests with the merged implementation. Trace visible labels and actions through the active composition root, not just a component whose name looks familiar. Check capability gates, platform fallbacks, lazy mounting, legacy routes, and persisted state.

For provider claims, inspect the implementation-status gate and actual adapter, not every enum variant. For platform claims, distinguish code presence, successful build, automated runtime qualification, physical-device acceptance, signed packaging, and public distribution. For command claims, inspect `package.json` and the wrapper that runs: frontend build, native hot reload, standalone app, release bundle, and test suite are not interchangeable.

For storage and privacy claims, inspect the actual format writer/reader and ownership exclusions. Do not call a portable deck a full personal backup, a library backup a credential backup, or a LAN sync package a copy of every document artifact. Never promise encryption at rest merely because transport or secrets are encrypted.

## Edit rather than append contradictions

Replace outdated canonical paragraphs. Consolidate duplicate guides into one current owner; preserve an old path as a clearly labeled navigation pointer when it has incoming links. A pointer must not repeat obsolete instructions. Mark historical designs as historical and link the replacement. Do not silently rewrite an accepted historical decision as though a later implementation was its original result.

Keep machine-consumed paths, source inventories, migration names/checksums, and compatibility readers intact unless a coordinated change replaces them. A document named after an old stage can still own a live regression procedure. A historical fixture is not stale merely because it exercises old input.

Update `docs/README.md`, relevant architecture links, and agent entry points in the same coherent commit. New canonical files should be reachable through an index rather than discoverable only from a PR comment.

## Classify cleanup candidates before deleting

Use the [cleanup audit](../development/documentation-and-cleanup-audit.md) to record the candidate, evidence, disposition, and remaining validation.

**Obsolete:** no current runtime, compile-time, dynamic import, test, platform, build, or durable compatibility consumer remains. Remove the source and its exclusively owned styles/tests/inventory entries together. Qualify the removal.

**Intentional compatibility:** a released schema, saved navigation checkpoint, transfer reader, migration, or supported external contract still uses it. Keep the narrow boundary, rationale, and regression coverage. An old UI label alone is not a compatibility requirement; trace the actual durable consumer.

**Live platform fallback:** a capability or target configuration selects it. Keep it unless the replacement covers those targets and unsupported states. Do not delete a stub merely because the primary desktop path no longer shows it.

**Active but awkward ownership:** a large file, facade, unused-looking parameter, or legacy name can still be live. Record a focused improvement proposal with its consumers and tests. Do not call renaming or splitting it dead-code removal. Do not hide uncertainty behind a claim that the entire repository has no stale code.

Repository search is an aid, not proof by itself. Combine the file tree, exact-source reads, import/caller searches, enforcing manifests, and executable gates. A negative indexed search can miss an unindexed or dynamically constructed reference.

## Preserve the workflow while updating workflow docs

[GitHub-direct development](GITHUB-DIRECT-DEVELOPMENT.md) is the canonical agent procedure. Do not copy an old MCP-first passage into a new guide. No manual workflow exception permits Actions source authoring. Do not reintroduce a dependency between the PR acceptance ledger and product qualification. Review readiness is not a fabricated passing test, and source publication is not qualification.

Use [the command runbook](COMMAND-RUNBOOK.md) and [local testing handoff](../development/local-testing-and-handoff.md) when supplying commands. Include exact source identity and data-safety warnings without requiring the human to reproduce checks the agent can already inspect in Actions.

## Validation and evidence

Run `pnpm verify:docs` for required paths, conventional organization, superseded references, and relative links. Run `node ./scripts/verify-agent-workflow-policy.mjs` when workflow guidance or implementation changes. These checks do not verify the truth of every prose claim, internal heading anchors, all external links, or the absence of all dead code. Review those explicitly.

Use static/fast/full and applicable platform qualification for the final published SHA according to repository policy. Record which commands actually ran and which source was checked. A source-linked architecture review is not a full runtime/security audit or new physical-device acceptance. Put transient CI results in the PR evidence, not timeless product claims in the README.
