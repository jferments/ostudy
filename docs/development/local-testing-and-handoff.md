# Local testing and agent handoff

A useful handoff lets the maintainer obtain the intended source, build the actual native app, test the changed behavior, and distinguish that test-drive from automated merge qualification. It must not bury those steps beneath infrastructure diagnostics.

This guide is shared by human contributors and agents. The [command runbook](../agents/COMMAND-RUNBOOK.md) is the short command reference; [GitHub-direct development](../agents/GITHUB-DIRECT-DEVELOPMENT.md) defines agent publication.

## What every handoff must state

State the repository, PR number/link and draft/ready state, branch, exact verified head, and base. Summarize what changed and what did not. Name actual checks and their tested SHAs, not merely commands that could be run. Explain any unrun checks, known failures, and manual boundaries. Give the local test-drive path followed by a small acceptance checklist for the task.

Also record the last expected publication parent and release of source-writer ownership. In GitHub-only mode say no execution workspace was used. Do not invent a clean worktree, PID, VM state, or claim that no unrelated repository jobs exist. Report only jobs actually inspected.

“Published,” “automatically qualified,” “manually accepted,” and “merged” are separate facts. An older green run does not qualify the current PR head. A displayed command is not a test result.

## Obtain the exact source safely

Use the user's existing authenticated repository checkout and `origin`. Agent repository operations use the Connector; these local Git commands are for the human's machine. Do not ask for `gh` or a new OAuth flow to test an already accessible repository.

Before switching an existing checkout, inspect local work:

```bash
git status --short
git fetch origin --prune
```

Do not discard unrelated edits. A detached worktree is a convenient way to inspect a PR without disturbing them. Replace all three values with the concrete values from the handoff:

```bash
PR_BRANCH='agent/REPLACE_WITH_BRANCH'
EXPECTED_SHA='REPLACE_WITH_FULL_40_CHARACTER_HEAD'
REVIEW_DIR='../ostudy-review-REPLACE_WITH_PR_NUMBER'

git fetch origin "refs/heads/$PR_BRANCH" &&
  test "$(git rev-parse FETCH_HEAD)" = "$EXPECTED_SHA" &&
  git worktree add --detach "$REVIEW_DIR" "$EXPECTED_SHA" &&
  cd "$REVIEW_DIR" &&
  git rev-parse HEAD
```

Stop if any step fails. A mismatched fetched head means the branch moved since the handoff; obtain the new head and its qualification state rather than silently testing different code. A pre-existing worktree directory should be inspected, not deleted automatically. Detached HEAD is intentional for read-only review; create a deliberate local branch before authoring fixes and coordinate with the current branch writer.

For routine updates to an existing clean local task branch, fetch, switch to that branch, and use `git pull --ff-only`. Do not recommend `reset --hard`, automatic stashing, force-pushing, or deleting a local branch to make checkout convenient. A squash merge may leave local ancestry that prevents safe `branch -d`; inspect it rather than substituting force deletion.

## Linux: ordinary test-drive

On an already configured machine, from the selected checkout:

```bash
./scripts/build.sh
./run-app.sh
```

On a genuinely unconfigured Ubuntu/Debian machine, use `./scripts/setup-ubuntu.sh` first. Do not repeat machine provisioning for every PR. Platform dependencies and optional OCR setup are in [Linux installation](../installation/linux.md).

The build wrapper performs pinned-environment checks, provisions PDFium, installs locked JavaScript dependencies, runs compile preflight, builds through Tauri, and stages a standalone debug executable. It does not create release bundles. `run-app.sh` verifies a build stamp and binary checksum before launching that staged executable. A source-only documentation change may not alter the runtime fingerprint, but source deletions and code changes can require a rebuild; follow the launcher's check.

Do not launch the mutable Cargo debug binary as a workaround. Later Tauri dev, native checks, or Android work can reuse that Cargo location. The staged standalone artifact exists specifically to keep the tested/launched executable stable.

`pnpm desktop` is the native hot-reload development path. `pnpm dev`/`pnpm build` are frontend commands and do not prove a working native application.

## Linux: isolated fresh-library tests

A code checkout does not isolate application data. A new worktree can still open the same local library. Back up before testing migrations, destructive actions, or restores.

For an isolated local-library UI test, keep the normal runtime/session environment and override the XDG data/config/cache/state roots for the application launch, as the repository's native smoke tests do:

```bash
TEST_ROOT="$(mktemp -d -t ostudy-review.XXXXXX)"
mkdir -p "$TEST_ROOT"/{data,config,cache,state}
printf 'Isolated test data: %s\n' "$TEST_ROOT"
XDG_DATA_HOME="$TEST_ROOT/data" \
XDG_CONFIG_HOME="$TEST_ROOT/config" \
XDG_CACHE_HOME="$TEST_ROOT/cache" \
XDG_STATE_HOME="$TEST_ROOT/state" \
  ./run-app.sh
```

Keep that directory for restart tests and rerun with the same four overrides. Do not delete it until evidence and any desired data have been preserved. This isolates file-backed library/presentation state, not the machine's operating-system credential service; do not use real API credentials for a disposable UI test or claim that XDG overrides constitute a security sandbox. Do not run two instances against the same test library.

## Android: normal physical-device acceptance

After the [Android setup](../installation/android.md), connect and authorize the device:

```bash
adb devices -l
pnpm android:doctor
pnpm android:install
```

The normal install path builds and verifies the ARM64 debug APK, installs non-streaming with replacement, and launches the debug app. With multiple devices, use the maintained serial-aware wrapper:

```bash
bash ./scripts/android-install.sh --serial 'REPLACE_WITH_ADB_SERIAL'
```

Use an explicit target supported by that wrapper when testing another ABI. A no-build install is useful only when the existing APK's provenance is known; the wrapper selects by modification time, not by Git SHA. It is not a guarantee that a current checkout was installed.

Debug uses `io.github.jferments.ostudy.dev`; release uses `io.github.jferments.ostudy`. Their libraries are separate. A replacement install preserves existing data and is not a fresh-install test. Use a disposable emulator/device profile for fresh-data acceptance. Uninstalling or clearing app storage can destroy its library: do not prescribe either silently. Debug/release signatures and package IDs are not interchangeable, and a signature mismatch is not permission to uninstall without a backup.

Use `pnpm android:dev` for deliberate live hot reload, not ordinary acceptance. Use `pnpm android:diagnostics` when collecting app-private debug diagnostics.

## Validation versus manual acceptance

Focused documentation checks:

```bash
pnpm verify:docs
node ./scripts/verify-agent-workflow-policy.mjs
```

Normal tiers:

```bash
./scripts/check-static.sh
./scripts/check-fast.sh
./scripts/check.sh
```

Android qualification:

```bash
pnpm android:test
pnpm android:smoke
```

Do not require every command merely for a human to look at a UI change. The agent should consume available exact-head Actions evidence and supply the small remaining human checklist. Conversely, “the app launched” does not replace these gates. [Testing and CI](testing-and-ci.md) explains the boundaries and cache/evidence behavior.

For UI work, acceptance should identify concrete initial state, action, and expected result. Cover the changed empty/populated/error paths, keyboard/touch interaction, compact width, and navigation/dirty-state behavior. For data work, include restart and appropriate isolated backup/import/restore checks. For a documentation-only revision, reviewing rendered Markdown and checking that commands/labels match the application is the relevant manual boundary; do not claim new physical-device acceptance.

## Failure reports and privacy

Provide exact SHA, command, OS/device/build mode, the first failing boundary, and minimal reproduction. Prefer the repository's bounded diagnostics over enormous terminal dumps. Distinguish a UI defect, compiler/test failure, missing dependency, runner fault, and Connector problem. Do not ask for passwords, API keys, raw private source documents, or a live library database when a synthetic fixture or redacted log suffices.

## Returning to main

After the human merges, fetch and update a clean main checkout with `git pull --ff-only`. Rebuild when runtime source changed. Remove a review worktree only after checking it for local edits and preserving evidence. No merge, branch deletion, tag, release publication, or destructive reset is implied by a request to provide testing instructions.
