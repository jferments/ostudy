use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct PreservedSecureStore {
    reads: AtomicUsize,
}

impl SecureSecretStore for PreservedSecureStore {
    fn availability(&self) -> SecretStoreAvailability {
        panic!("session reset must not query OS credential availability");
    }

    fn read(&self, _locator: &SecretLocator) -> Result<Option<SecretBytes>, SecretStoreError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(Some(SecretBytes::new(b"saved-credential".to_vec())))
    }

    fn write(&self, _locator: &SecretLocator, _value: &[u8]) -> Result<(), SecretStoreError> {
        panic!("session reset must not write saved credentials");
    }

    fn delete(&self, _locator: &SecretLocator) -> Result<(), SecretStoreError> {
        panic!("session reset must not delete saved credentials");
    }
}

#[test]
fn library_restore_clears_every_session_entry_without_database_or_os_access() {
    let secure = Arc::new(PreservedSecureStore::default());
    let manager = SecretManager::new(secure.clone());
    let ids = ["1".repeat(32), "2".repeat(32)];
    // No database exists. Cleanup must also cover entries whose profile rows
    // were deleted or never appeared in the newly restored library.
    for id in &ids {
        manager.insert_session(id, b"old-session".to_vec()).unwrap();
        assert!(manager
            .resolve(id, AuthMode::Bearer, CredentialStorage::Session)
            .is_ok());
    }
    manager.clear_session_credentials().unwrap();
    manager.clear_session_credentials().unwrap();
    assert_eq!(secure.reads.load(Ordering::SeqCst), 0);
    for id in &ids {
        let error = manager
            .resolve(id, AuthMode::Bearer, CredentialStorage::Session)
            .unwrap_err();
        assert_eq!(error.code, "ai_credential_required");
    }
    assert!(manager.session.lock().unwrap().is_empty());
}

#[test]
fn library_restore_expires_session_override_but_preserves_saved_credential() {
    let secure = Arc::new(PreservedSecureStore::default());
    let manager = SecretManager::new(secure.clone());
    let id = "3".repeat(32);
    manager.insert_session(&id, b"session-override".to_vec()).unwrap();
    assert_eq!(
        manager
            .resolve(&id, AuthMode::Bearer, CredentialStorage::Secure)
            .unwrap()
            .unwrap()
            .as_bytes(),
        b"session-override",
    );
    manager.clear_session_credentials().unwrap();
    assert_eq!(secure.reads.load(Ordering::SeqCst), 0);
    assert_eq!(
        manager
            .resolve(&id, AuthMode::Bearer, CredentialStorage::Secure)
            .unwrap()
            .unwrap()
            .as_bytes(),
        b"saved-credential",
    );
    assert_eq!(secure.reads.load(Ordering::SeqCst), 1);
}

#[test]
fn library_restore_reports_poisoned_session_instead_of_claiming_cleanup() {
    let secure = Arc::new(PreservedSecureStore::default());
    let manager = SecretManager::new(secure.clone());
    manager.insert_session("orphan", b"retained".to_vec()).unwrap();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _session = manager.session.lock().unwrap();
        panic!("injected credential mutex poison");
    }));
    assert!(panic.is_err());
    assert_eq!(
        manager.clear_session_credentials().unwrap_err().code,
        "ai_secret_lock_poisoned",
    );
    assert_eq!(secure.reads.load(Ordering::SeqCst), 0);
}
