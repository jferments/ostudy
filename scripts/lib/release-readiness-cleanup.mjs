export function verifyReleaseReadinessCleanup({
  audit,
  startup,
  database,
  projectManifests,
}) {
  const failures = [];

  function requireText(label, source, needle) {
    if (!source.includes(needle)) failures.push(`${label} missing ${JSON.stringify(needle)}`);
  }

  function rejectText(label, source, needle) {
    if (source.includes(needle)) {
      failures.push(`${label} still contains retired runtime ownership ${JSON.stringify(needle)}`);
    }
  }

  for (const path of [
    "scripts/restore_pre_public_backup.py",
    "scripts/reset-pre-public-data.sh",
  ]) {
    requireText("cleanup audit", audit, path);
  }
  requireText("retired v26 runtime decision", audit, "retire from runtime");
  rejectText("runtime startup", startup, "pre_public_transition::adopt_final_pre_public_database");
  rejectText("runtime startup module", startup, "mod pre_public_transition");
  requireText("pre-public fail-closed marker", database, "PRE_PUBLIC_RESET_PREFIX");
  requireText("reset helper manifest", projectManifests, '"scripts/reset-pre-public-data.sh"');
  requireText("completed retirement gate", audit, "human lineage gate confirmed");
  requireText("explicit historical recovery", audit, "explicit staged recovery tool");
  requireText("release baseline", audit, "`ostudy-release-v1` / schema 1");
  requireText("migration retention", audit, "public-v1 migrations 1–30");
  requireText("repair retention", audit, "public-v1 repairs 1–3");
  requireText(
    "legacy compatibility disposition",
    audit,
    "retain as compatibility evidence",
  );

  return failures;
}
