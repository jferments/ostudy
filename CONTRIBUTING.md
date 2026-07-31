# Contributing to ostudy

Before changing code, read:

- `AGENTS.md`
- `docs/ARCHITECTURE.md`
- `docs/DEVELOPMENT-AND-QUALITY.md`
- `docs/GIT-WORKFLOW.md`
- `docs/PHASE-2-LOCAL-TEST-PLAN.md` for Phase 2 behavior
- `docs/agents/README.md`

## Required local gate

```bash
./scripts/check.sh
```

This is the normal automated command. After it passes, launch `./run-app.sh`
and manually exercise the UI path changed by the branch.

Do not submit a change with warnings, modified released migrations, missing
lockfiles, or undocumented command/schema changes.

## Change structure

- Keep each change small and cohesive.
- Add a regression test for every defect.
- Comment non-obvious invariants and ordering decisions.
- Update README, architecture, and ADR documentation in the same change.
- Separate dependency upgrades from feature work whenever practical.
- Preserve accessibility, theme behavior, import compatibility, and stable
  application-data identifiers unless the task explicitly changes them with a
  documented compatibility decision.
- Treat card text, template values, FTS queries, and shared SQLite decks as
  untrusted data.

A useful commit message describes the behavioral result, for example:

```text
fix(cards): preserve unsupported-node error precedence
```
