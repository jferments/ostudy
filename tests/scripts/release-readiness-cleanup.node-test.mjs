import assert from "node:assert/strict";
import test from "node:test";

import { verifyReleaseReadinessCleanup } from "../../scripts/lib/release-readiness-cleanup.mjs";

function fixture(overrides = {}) {
  return {
    audit: [
      "retire from runtime",
      "human lineage gate confirmed",
      "explicit staged recovery tool",
      "`ostudy-release-v1` / schema 1",
      "public-v1 migrations 1–30",
      "public-v1 repairs 1–3",
      "retain as compatibility evidence",
      "scripts/restore_pre_public_backup.py",
      "scripts/reset-pre-public-data.sh",
    ].join("\n"),
    startup: "fn initialize_current_release() {}",
    database: "pub(crate) const PRE_PUBLIC_RESET_PREFIX: &str = \"PRE_PUBLIC_DATABASE_REQUIRES_RESET\";",
    projectManifests: 'const required = ["scripts/reset-pre-public-data.sh"];',
    ...overrides,
  };
}

test("release cleanup verification protects technical recovery and lineage contracts", () => {
  assert.deepEqual(verifyReleaseReadinessCleanup(fixture()), []);

  const missingResetMarker = fixture({ database: "pub struct Database;" });
  assert.match(
    verifyReleaseReadinessCleanup(missingResetMarker).join("\n"),
    /pre-public fail-closed marker/u,
  );

  const restoredRuntimeAdoption = fixture({
    startup: "pre_public_transition::adopt_final_pre_public_database();",
  });
  assert.match(
    verifyReleaseReadinessCleanup(restoredRuntimeAdoption).join("\n"),
    /retired runtime ownership/u,
  );
});

test("pull-request status prose is not part of static release-readiness qualification", () => {
  const value = fixture();
  value.audit += [
    "",
    "PR #106 is active.",
    "PR #106 is merged.",
    "PR #105 is intentionally deferred.",
  ].join("\n");
  assert.deepEqual(verifyReleaseReadinessCleanup(value), []);
});
