# Recoverable library upgrade acceptance

Read [library upgrade ownership](../architecture/library-restore.md#startup-library-upgrades),
the [user recovery guide](../user-guide/library-upgrades.md), and
[local testing and handoff](local-testing-and-handoff.md) first. The production
school library is never a fixture. Package installation and library migration are
separate acceptance boundaries. No merge or production cutover is implied here.

## Acceptance contract

| Criterion | Required behavior |
| --- | --- |
| AC-01 | Read-only source admission rejects unsupported/future/forged schemas and unsafe database/journal paths before migration. Existing explicit pre-public recovery remains. |
| AC-02 | A verified coherent SQLite and managed-file checkpoint exists before the first migration write. Copy/IO/space failure never bypasses this requirement. |
| AC-03 | All forward work operates on a private candidate; target admission, integrity, relationships and applicable typed-value preservation pass before activation. |
| AC-04 | The shared versioned journal recovers interrupted activation and rollback, retains previous/failed state, and does not blindly retry abandoned preparations. |
| AC-05 | Startup progress does not bypass readiness. Native recovery requires operation-bound confirmation and distinguishes retry, library restoration and installing an older executable. |
| AC-06 | Available build/schema provenance and retained-copy costs are documented; missing historical identity stays unknown. Production/dev separation, package data ownership and external backup requirements remain. |
| AC-07 | Focused regressions, required exact-head automated gates and disposable manual handoff are provided, without claiming unrun provider, physical-device or installation acceptance. |

Non-goals: automatic application downloads, an apt repository, root-level package
rollback, reverse SQL migrations, automatic checkpoint pruning, a historical
checkpoint Settings browser, new schema/version numbers solely for demonstration,
released migration edits, runner changes, provider changes and real-library tests.

## Automated regression boundaries

The maintained Fast gate includes the upgrade/checkpoint suites. Focused native
reproduction on an already provisioned checkout is:

```bash
bash ./scripts/test-native-contracts.sh \
  startup::upgrade:: \
  startup::upgrade_dialog:: \
  startup::progress:: \
  library_backup::activation::checkpoint::

pnpm exec vitest run \
  src/components/NativeStartupGate.test.tsx \
  src/ipc/libraryUpgrade.test.ts
```

Coverage includes read-only older/future admission, SQLite-full snapshot refusal,
committed WAL preservation, changed private bytes, managed-root drift, equal-count
value loss, rewritten or backfilled review evidence, table changes no preservation
contract lists, releases without a contract, populated schedules/review evidence,
stale recovery choices, post-reconciliation commit refusal, compatible-schema
restoration, committed bookkeeping failures and abrupt child-process termination
during preparation, activation and rollback. Existing
package/current-format/live-restore suites remain required compatibility
evidence. Counts are not a substitute for retained values.

Finish source and documentation review before freezing the terminal candidate.
Then qualify its exact SHA with Static/Fast, full Linux, production Debian package,
Android compile/package and emulator gates through the maintained Check workflow.
A green draft Fast run does not mean full Linux or Android ran. Native progress
unit tests are also exercised by the full Rust suite. Treat visual dialogs,
installed-copy preservation and physical devices as separate manual boundaries.

## Disposable forward-upgrade test

From the exact reviewed source, build the maintained standalone development app:

```bash
./scripts/build.sh
```

Create a new synthetic release-schema-1 library. This script creates a fresh
application directory and refuses an existing one; it never edits a learner
library's schema or copies personal data. Keep the same shell for later commands.

```bash
TEST_ROOT="$(mktemp -d -t ostudy-upgrade-review.XXXXXX)"
export TEST_ROOT
mkdir -p "$TEST_ROOT"/{config,cache,state}

python3 - <<'PY'
import hashlib
import os
import sqlite3
from pathlib import Path

root = Path(os.environ["TEST_ROOT"])
app = root / "data" / "io.github.jferments.ostudy.dev"
app.mkdir(parents=True, exist_ok=False)
sql = Path("src-tauri/schema/0001_release_baseline.sql").read_bytes()
with sqlite3.connect(app / "library.sqlite3") as connection:
    connection.executescript(sql.decode("utf-8"))
    connection.execute(
        "INSERT INTO schema_metadata(key, value) VALUES ('schema_lineage', ?)",
        ("ostudy-release-v1",),
    )
    connection.execute(
        "INSERT INTO schema_migrations(version, name, checksum, applied_at) "
        "VALUES (1, 'release_baseline', ?, 0)",
        (hashlib.sha256(sql).hexdigest(),),
    )
    connection.execute("PRAGMA user_version = 1")
    connection.execute(
        "UPDATE app_settings SET value_json = '27' WHERE key = 'dailyNewLimit'"
    )
print("Synthetic library:", app)
PY

fixture_env() {
  env XDG_DATA_HOME="$TEST_ROOT/data" \
    XDG_CONFIG_HOME="$TEST_ROOT/config" \
    XDG_CACHE_HOME="$TEST_ROOT/cache" \
    XDG_STATE_HOME="$TEST_ROOT/state" "$@"
}
fixture_env ./scripts/ostudy-dev library-status --profile development --verify
fixture_env ./run-app.sh
```

Stop if fixture creation or verification fails. Do not substitute an existing
`.dev` or production path. XDG overrides isolate files, not the OS keyring; do not
enter real API credentials. Close any other process using this test root.

Check that the app opens, the synthetic daily-new-card limit remains 27, and a
committed update notice is dismissible. A tiny fixture may complete before every
phase is visually noticeable; do not add artificial sleeps to product startup.
Close the app and inspect:

```bash
fixture_env ./scripts/ostudy-dev library-status --profile development --verify
find "$TEST_ROOT/data/io.github.jferments.ostudy.dev/restore-transactions" \
  -maxdepth 2 -type f -print
fixture_env ./run-app.sh
```

The library should now be at the current release-v1 schema with integrity/relationships valid.
The verified checkpoint, metadata, commit marker and previous library remain.
A second normal launch should not create another upgrade transaction. Use this
same root for restart checks and retain it until acceptance evidence is recorded.

## Native recovery dialog using a test-only cutpoint

Create another fresh fixture with the previous creation snippet, but do not launch
it yet. This procedure uses the Rust test executable, not a fault-injection switch
in the production app. It terminates preparation after a complete checkpoint and
before candidate migration. Never point it at an ordinary application directory.

```bash
cargo test --manifest-path src-tauri/Cargo.toml --locked --all-features \
  --lib --no-run --message-format=json > "$TEST_ROOT/native-tests.jsonl"

TEST_EXE="$(python3 - "$TEST_ROOT/native-tests.jsonl" <<'PY'
import json
import sys
from pathlib import Path

executables = set()
candidates = []
for line in Path(sys.argv[1]).read_text().splitlines():
    record = json.loads(line)
    if (record.get("reason") != "compiler-artifact"
            or not record.get("profile", {}).get("test")
            or not record.get("executable")):
        continue
    target = record.get("target", {})
    candidates.append((target.get("name"), target.get("kind"), record["executable"]))
    if target.get("name") == "ostudy_lib":
        executables.add(record["executable"])
if len(executables) != 1:
    raise SystemExit(
        f"Expected exactly one ostudy_lib test executable; candidates={candidates!r}"
    )
print(executables.pop())
PY
)"

status=0
env OSTUDY_UPGRADE_PREPARATION_ROOT="$TEST_ROOT/data/io.github.jferments.ostudy.dev" \
  OSTUDY_UPGRADE_PREPARATION_PHASE='Updating a private library copy…' \
  "$TEST_EXE" --exact \
  library_backup::activation::checkpoint::tests::followup::preparation_crash_child \
  --test-threads=1 || status=$?
test "$status" -eq 77 && fixture_env ./run-app.sh
```

The Cargo test target is selected by its stable name `ostudy_lib`, not by a generic
`target.kind == "lib"` assumption: this crate explicitly builds `staticlib`,
`cdylib` and `rlib` forms.

Exit 77 is expected only from this explicitly requested child-process cutpoint.
Any other exit is a failed setup; do not infer that recovery was exercised.
The normal app should now show **Library update stopped**, not silently
repeat migration. Closing must leave the fixture and recovery files available.
**Recovery options → Restore backup…** must show a second destructive
confirmation explaining newer-work loss and separate package installation.
Confirm restoration only for this synthetic fixture; the app then closes.
Reopening with the same newer executable must not immediately upgrade it again.
The **Your library is restored** screen must explain that restoration succeeded,
why the newer app needs a library update, and that **Keep restored library** closes
without changing it. **Update on next launch** is an optional new library update,
not another restoration or a fix for the cause. The **Library update scheduled**
message must say no update has run yet. Reopening after that decision should permit
a fresh transaction; existing saved copies remain.

This minimal fixture exercises the dialogs and their decisions, not preservation
of a populated school library. The automated
`populated_crash_restore_and_explicit_retry_preserve_study_data_and_newer_copy`
regression uses notes, cards, schedules, review events, attribution and managed
bytes. It verifies typed saved values across interruption, restoration and an
explicit retry, including preservation of newer work in the replaced copy.
Do not treat empty-library UI success as that data-preservation result.

This demonstrates library recovery, not an installed application downgrade. A
real previous-package/current-package pair must still be tested in a disposable
installation environment. Do not downgrade a binary while a journal is unresolved.

## Production package and final handoff

Use `./scripts/ostudy-dev package-deb` only when a local package candidate is
needed; it builds/verifies and does not install. The exact-SHA terminal workflow
also retains a qualified package on the self-hosted runner. Test replacement,
removal and reinstall preservation only with disposable production-profile data,
not the school library or its real credentials.

The handoff records exact base/head, last parent, branch/PR, actual named gate
outcomes and run IDs, safe qualification reuse when applicable, and outstanding
manual dialog/package/physical-device checks. Retained checkpoints consume disk;
no cleanup or production cutover is implied. Keep independent `.ostudybackup`
copies even after upgrade acceptance succeeds.
