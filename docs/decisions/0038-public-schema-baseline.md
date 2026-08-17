# ADR 0038: Public database schema baseline

## Decision

The first supported public schema lineage is `ostudy-public-v1`, represented by the immutable migration `0001_public_baseline.sql`. The final pre-release schema was built from all development migrations, validated, converted into canonical DDL plus seed rows, and checked against a committed semantic schema manifest.

Databases with development-era objects but no public lineage marker are rejected without writes. The explicit reset helper moves the complete application-data directory to a timestamped backup. Future migrations begin at version 2 and are append-only and checksum-protected.

Historical repair results are folded into the baseline. The independent repair framework remains with an empty public registry, ready for future versioned repairs.
