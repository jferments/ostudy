use keyring::v1::{Entry, Error as KeyringError};

use crate::platform::{
    SecretBytes, SecretLocator, SecretStoreAvailability, SecretStoreError, SecureSecretStore,
};

const KEYRING_SERVICE: &str = "io.github.jferments.ostudy";
const KEYRING_AVAILABILITY_ACCOUNT: &str = "availability-probe";

pub(super) struct KeyringSecretStore;

impl SecureSecretStore for KeyringSecretStore {
    fn availability(&self) -> SecretStoreAvailability {
        let Ok(entry) = Entry::new(KEYRING_SERVICE, KEYRING_AVAILABILITY_ACCOUNT) else {
            return SecretStoreAvailability::Unavailable;
        };
        match entry.get_secret() {
            Ok(_) | Err(KeyringError::NoEntry) => SecretStoreAvailability::Available,
            Err(error) => match classify_error(error) {
                SecretStoreError::Unavailable | SecretStoreError::Locked => {
                    SecretStoreAvailability::Unavailable
                }
                SecretStoreError::Failed => SecretStoreAvailability::Unavailable,
            },
        }
    }

    fn read(&self, locator: &SecretLocator) -> Result<Option<SecretBytes>, SecretStoreError> {
        let entry = entry(locator)?;
        match entry.get_secret() {
            Ok(secret) => Ok(Some(SecretBytes::new(secret))),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(classify_error(error)),
        }
    }

    fn write(&self, locator: &SecretLocator, value: &[u8]) -> Result<(), SecretStoreError> {
        entry(locator)?.set_secret(value).map_err(classify_error)
    }

    fn delete(&self, locator: &SecretLocator) -> Result<(), SecretStoreError> {
        match entry(locator)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(classify_error(error)),
        }
    }
}

fn entry(locator: &SecretLocator) -> Result<Entry, SecretStoreError> {
    Entry::new(
        &format!("{KEYRING_SERVICE}.{}", locator.namespace),
        &locator.key,
    )
    .map_err(classify_error)
}

fn classify_error(error: KeyringError) -> SecretStoreError {
    match error {
        KeyringError::NoDefaultStore
        | KeyringError::NoStorageAccess(_)
        | KeyringError::NotSupportedByStore(_) => SecretStoreError::Unavailable,
        _ => SecretStoreError::Failed,
    }
}
