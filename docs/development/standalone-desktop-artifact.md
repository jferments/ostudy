# Standalone desktop artifact boundary

The Linux development launcher and native WebDriver acceptance tests must execute the same immutable standalone **oStudy Dev** Tauri artifact. That artifact uses `io.github.jferments.ostudy.dev`; production packages use the stable `io.github.jferments.ostudy` identity and are a separate release boundary.

`scripts/build.sh` runs `pnpm tauri build --config src-tauri/tauri.dev.conf.json --debug --no-bundle --ci`, which writes its immediate Cargo output to the ordinary debug target. That path is mutable development/build-tool state: later Cargo, Tauri development, or Android qualification may reuse the same target tree. It is therefore not a durable manual-launch contract.

After every standalone desktop build, `scripts/build-state.sh` copies the just-built executable to `standalone/ostudy` under the configured Cargo target directory. `scripts/check.sh` stages that copy before native startup/WebDriver smoke, and `run-app.sh` launches that same staged copy. The build stamp records both the source fingerprint and the staged executable SHA-256. If either changes, the launcher requires a fresh `scripts/build.sh` or `scripts/check.sh` instead of attempting to launch an ambiguous artifact.

This boundary intentionally allows Android qualification and ordinary development work to continue using mutable Cargo/Tauri outputs without invalidating the exact desktop executable already accepted by native smoke.
