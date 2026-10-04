# CLAUDE.md

@AGENTS.md

## Claude Code sessions

- **Fresh cloud container:** run `./scripts/setup-ubuntu.sh --no-build` once, in the background while you read code. It installs the system packages, pinned Node, pnpm, Rust, `tauri-driver` and the script linters, and is safe to re-run. Then `export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"` if the shell does not already have it.
- **Direct `cargo test`:** export `OSTUDY_ROOT_DIR="$PWD"` first so the PDFium tests find `src-tauri/resources/native/pdfium/libpdfium.so`. The check scripts set it themselves. Add `--no-fail-fast` when running the whole suite so integration tests still run after a failure.
- **Native tests run headless.** `./scripts/check.sh` and `./scripts/smoke-native.sh` drive the built app under Xvfb with WebDriver. The Android emulator needs KVM, which cloud containers lack, so Android coverage comes from CI.
- **Before pushing:** `./scripts/check-static.sh`. **Before marking a PR ready:** `./scripts/check-fast.sh`, plus `./scripts/check.sh` when native, migration, or WebDriver-covered behavior changed.
- **Branches and PRs:** name branches `claude/<topic>` unless the session assigns one. Open PRs as drafts, fill in the template, and keep the description in step with each push. Commit messages use an imperative subject under 72 characters and a body that explains why.
- **Reading CI:** the `check` job ends with the failure summary, so the last couple of hundred log lines show which tier failed, the failing tests, and the local command to reproduce it. Failures also appear as annotations on the check run. A ready PR's full run takes a while because Linux and Android share one self-hosted runner; do not re-run it to see a result sooner.
