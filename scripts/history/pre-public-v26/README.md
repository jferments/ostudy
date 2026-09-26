# Historical final-v26 recovery

This directory contains the immutable contract for oStudy's last developer-only
database lineage before `ostudy-public-v1`.

- Historical cutoff source: `61b50d91170ce8bd00653af9f7e5f1cec686341f`.
- The contract accepts only the exact 26 migration identities, 3 repair identities,
  and SQLite markers 0 or 26 recorded in `ledger.json`.
- Normal oStudy startup no longer compiles or executes the v26 converter.
- Explicit recovery is available through `scripts/transition-pre-public-data.sh`.
- Recovery copies the source before SQLite opens it, converts only a staging copy
  to public baseline migration 1, validates preservation/integrity, archives the
  previous active directory, and leaves the historical source untouched.
- Current oStudy then advances the recovered `ostudy-public-v1` copy through its
  ordinary immutable migration/repair chain.

Do not register this historical ledger as a current migration source.
