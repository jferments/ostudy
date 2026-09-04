# Draft full qualification

Draft pull requests normally run the `fast` tier when their source head changes. This keeps routine iteration bounded while the pull request is still under active development.

When an agent or maintainer needs terminal qualification **without falsely marking an unfinished pull request ready for review**, add the pull-request label:

```text
qualification:full
```

The label does not write source and does not start a workflow by itself. On the next ordinary source synchronization for that draft pull request, `.github/workflows/check.yml` reads the pull-request event payload and selects the existing `all-full` gate. The same read-only, self-hosted `Check` job then executes the complete serialized qualification sequence:

```text
check-static.sh
→ unified runner/KVM preflight
→ locked Rust dependency-graph validation
→ check.sh
→ android-test.sh
→ android-emulator-smoke.sh
```

This mechanism exists for terminal acceptance work where `workflow_dispatch` is unavailable to the active connector. It does **not** turn GitHub Actions into a source editor: application and test source must already have been published directly to the development branch before qualification begins.

Keep the pull request draft while this label is present. Every later source commit will run `all-full`, which is useful while fixing terminal-gate failures because each replacement head is qualified comprehensively. Remove the label after the final exact head has a green full result so ordinary draft iteration returns to the `fast` tier.

A green result qualifies only the exact SHA that the workflow checked out. The pull request description and handoff must record that SHA and the authoritative self-hosted workflow run; never reuse an earlier green result after source changes.
