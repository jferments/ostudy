# PR #106 roadmap — post-#104 stabilization, production baseline, release readiness, and tooling

Status: **active PR #106 qualification and installed-copy/manual acceptance** on top of merged PR #104. This roadmap defines the acceptance boundaries; the current head and pass/fail state belong to PR #106 and exact-SHA Actions evidence. PR #105 is explicitly deferred and is **not** an activation dependency.

This roadmap owns the current post-#104 production-baseline and migration cleanup that was previously assigned to PR #105/LK-13, together with development/production data isolation, contributor tooling, bounded cleanup, packaging safety, and final qualification.

It does **not** implement responsive lessons, TTS/narration, PowerPoint, STT, voice cloning, speech grading, or any other LK-10–LK-12 feature. Those remain preserved in deferred PR #105. Future #105 schema changes extend the production baseline established here through normal reviewed forward migrations.

Activated baseline: PR #104 squash-merged as `6529004ad4e31946df4116f7a0591d6c80d8e586` from accepted source head `bc1e1c2179b80f73496ddd41312fbc8fdc512896`. The merged public schema is version **30**. PR #106 integrates that exact `main` commit as ancestry and must preserve #104's native-owned study-material pipeline rather than reopening document/AI architecture.

## Product result

After this PR, oStudy has a production-safe local installation boundary suitable for important real study data and a separate development boundary safe for ongoing experimentation.

The intended local topology is:

```text
Production oStudy
  application identity: io.github.jferments.ostudy
  production data / managed files / credentials
  known-good installed package
            │
            └── real learner library

Development oStudy
  development identity: io.github.jferments.ostudy.dev
  separate data / managed files / credentials
  launched from the source checkout
            │
            └── disposable development library

Automated tests
  temporary isolated storage
  never production or ordinary development data
```

A source checkout is not allowed to rely on human memory to avoid the production library. Development launch/build wrappers must select the development profile, and a development startup that resolves the production storage boundary must fail closed.

The current schema/migration lineage is audited and, where safe, simplified into a documented production baseline before the learner begins depending on it. Obsolete **development-only** migration/converter/runtime machinery may be archived only after an explicit recovery path and preservation evidence exist. Released production migrations, once established, become compatibility obligations and are not periodically rewritten for aesthetics.

Contributor setup and local testing must not require the repository owner's private self-hosted runner, VM registration, runner-local caches, signing credentials, or GitHub configuration. Repository-owned setup/doctor/check commands define the test contract; CI invokes the same maintained boundaries in a controlled environment.

## Scope and non-goals

### Included

- development versus production application/data/credential isolation;
- explicit guards preventing normal development launchers from using production storage;
- clean contributor setup/doctor/build/run/test tooling;
- post-#104 schema/migration/reader audit;
- one current production baseline and supported upgrade/recovery contract;
- isolated-copy adoption/preservation proof for retained pre-baseline data where required;
- gated archival of obsolete **pre-release development** migration/reader machinery;
- complete backup/restore/reopen verification across current retained data;
- normal uninstall preserving user data;
- explicit destructive purge as a separate confirmed operation;
- proven-dead code/dependency/documentation cleanup;
- package/release contracts required for a trustworthy native Linux install;
- exact-head Linux/Android qualification and manual production/dev isolation acceptance.

### Not included

- LK-10 lessons, LK-11 narration/TTS, LK-12 PowerPoint;
- a second migration-history cleanup when #105 later adds ordinary forward migrations;
- full Debian/Fedora/Arch/Flatpak public distribution matrix;
- final public `install.sh` dispatcher, signed release manifest, Flathub/F-Droid publication, or ostudy.io brochure site;
- automatic merge;
- personal-library upload to GitHub or CI;
- destructive reset as routine troubleshooting;
- weakening tests, permissions, assertions, timeouts, or recovery guarantees to obtain green CI.

The full public installer/distribution work follows this PR and consumes the stable contracts established here.

## Acceptance boundaries

### AC-01 — production baseline and migration/recovery contract

The post-#104 schema and durable payload set has one documented current production baseline. Fresh current libraries and correctly migrated retained libraries are behaviorally equivalent at supported boundaries. Unknown/future/tampered/corrupt inputs fail closed.

Obsolete pre-release development migration/converter/read machinery may leave the ordinary startup/package path only when:

- its live callers and serialized consumers are proven absent;
- retained data that still needs it has an explicit offline recovery/conversion route;
- migration/checksum provenance is preserved rather than rewritten;
- isolated-copy conversion/preservation tests pass;
- normal current startup no longer depends on the archived machinery.

Future released schema changes resume as immutable forward migrations from this baseline.

### AC-02 — production/development/test isolation and safe uninstall

Production, development, and automated-test profiles cannot resolve to the same ordinary storage or credential namespace.

Normal package removal deletes package-owned executable/integration files but preserves learner data. Destructive user-data purge is a separate explicit operation with a precise validated target and strong confirmation.

Reinstalling the same/newer compatible production package reopens the retained production library. Development uninstall/build cleanup cannot delete production data.

### AC-03 — contributor-local development and CI portability

A contributor on a supported development system can obtain the required source toolchain, run environment diagnostics, build/run oStudy Dev, and execute maintained static/fast/full checks without recreating the repository owner's GitHub runner.

Private runner registration, Actions tokens, release signing material, runner-local evidence/caches, and host-specific services are not ordinary developer prerequisites.

CI remains authoritative exact-SHA qualification, but calls repository-owned commands rather than depending on undocumented machine-only behavior.

### AC-04 — release-readiness qualification and installed-copy acceptance

One frozen exact SHA passes the required repository/platform gates. A real production package installs beside the development checkout, and human acceptance verifies:

- production and development libraries are different;
- content created in one is invisible to the other;
- development startup cannot target production storage;
- production uninstall preserves production data;
- production reinstall reopens that data;
- a complete backup can be created and verified;
- no destructive test is performed against the only valuable copy of a learner library.

### AC-05 — crash-safe complete-library activation

Complete-library activation is journaled and recoverable across abrupt termination. SQLite database/WAL/SHM/rollback-journal state, managed files, retained input/previous libraries and process ownership move under one explicit activation owner. Ambiguous commit state is resolved from durable markers rather than guessed from an interrupted reply.

### AC-06 — same-process restore and stale-request exclusion

Normal restore completes in the running native process. The maintenance boundary drains active work, advances the library generation after successful replacement, rejects stale-generation requests, records a durable completion receipt, refreshes the UI, and clears the native process's session-only credential map without deleting saved OS-keyring entries.

### AC-07 — maintained documentation and release-readiness cleanup

Maintained user, architecture, agent, testing, cleanup and release-baseline documentation must describe the implemented contracts rather than historical stage behavior. Documentation verification and release-readiness cleanup checks cover the current owning boundaries; historical plans remain evidence rather than current product specification.

### AC-08 — release-v1 baseline and staged preservation conversion

Fresh 0.1 libraries use the checksum-protected `ostudy-release-v1` schema-1 baseline. Supported `ostudy-public-v1` inputs remain immutable compatibility evidence/readers and are staged, normalized and preservation-verified before atomic lineage conversion. The explicit final-v26 historical recovery route remains outside ordinary startup.

## 106-A — development / production / test profile isolation

Inspect the final post-#104 startup/path/platform/secret boundaries at the exact merged baseline.

Implement an application-owned profile/build contract rather than relying on ad-hoc shell environment variables alone.

Production keeps the stable release identifier:

```text
io.github.jferments.ostudy
```

Development receives a distinct identity such as:

```text
io.github.jferments.ostudy.dev
```

and corresponding distinct Tauri data/config/cache/state and desktop keyring namespaces. The exact naming/config mechanism must preserve the stable production identifier and package compatibility.

Ordinary checkout commands such as development hot reload, standalone development build, and `run-app.sh` must select the development profile. Production package/release commands select production explicitly.

Add a hard development guard: if a development process resolves the production app-data directory or production secret namespace, startup/build launch refuses rather than warning and continuing.

Tests cover path derivation, profile selection, environment overrides used by tests, keyring namespace separation, managed files, backups, and launcher behavior.

Do not migrate or copy the production library automatically merely because a development library exists. Cross-profile transfer is explicit backup/import/recovery behavior.

## 106-B — contributor setup, build, doctor, and local test tooling

Make the supported contributor flow easy to discover and self-contained.

Target experience:

```bash
git clone ...
cd ostudy

./scripts/ostudy-dev setup
./scripts/ostudy-dev doctor
./scripts/ostudy-dev run
```

and maintained validation:

```bash
./scripts/ostudy-dev check-changed
./scripts/ostudy-dev check-static
./scripts/ostudy-dev check-fast
./scripts/ostudy-dev check
```

The precise command surface may reuse existing scripts rather than duplicating them. One wrapper should not conceal multiple contradictory toolchain definitions.

Audit what truly belongs to:

- ordinary contributor setup;
- source builds;
- Android development;
- CI-only runner provisioning;
- release signing/publication.

Developer setup may install/build dependencies needed to compile and test. It must not require release credentials or private infrastructure.

`doctor` should clearly report missing versions/capabilities and distinguish optional platform gates from ordinary frontend/native development.

CI should invoke the same underlying build/test contracts with extra isolation/evidence controls rather than maintaining a parallel private test implementation.

## 106-C — post-#104 schema and durable-data audit

At the exact merged #104 baseline, inventory:

- schema version and immutable migration registry/checksums;
- tables/indexes/triggers/FTS state;
- durable JSON/payload versions;
- current backup/restore and content-transfer formats;
- managed file kinds and integrity metadata;
- startup/adoption/readers/repairs;
- current document, collection, StudyTarget, guide, Tutor project/conversation, question, media, schedule, review, draft, and asset ownership.

Build an evidence-backed issue register. Every proposed deletion/repair names its caller/consumer evidence, invariant impact, smallest regression, and disposition.

Tests include fresh current creation, current no-op reopen, supported forward upgrade, future/unknown/tampered/corrupt refusal, foreign keys, FTS, transactional failure, managed-file preservation, and complete restore/reopen parity.

A newly demonstrated defect is fixed with a small forward migration/repair. Released migration bytes/checksums are never edited to manufacture a cleaner history.

## 106-D — production baseline and gated archival of obsolete development migration machinery

After 106-C defines the real lineage, establish the clean production starting point that future public installations use.

The implementation must distinguish:

```text
historical pre-release development formats
        │
        ├── archive/recovery path where needed
        ▼
════════ production baseline ════════
        │
        ├── immutable future migration N+1
        ├── immutable future migration N+2
        └── ...
```

Do not remove a reader merely because it looks old. Trace runtime/compile/test/package/restore/import consumers and retained-data obligations.

Where old development-only machinery is retired from normal runtime/package paths, preserve:

- source provenance and hashes;
- any required fixture/converter material;
- an explicit offline historical-backup recovery entry point;
- tests proving the current runtime does not accidentally depend on the archive.

If a retained local library requires adoption, test conversion on an isolated copy first. Personal data remains on the user's machine. Record only privacy-safe evidence such as input lineage, converter SHA, preservation digest/counts/hashes, output baseline identity, and successful reopen.

## 106-E — backup, upgrade, uninstall, and learner-data safety

Exercise current complete-library backup/restore across every retained current feature.

Package/uninstall policy:

```text
normal uninstall
  -> remove package-owned program/integration files
  -> preserve learner data and credentials unless platform policy requires otherwise

explicit purge
  -> separate user-invoked action
  -> validate exact oStudy user-data target
  -> show destructive consequence
  -> require strong confirmation
```

System/root package operations must not casually recurse through arbitrary user home paths. Static package policy and built-Debian inspection must reject package ownership or maintainer-script references to the production XDG learner-data directory.

For significant production upgrades, define the safe sequence around backup, supported forward migration, verification, and failure behavior. Do not promise a binary downgrade can safely open a database already migrated by a newer version.

Tests cover install/reinstall/remove semantics at the package boundary where automatable, plus backup/restore and migration fault injection.

## 106-F — proven-dead code, dependencies, documentation, and bounded workflow cleanup

Use a deletion ledger: candidate owner, direct/indirect callers, serialized entry points, platform/packaging fallback need, replacement, exclusive styles/dependencies/tests, regression, disposition.

Remove only code with no current runtime, compile-time, dynamic, test, packaging, platform, or supported compatibility consumer. An awkward live module is refactoring scope, not dead code.

Reconcile canonical documentation for:

- production versus development installs/data;
- contributor setup/build/test;
- current schema/baseline/recovery;
- backup/restore and uninstall;
- Linux/Android packaging status;
- document/collection/guide/Tutor behavior after #104;
- exact CI qualification semantics.

Remove stale lesson/TTS/PPTX claims from current docs if any accidentally present; those remain deferred roadmap material.

Workflow changes are permitted only for demonstrated problems with an owning regression. Preserve source-read-only Actions, exact tested SHA, truthful gate applicability, bounded evidence, and ANCHORLINE recovery behavior.

## 106-G — production package and release boundary required for local use

Before the full public installer project, qualify at least one native production Linux package suitable for the current Ubuntu/Debian user path.

The package must:

- use the production application identity/profile;
- install independently of the source checkout;
- contain runtime rather than developer toolchain requirements;
- preserve desktop/MIME integration;
- coexist with oStudy Dev;
- preserve user data on normal removal;
- reopen retained data after reinstall.

Do not broaden this increment into the complete Fedora/Arch/Flatpak/release-manifest project. Its purpose is to prove the production package/data boundary and make a known-good local school installation possible.

## 106-H — freeze and exact-head terminal qualification

Complete broad source/data/docs/dependency/cleanup review **before** terminal qualification.

Freeze one exact candidate, then run and record the required current gates, including:

- static qualification;
- fast qualification;
- authoritative full Linux qualification;
- Android compile/package;
- Android emulator smoke;
- PR/repository contract checks that apply.

Qualification reuse, if allowed by the current conservative fingerprint, must name the earlier producing SHA and reused tier. Source/tests/scripts/workflows/config/assets/lockfiles or other invalidating paths require fresh applicable execution.

Once expensive gates run, do not reopen broad optional cleanup. Diagnose actual failures; nonblocking late findings become follow-up work.

## 106-I — human production/dev isolation and release-readiness acceptance

Use disposable/synthetic content first.

Required observations:

1. Install the qualified production package.
2. Verify its production application/data identity.
3. Create a small disposable production deck/card and restart.
4. Launch oStudy Dev from the source checkout and verify a separate development library.
5. Create different disposable development content and confirm neither profile sees the other's content.
6. Verify the development guard refuses any attempt to resolve production storage.
7. Create and verify a complete production backup.
8. Remove the production package normally and confirm its user data remains.
9. Reinstall the same compatible production package and confirm the disposable production data reappears.
10. If existing retained development data must be adopted, run only the qualified isolated-copy path before any intentional production import/adoption.
11. Only after these checks should the human begin treating the installed production profile as the protected school library.

Do not use the only copy of valuable school data for destructive/uninstall/migration experiments.

## Human actions by stage

### PR #104 merge handoff — completed

PR #104 is merged at `6529004ad4e31946df4116f7a0591d6c80d8e586`. PR #106 has reconstructed the exact merged boundary and begun 106-A. No further #104 action is required.

### Personal-library lineage gate — completed

The human ran:

```bash
./scripts/ostudy-dev library-status --verify
```

against the current PR branch. Privacy-safe outcome:

- development profile: no library present;
- production profile: `ostudy-public-v1`, schema 30, full migration/repair ledgers current;
- SQLite integrity: `ok`;
- foreign keys: `ok`.

No personal content or database bytes were uploaded or committed. Because no retained valuable library needs final-v26 runtime adoption, 106-D may retire the automatic Rust transition while keeping explicit historical recovery tooling.

### Before production-install acceptance

Use disposable production content for install/uninstall/reinstall tests. Keep the real nursing-school library empty or separately backed up until the isolation and reinstall checks pass.

### After #106 qualifies and is merged

Install the known-good production package, create an external whole-library backup location, and use the production profile for school. Continue development only through oStudy Dev. Keep the production version pinned/frozen unless intentionally upgrading through a qualified release.

### During the later public installer project

The protected production library stays independent of installer-development VMs/test profiles. Installer acceptance uses clean VMs and disposable data. No need to move school data into those environments.

## Delivery sequence

1. **106-A:** production/development/test profile and credential isolation.
2. **106-B:** contributor setup/build/doctor/local-test portability.
3. **106-C:** exact post-#104 schema/durable-data audit and issue register.
4. **106-D:** production baseline + gated archival/recovery of obsolete pre-release migration machinery — implementation complete after the human lineage gate; terminal qualification pending.
5. **106-E:** backup/upgrade/uninstall/data-safety contracts.
6. **106-F:** proven-dead code/dependency/docs and evidence-backed workflow cleanup.
7. **106-G:** qualified production Debian/Ubuntu package boundary for local use.
8. **106-H:** freeze exact candidate and run terminal repository/platform qualification.
9. **106-I:** production/dev isolation, uninstall/reinstall, backup, and local-release human acceptance.
10. **Handoff:** exact evidence, known limits, writer release, and follow-on public installer/distribution scope.

Proposed implementation title after activation: **chore(release): establish production isolation, baseline and release readiness**.
