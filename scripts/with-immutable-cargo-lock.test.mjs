import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const wrapper = join(scriptDirectory, "with-immutable-cargo-lock.mjs");

function createFixture({ initializeGit = false } = {}) {
  const root = mkdtempSync(join(tmpdir(), "ostudy-lock-test-"));
  const cargoDirectory = join(root, "src-tauri");
  const binDirectory = join(root, "bin");
  mkdirSync(cargoDirectory, { recursive: true });
  mkdirSync(binDirectory, { recursive: true });
  writeFileSync(join(cargoDirectory, "Cargo.toml"), "[package]\nname='fixture'\nversion='0.1.0'\n");
  writeFileSync(join(cargoDirectory, "Cargo.lock"), "committed lock bytes\n");

  if (initializeGit) {
    for (const args of [
      ["init", "-q"],
      ["config", "user.email", "tests@ostudy.invalid"],
      ["config", "user.name", "ostudy tests"],
      ["add", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock"],
      ["commit", "-qm", "fixture"],
    ]) {
      const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
      assert.equal(result.status, 0, result.stderr);
    }
  }

  const fakeCargo = join(binDirectory, "cargo");
  writeFileSync(fakeCargo, "#!/usr/bin/env bash\nexit 0\n");
  chmodSync(fakeCargo, 0o755);

  const mutate = join(binDirectory, "mutate-lock");
  writeFileSync(
    mutate,
    "#!/usr/bin/env bash\nprintf 'changed lock bytes\\n' > \"$OSTUDY_CARGO_LOCK_ROOT/src-tauri/Cargo.lock\"\n",
  );
  chmodSync(mutate, 0o755);

  const remove = join(binDirectory, "remove-lock");
  writeFileSync(
    remove,
    "#!/usr/bin/env bash\nrm -f -- \"$OSTUDY_CARGO_LOCK_ROOT/src-tauri/Cargo.lock\"\n",
  );
  chmodSync(remove, 0o755);

  const succeed = join(binDirectory, "leave-lock-alone");
  writeFileSync(succeed, "#!/usr/bin/env bash\nexit 0\n");
  chmodSync(succeed, 0o755);

  return { root, binDirectory, lockPath: join(cargoDirectory, "Cargo.lock"), mutate, remove, succeed };
}

function runFixture(fixture, command) {
  return spawnSync(process.execPath, [wrapper, command], {
    cwd: fixture.root,
    encoding: "utf8",
    env: {
      ...process.env,
      OSTUDY_CARGO_LOCK_ROOT: fixture.root,
      OSTUDY_CARGO_COMMAND: join(fixture.binDirectory, "cargo"),
      PATH: `${fixture.binDirectory}:${process.env.PATH ?? ""}`,
    },
  });
}


test("rejects a pre-existing unstaged Cargo.lock change with recovery instructions", () => {
  const fixture = createFixture({ initializeGit: true });
  try {
    writeFileSync(fixture.lockPath, "accidental setup mutation\n");
    const result = runFixture(fixture, fixture.succeed);
    assert.equal(result.status, 65, result.stderr);
    assert.match(result.stderr, /has unstaged changes before the build/u);
    assert.match(result.stderr, /git restore --worktree src-tauri\/Cargo\.lock/u);
    assert.equal(readFileSync(fixture.lockPath, "utf8"), "accidental setup mutation\n");
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("allows a build command that leaves Cargo.lock unchanged", () => {
  const fixture = createFixture();
  try {
    const result = runFixture(fixture, fixture.succeed);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(readFileSync(fixture.lockPath, "utf8"), "committed lock bytes\n");
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("restores and rejects a build that changes Cargo.lock", () => {
  const fixture = createFixture();
  try {
    const result = runFixture(fixture, fixture.mutate);
    assert.equal(result.status, 67, result.stderr);
    assert.match(result.stderr, /attempted to modify src-tauri\/Cargo\.lock/u);
    assert.equal(readFileSync(fixture.lockPath, "utf8"), "committed lock bytes\n");
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("restores and rejects a build that deletes Cargo.lock", () => {
  const fixture = createFixture();
  try {
    const result = runFixture(fixture, fixture.remove);
    assert.equal(result.status, 67, result.stderr);
    assert.equal(readFileSync(fixture.lockPath, "utf8"), "committed lock bytes\n");
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});
