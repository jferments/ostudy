use keyring::v1::{Entry, Error as KeyringError};

use crate::platform::{
    SecretBytes, SecretLocator, SecretStoreAvailability, SecretStoreError, SecureSecretStore,
};

const KEYRING_AVAILABILITY_ACCOUNT: &str = "availability-probe";

pub(super) struct KeyringSecretStore {
    service: String,
}

impl KeyringSecretStore {
    pub(super) fn new(application_identifier: &str) -> Self {
        Self {
            service: application_identifier.to_string(),
        }
    }

    fn service_for_namespace(&self, namespace: &str) -> String {
        format!("{}.{}", self.service, namespace)
    }
}

impl SecureSecretStore for KeyringSecretStore {
    fn availability(&self) -> SecretStoreAvailability {
        let Ok(entry) = Entry::new(&self.service, KEYRING_AVAILABILITY_ACCOUNT) else {
            return SecretStoreAvailability::Unavailable;
        };
        match entry.get_secret() {
            Ok(_) | Err(KeyringError::NoEntry) => SecretStoreAvailability::Available,
            Err(error) => match classify_error(error) {
                SecretStoreError::Locked => SecretStoreAvailability::Locked,
                SecretStoreError::Unavailable
                | SecretStoreError::PermissionDenied
                | SecretStoreError::Invalidated
                | SecretStoreError::Failed => SecretStoreAvailability::Unavailable,
            },
        }
    }

    fn read(&self, locator: &SecretLocator) -> Result<Option<SecretBytes>, SecretStoreError> {
        let entry = self.entry(locator)?;
        match entry.get_secret() {
            Ok(secret) => Ok(Some(SecretBytes::new(secret))),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(classify_error(error)),
        }
    }

    fn write(&self, locator: &SecretLocator, value: &[u8]) -> Result<(), SecretStoreError> {
        self.entry(locator)?.set_secret(value).map_err(classify_error)
    }

    fn delete(&self, locator: &SecretLocator) -> Result<(), SecretStoreError> {
        match self.entry(locator)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(classify_error(error)),
        }
    }
}

impl KeyringSecretStore {
    fn entry(&self, locator: &SecretLocator) -> Result<Entry, SecretStoreError> {
        Entry::new(
            &self.service_for_namespace(locator.namespace),
            &locator.key,
        )
        .map_err(classify_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_and_development_services_are_distinct() {
        let production = KeyringSecretStore::new("io.github.jferments.ostudy");
        let development = KeyringSecretStore::new("io.github.jferments.ostudy.dev");

        assert_ne!(production.service, development.service);
        assert_eq!(
            production.service_for_namespace("ai_provider"),
            "io.github.jferments.ostudy.ai_provider"
        );
        assert_eq!(
            development.service_for_namespace("ai_provider"),
            "io.github.jferments.ostudy.dev.ai_provider"
        );
    }
}

fn classify_error(error: KeyringError) -> SecretStoreError {
    match error {
        KeyringError::NoDefaultStore
        | KeyringError::NoStorageAccess(_)
        | KeyringError::NotSupportedByStore(_) => SecretStoreError::Unavailable,
        _ => SecretStoreError::Failed,
    }
}
