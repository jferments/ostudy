# Library updates and recovery

Installing a newer oStudy application and upgrading its library are separate operations. The operating system installs the application. On startup, oStudy checks the selected library and automatically upgrades a supported older schema when necessary. There is no automatic application downloader or configured apt update repository in this workflow.

## Keep school work separate from development

Use the installed production application for the school library and the maintained source-build launcher for development. The normal Linux directories are:

| Profile | Application identity | Default library directory |
| --- | --- | --- |
| Production | `io.github.jferments.ostudy` | `~/.local/share/io.github.jferments.ostudy/` |
| Development | `io.github.jferments.ostudy.dev` | `~/.local/share/io.github.jferments.ostudy.dev/` |

An explicit `XDG_DATA_HOME` changes the parent directory. The development runtime refuses the production identity, and production and development keep separate credential namespaces. Separate source worktrees still share the ordinary development library unless their XDG roots are isolated.

There is no automatic synchronization or merging between these libraries. A deck import transfers content with local schedules; a whole-library restore replaces the destination. Neither is a safe way to combine two independently evolving review histories. See [Backups, imports, and sharing](backups-imports-and-sharing.md).

## Install an application update

Keep a portable `.ostudybackup` outside the application directory, and retain the previous qualified application package with its checksums and build information. Close oStudy before replacing the installed package. Install the selected qualified `.deb` through the package manager, then launch the installed application normally. The package owns program and integration files, not the school library. See [Linux installation](../installation/linux.md).

An application version and a library schema are different identifiers. Installing a new build with the same supported schema does not require a schema migration or create another automatic migration checkpoint. In particular, this upgrade-safety implementation retains `ostudy-release-v1` schema 2; it does not invent schema 3 merely to demonstrate an update. Existing schema-1 libraries exercise the forward upgrade path.

Builds may still share the application version `0.1.0`. Do not identify a candidate or rollback package by that number alone: retain `BUILD-INFO.txt`, the exact source SHA and package checksum. A historical library without an earlier build receipt has an unknown originating build; the new application does not guess it.

## What happens when migration is required

Before ordinary library commands become available, oStudy takes exclusive cooperating-process ownership and inspects schema identity, compatibility and recovery state. A supported older library is copied into a private recovery transaction. The SQLite snapshot includes committed WAL data; managed assets, documents, transfers and cache trees are copied and checked too. Saved operating-system credentials are not read or changed.

The complete recovery checkpoint must verify before any migration is attempted. All required migration work happens on a separate candidate. That candidate must reach the expected schema and pass native admission, integrity, relationship and applicable preservation checks before activation. The original active library is not progressively migrated.

The startup screen reports checkpoint creation, private-copy migration, verification and activation. A late-loading window can recover the current progress message. These messages do not grant database access; normal startup readiness remains independent. After successful activation, reconciliation and commit, a dismissible notice says that the library was updated and its recovery checkpoint retained.

The shared activation journal coordinates the database and its journals with managed files. A failed operation before commit attempts to restore the previous coherent state. If the process is interrupted, the next launch reads the journal before opening the library. A committed operation is not reversed merely because bookkeeping cleanup failed.

## When an update fails

Do not reset the library, delete SQLite sidecars, remove `library-owner.lock`, or repeatedly reinstall packages. Preserve the app data and recovery files. Other programs and historical binaries that ignore oStudy's ownership lock must also be closed.

A failed or interrupted preparation does not silently retry at every launch. The native recovery dialog offers closing the app or explicitly allowing a new attempt on the next launch. When checkpoint metadata is available, **Recovery options** also offers **Restore checkpoint…**. The checkpoint is verified again before restoration; its mere presence is not proof that it is usable.

Restoration requires a second destructive confirmation. It reinstates the old library bytes instead of executing reverse SQL. Any work newer than the checkpoint disappears from the active library, but the replaced library is retained separately in the recovery transaction. oStudy then closes. The restoration does **not** install, remove or downgrade an application package.

To return to an older release, finish library recovery first, close oStudy and install the retained compatible previous package. The newer executable will not immediately re-upgrade an explicitly restored older library without a new decision. An executable whose target schema matches that restored checkpoint can admit it after checking the recovery state. Never install an older binary while an activation journal is unresolved: that binary might not understand the newer journal format.

A future, forged or unsupported schema is refused rather than relabelled. Pre-public libraries still require the existing explicit recovery procedure; this feature does not restore automatic pre-public conversion. If native recovery cannot establish a coherent state, it preserves the files and keeps normal use blocked. Keep the displayed operation context and inspect local diagnostics rather than deleting evidence.

## Retained copies and disk space

Recovery transactions live under `restore-transactions/<operation-id>/` within the selected profile. They contain checkpoint metadata and a verified checkpoint, the migration candidate while it exists, and previous or failed-library entries according to the reached phase. Metadata records source/target lineage and schema, application identity, timestamp, checkpoint hash and available build identities.

These copies are retained, not automatically pruned. Allow space for the checkpoint and candidate in addition to the active library; repeated failures or restoration can retain further copies. If copying, verification or migration runs out of space or encounters an IO error, the operation stops rather than bypassing the checkpoint requirement. Do not free space by deleting unresolved transaction files.

An internal checkpoint on the same disk is not an external disaster-recovery backup, is not automatically encrypted, and is not a portable `.ostudybackup` package. Do not rename its database to make it look like one. Continue making ordinary backups after substantial school work and keep at least one copy outside the application directory and the same physical disk.

The recovery dialog handles an outstanding failed/interrupted update. This release does not add a general Settings browser for all historical checkpoints, automatic checkpoint retention controls, an application rollback installer, or rollback protection for every same-schema application change.

For disposable testing, use the [upgrade acceptance procedure](../development/library-upgrade-acceptance.md). Do not use the school library as an upgrade failure fixture.
