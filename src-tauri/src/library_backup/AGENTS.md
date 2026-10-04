# Library backup and activation

Follow the repository instructions and the [rules for changing backup and restore code](../../../docs/architecture/library-restore.md#rules-for-changing-backup-and-restore-code).

The package, journaled activation and live-runtime owners must remain distinct.
Never replace a live database without draining its consumers, detach a required
WAL, delete unknown recovery copies, or equate a prepared input with completion.
The source backup and previous coherent library are retained through verification.
Use current-format preservation, interrupted-process recovery and live-generation
tests; user libraries remain local and are never CI fixtures.
