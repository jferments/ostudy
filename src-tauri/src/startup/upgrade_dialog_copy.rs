//! User-facing copy only. The native recovery owner decides what happened and
//! which actions are safe; this module cannot authorize or perform an update.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DialogCopy {
    pub title: &'static str,
    pub message: &'static str,
}

pub(super) const CLOSE: &str = "Close oStudy";
pub(super) const UPDATE_NEXT_LAUNCH: &str = "Update on next launch";
pub(super) const KEEP_RESTORED: &str = "Keep restored library";

pub(super) const RESTORED: DialogCopy = DialogCopy {
    title: "Your library is restored",
    message: concat!(
        "Your library has been restored from the backup made before the update. ",
        "This version of oStudy needs to update it before you can study.\n\n",
        "Keep restored library: close without changing it. Install the previous ",
        "compatible version of oStudy to use it as it is.\n\n",
        "Update on next launch: try the library update again when you reopen oStudy. ",
        "This is optional. It may fail again unless the original problem has been fixed.",
    ),
};

pub(super) const INTERRUPTED: DialogCopy = DialogCopy {
    title: "Library update stopped",
    message: concat!(
        "oStudy could not finish updating your library. Your saved library copies ",
        "have been kept.\n\n",
        "Recovery options lets you restore the backup from before the update or ",
        "choose to try the library update again. Neither happens until you confirm.\n\n",
        "Close oStudy leaves the library and saved copies as they are.",
    ),
};

pub(super) const NO_BACKUP: DialogCopy = DialogCopy {
    title: "Library update needs attention",
    message: concat!(
        "This library update needs attention. A completed backup ",
        "from this attempt is not available in this dialog.\n\n",
        "Close without updating keeps the existing library files. Check the cause ",
        "before trying again, such as low disk space or an oStudy error.\n\n",
        "Update on next launch makes a new backup attempt when you reopen oStudy. ",
        "The library will not be updated unless that backup is verified.",
    ),
};

pub(super) const UNRESOLVED: DialogCopy = DialogCopy {
    title: "Library recovery is not finished",
    message: concat!(
        "oStudy has not yet confirmed which saved library copy it can safely open. ",
        "It cannot offer an update or restore choice yet.\n\n",
        "Close and reopen oStudy so it can finish checking the interrupted changes. ",
        "If this message appears again, get help with the error before continuing.\n\n",
        "Do not install an older version of oStudy yet. Keep all library and backup files.",
    ),
};

pub(super) const OPTIONS: DialogCopy = DialogCopy {
    title: "Choose how to recover your library",
    message: concat!(
        "Restore backup returns your library to its state before the update. ",
        "It does not reinstall the previous version of oStudy.\n\n",
        "Try update again lets you schedule another library update for the next launch. ",
        "It may fail for the same reason unless the cause has been fixed.\n\n",
        "The next screen explains your choice and asks you to confirm.",
    ),
};

pub(super) const CONFIRM_RESTORE: DialogCopy = DialogCopy {
    title: "Restore your library from the backup?",
    message: concat!(
        "Your library will return to the backup made before the update. Cards, edits ",
        "and study progress added since then will no longer appear in the library you use.\n\n",
        "oStudy will keep a separate copy of the library it replaces, so newer work ",
        "is not deleted from that saved copy.\n\n",
        "After restoring, oStudy will close. To use the restored library without ",
        "updating it, install the previous compatible version of oStudy separately.",
    ),
};

pub(super) const CONFIRM_RETRY: DialogCopy = DialogCopy {
    title: "Try the library update again?",
    message: concat!(
        "This tries the library update again, not the backup restoration. ",
        "It does not repair the cause of the earlier failure.\n\n",
        "Choose Update on next launch only after addressing that cause, such as ",
        "freeing disk space or installing a corrected oStudy version. ",
        "The update starts when you reopen oStudy, not now.\n\n",
        "Previous saved copies will be kept. Close without updating leaves this attempt on hold.",
    ),
};

pub(super) const RESTORE_DONE: DialogCopy = DialogCopy {
    title: "Your library was restored",
    message: concat!(
        "The backup from before the update is now your library. ",
        "The replaced library was saved separately.\n\n",
        "Close oStudy, then install the previous compatible version to use the library ",
        "without updating it. No application version was changed by this restore.\n\n",
        "Opening this newer version will ask what you want to do. ",
        "It will not update the restored library again unless you choose to try again.",
    ),
};

pub(super) const RETRY_SCHEDULED: DialogCopy = DialogCopy {
    title: "Library update scheduled",
    message: concat!(
        "No library update has run yet. Close oStudy now.\n\n",
        "The next time you open oStudy, it will make a new backup and try to update ",
        "your library for this version. Previous saved copies will be kept.\n\n",
        "This does not fix the original problem. Reopen only after its cause has been addressed.",
    ),
};

pub(super) const UNSUPPORTED: DialogCopy = DialogCopy {
    title: "This version cannot open the library",
    message: concat!(
        "oStudy could not safely open this library. No library update was started.\n\n",
        "Close oStudy and use a version that supports this library. ",
        "Keep the library and its backup files; do not reset or delete them.\n\n",
        "The local diagnostic log contains the error needed to investigate.",
    ),
};

pub(super) const PREPARATION_FAILED: DialogCopy = DialogCopy {
    title: "Library update could not start",
    message: concat!(
        "oStudy could not prepare the library update.\n\n",
        "Close oStudy and check the cause before trying again. ",
        "The local diagnostic log contains the error. ",
        "Keep your library and backup files; do not reset or delete them.",
    ),
};

pub(super) const INFORMATION_UNREADABLE: DialogCopy = DialogCopy {
    title: "oStudy cannot verify the recovery information",
    message: concat!(
        "oStudy could not verify the saved information about this library update, ",
        "so it cannot safely offer a restore or another update.\n\n",
        "Close oStudy and get help with the error in the local diagnostic log. ",
        "Keep all library and backup files. Do not reset the library or install ",
        "an older version until recovery has been checked.",
    ),
};

pub(super) const RECOVERY_FAILED: DialogCopy = DialogCopy {
    title: "The recovery step did not finish",
    message: concat!(
        "oStudy could not complete your recovery choice. ",
        "The remaining library and backup files have been kept.\n\n",
        "Close and reopen oStudy so it can check for interrupted changes. ",
        "If this message appears again, get help with the error in the local diagnostic log.\n\n",
        "Do not delete library files or install an older version before recovery finishes.",
    ),
};

pub(super) fn entry(has_backup: bool, restored: bool, unresolved: bool) -> DialogCopy {
    if unresolved {
        UNRESOLVED
    } else if restored {
        RESTORED
    } else if has_backup {
        INTERRUPTED
    } else {
        NO_BACKUP
    }
}

pub(super) fn retry_cancel(restored: bool) -> &'static str {
    if restored {
        KEEP_RESTORED
    } else {
        "Close without updating"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_recovery_overrides_a_restore_marker_or_backup_presence() {
        for has_backup in [false, true] {
            for restored in [false, true] {
                assert_eq!(entry(has_backup, restored, true), UNRESOLVED);
            }
        }
        assert!(UNRESOLVED.message.contains("Do not install an older version"));
        assert!(!UNRESOLVED.message.contains("has been restored"));
    }

    #[test]
    fn a_missing_backup_is_not_presented_as_an_available_restore() {
        assert_eq!(entry(false, false, false), NO_BACKUP);
        assert!(NO_BACKUP.message.contains("not available in this dialog"));
        assert!(NO_BACKUP.message.contains("unless that backup is verified"));
        assert_eq!(entry(true, false, false), INTERRUPTED);
    }

    #[test]
    fn a_successful_restore_explains_why_updating_is_optional_not_recovery_failure() {
        assert_eq!(entry(true, true, false), RESTORED);
        assert_eq!(entry(false, true, false), RESTORED);
        assert!(RESTORED.message.contains("has been restored"));
        assert!(RESTORED.message.contains("before you can study"));
        assert!(RESTORED.message.contains("This is optional"));
        assert!(RESTORED.message.contains("may fail again"));
        assert!(RESTORED.message.contains("previous compatible version"));
        assert_eq!(retry_cancel(true), "Keep restored library");
        assert!(RESTORED.message.contains(retry_cancel(true)));
        assert!(RESTORED.message.contains(UPDATE_NEXT_LAUNCH));
    }

    #[test]
    fn scheduling_an_update_is_not_claimed_as_a_successful_update_or_a_fix() {
        assert!(CONFIRM_RETRY.message.contains("not the backup restoration"));
        assert!(CONFIRM_RETRY.message.contains("not now"));
        assert!(CONFIRM_RETRY.message.contains("freeing disk space"));
        assert!(RETRY_SCHEDULED.message.contains("No library update has run yet"));
        assert!(RETRY_SCHEDULED.message.contains("next time you open"));
        assert!(RETRY_SCHEDULED.message.contains("does not fix the original problem"));
    }

    #[test]
    fn restoration_explains_newer_work_and_does_not_promise_application_downgrade() {
        assert!(CONFIRM_RESTORE.message.contains("study progress added since then"));
        assert!(CONFIRM_RESTORE.message.contains("separate copy"));
        assert!(CONFIRM_RESTORE.message.contains("install the previous compatible version"));
        assert!(RESTORE_DONE.message.contains("No application version was changed"));
        assert!(RESTORE_DONE.message.contains("unless you choose to try again"));
    }

    #[test]
    fn user_facing_recovery_copy_uses_paragraphs_not_storage_implementation_jargon() {
        for copy in [
            RESTORED,
            INTERRUPTED,
            NO_BACKUP,
            UNRESOLVED,
            OPTIONS,
            CONFIRM_RESTORE,
            CONFIRM_RETRY,
            RESTORE_DONE,
            RETRY_SCHEDULED,
            UNSUPPORTED,
            PREPARATION_FAILED,
            INFORMATION_UNREADABLE,
            RECOVERY_FAILED,
        ] {
            assert!(copy.message.contains("\n\n"), "{}", copy.title);
            let text = format!("{} {}", copy.title, copy.message).to_lowercase();
            for jargon in ["checkpoint", "candidate", "executable", "journal", "sql", "schema"] {
                assert!(!text.contains(jargon), "{}: {jargon}", copy.title);
            }
        }
    }
}
