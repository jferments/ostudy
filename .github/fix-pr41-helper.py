from pathlib import Path

# Repair the temporary quality-pass helper's architecture-doc matcher so it
# matches the current PR text (which already includes the provenance-tag rule).
path = Path('.github/pr41-quality-pass.py')
text = path.read_text()
old = "Cloze syntax, tags, duplicate fingerprints, fresh schedules, and transactional deck/card creation. There is no generated-card review queue."
new = "Cloze syntax, subject-matter tags, duplicate fingerprints, fresh schedules, and transactional deck/card creation. There is no generated-card review queue. Document provenance remains in the managed-document/deck relationship and never becomes learner-facing tags such as `PDF` or `AI generated`; defensive normalization strips those legacy provenance labels if a model returns them anyway."
if old not in text:
    raise SystemExit('expected architecture-doc matcher not found in PR41 helper')
path.write_text(text.replace(old, new, 1))

# The production workload API now takes a deterministic per-session selection
# seed. Update the one direct regression-test caller; practice_selection passes
# the real operation ID separately in the main quality patch.
path = Path('src-tauri/src/regression_tests.rs')
text = path.read_text()
old = 'planned_scheduled_cards_for_scope(connection, scope, &settings, &time)?;'
new = 'planned_scheduled_cards_for_scope(connection, scope, &settings, &time, "regression-workload")?;'
if old not in text:
    raise SystemExit('expected workload regression caller not found')
path.write_text(text.replace(old, new, 1))
