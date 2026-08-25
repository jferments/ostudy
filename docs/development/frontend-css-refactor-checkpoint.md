# Frontend and CSS refactor checkpoint

This document is the durable repository checkpoint for the restarted frontend and CSS ownership refactor tracked by pull request #75.

## Verified repository state

- Pull request: `#75` (`refactor(ui): complete explicit frontend and CSS ownership`)
- Canonical work branch: `refactor/pr75-css-ui-restart`
- Pull-request compatibility branch: `agent/frontend-css-ownership-refactor-restart`
- Target branch: `main`
- Exact pre-PR-#74 base: `f088c401235419535a9fd36e76badbc004509255`
- PR #74 roadmap: `docs/development/frontend-css-refactor-roadmap.md`
- Accidental `test-do-not-create` branch: absent when this checkpoint was established
- Copilot assignment on PR #75: absent when this checkpoint was established

GitHub does not support replacing the head branch of an existing pull request. PR #75 was originally opened from `agent/frontend-css-ownership-refactor-restart`, so that ref remains attached to the pull request. The canonical branch requested for the restart is `refactor/pr75-css-ui-restart`. During this refactor, both refs must point to the same commit after every stage; a mismatch is a release-blocking error.

## Ancestry invariant

The refactor must remain a linear descendant of the exact state immediately before PR #74:

```text
f088c401235419535a9fd36e76badbc004509255
└── PR #75 bounded stage commits
```

The first PR #75 commit, `0e65e79083ac4267d995d2da5afad137952e22be`, has `f088c401235419535a9fd36e76badbc004509255` as its sole parent. Before this checkpoint was added, comparison of the verified base with PR #75 head `403e37c3a7af8fbd04d85f0c63d75b67b77c2fdc` reported:

- merge base: `f088c401235419535a9fd36e76badbc004509255`
- ahead: 28 commits
- behind: 0 commits

No PR #74 implementation commit is an ancestor of PR #75.

## Source checkpoints

Historical exact-source artifacts were produced for intermediate heads, including:

- `frontend-refactor-source-01c348843e8c9277ddde90da8fe53d2ec7c6f952`
- `frontend-refactor-source-ddeeaf1a68c6d22310dff1ace29cc7cb3c90fdca`

The workflow `.github/workflows/frontend-refactor-source-checkpoint.yml` creates a short-lived exact-source artifact whenever this checkpoint document or the workflow itself changes. The artifact is a recovery mechanism, not a substitute for commits.

## Roadmap execution ledger

The exact roadmap is authoritative. This ledger records implementation and fresh validation separately so inherited code is not treated as qualified merely because it exists.

| Stage | Scope | Implementation | Fresh focused validation |
| --- | --- | --- | --- |
| 0 | Inventory, characterization, and ratchet | inherited; checkpoint corrected here | pending |
| 1 | Pure deck-library organization model | inherited | pending |
| 2 | Deck-library controller | inherited | pending |
| 3 | Unified reorder and deck-library view decomposition | inherited | pending |
| 4 | Foundations and application shell | inherited | pending |
| 5 | Shared controls, feedback, and overlays | inherited | pending |
| 6 | Rich editor and rendered-content ownership | inherited | pending |
| 7 | Feature-by-feature global CSS retirement | pending | pending |
| 8 | Delete legacy CSS and normalize the cascade | pending | pending |

A stage is complete only after its focused tests pass against the exact stage head. The final head additionally requires the repository’s full validation suite and exact-head CI evidence. Manual visual and assistive-technology checks remain explicitly recorded when they cannot be automated.

## Commit discipline

1. Make one bounded architectural stage or feature-family migration per commit.
2. Run and record focused tests against that exact commit.
3. Advance both branch refs to the same commit.
4. Do not use temporary source-export, patch-applicator, or migration-runner files as implementation state.
5. Run full validation only after all focused stage checks are green.
