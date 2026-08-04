#!/usr/bin/env python3
"""Small follow-up edits applied with the main Stage 3.4/3.5 generator."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
path = ROOT / "src-tauri/src/library_backup.rs"
text = path.read_text(encoding="utf-8")
old = '''impl PendingRestoreGuard {
    pub(crate) fn commit(mut self) -> Result<(), String> {
        if let Some(path) = self.rollback_database.take() {
            remove_file_if_exists(&path).map_err(|error| {
                format!("Could not remove the previous database backup: {error}")
            })?;
        }
        if let Some(path) = self.rollback_assets.take() {
            remove_dir_if_exists(&path).map_err(|error| {
                format!("Could not remove the previous asset backup: {error}")
            })?;
        }
        remove_file_if_exists(&self.pending_package)
            .map_err(|error| format!("Could not remove the completed restore package: {error}"))?;
        remove_dir_if_exists(&self.staging_root)
            .map_err(|error| format!("Could not remove restore staging: {error}"))?;
        self.committed = true;
        Ok(())
    }
}
'''
new = '''impl PendingRestoreGuard {
    pub(crate) fn commit(mut self) {
        // Once the restored database has opened and migrated successfully, a
        // cleanup failure must never invoke Drop's rollback against that live
        // connection. Mark the guard committed first and treat stale rollback
        // files as conservative cleanup residue for the next maintenance pass.
        self.committed = true;
        if let Some(path) = self.rollback_database.take() {
            let _ = remove_file_if_exists(&path);
        }
        if let Some(path) = self.rollback_assets.take() {
            let _ = remove_dir_if_exists(&path);
        }
        let _ = remove_file_if_exists(&self.pending_package);
        let _ = remove_dir_if_exists(&self.staging_root);
    }
}
'''
if text.count(old) != 1:
    raise SystemExit("pending restore commit boundary did not match")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
print("Applied backup restore guard follow-up.")
