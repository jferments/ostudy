# Contributing

Before changing code, read:

- `docs/ARCHITECTURE.md`
- `docs/DEVELOPMENT-AND-QUALITY.md`
- `docs/GIT-WORKFLOW.md`

## Required local gate

```bash
./scripts/check.sh
```

Do not submit a change with warnings, modified released migrations, missing
lockfiles, or undocumented command/schema changes.

## Change structure

- Keep each change small and cohesive.
- Add a regression test for every defect.
- Comment non-obvious invariants and ordering decisions.
- Update README, architecture, and ADR documentation in the same change.
- Separate dependency upgrades from feature work whenever practical.

A useful commit message describes the behavioral result, for example:

```text
fix(cards): preserve unsupported-node error precedence
```
