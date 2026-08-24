use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};

use crate::{
    error::CommandError,
    logging,
    platform::{
        SecretBytes, SecretLocator, SecretStoreAvailability, SecretStoreError, SecureSecretStore,
        UnavailableSecretStore,
    },
};

use super::catalog::AuthMode;

const MAX_CREDENTIAL_BYTES: usize = 8192;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialPersistence {
    Secure,
    Session,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CredentialStorage {
    None,
    Secure,
    Session,
}

impl CredentialStorage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Secure => "secure",
            Self::Session => "session",
        }
    }

    pub(crate) fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "none" => Ok(Self::None),
            "secure" => Ok(Self::Secure),
            "session" => Ok(Self::Session),
            _ => Err(CommandError::new(
                "invalid_ai_credential_storage",
                "The saved AI credential-storage state is invalid.",
            )),
        }
    }
}

impl From<CredentialPersistence> for CredentialStorage {
    fn from(value: CredentialPersistence) -> Self {
        match value {
            CredentialPersistence::Secure => Self::Secure,
            CredentialPersistence::Session => Self::Session,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStatus {
    NotRequired,
    Missing,
    SavedSecurely,
    SessionOnly,
    SecureStoreUnavailable,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatusDto {
    pub status: CredentialStatus,
    pub authentication_required: bool,
    pub secure_persistence_available: bool,
    pub session_only_available: bool,
}

pub type SecretValue = SecretBytes;
pub(crate) type ResolvedCredential = SecretValue;

pub struct SecretManager {
    secure: Arc<dyn SecureSecretStore>,
    session: Mutex<HashMap<String, SecretValue>>,
}

impl Default for SecretManager {
    fn default() -> Self {
        Self::new(Arc::new(UnavailableSecretStore))
    }
}

impl SecretManager {
    pub fn new(secure: Arc<dyn SecureSecretStore>) -> Self {
        Self {
            secure,
            session: Mutex::new(HashMap::new()),
        }
    }

    pub fn status(
        &self,
        profile_id: &str,
        auth_mode: AuthMode,
        storage: CredentialStorage,
    ) -> Result<CredentialStatusDto, CommandError> {
        let secure_available = self.secure.availability() == SecretStoreAvailability::Available;
        if !auth_mode.requires_credential() {
            return Ok(CredentialStatusDto {
                status: CredentialStatus::NotRequired,
                authentication_required: false,
                secure_persistence_available: secure_available,
                session_only_available: false,
            });
        }

        match storage {
            CredentialStorage::None => Ok(CredentialStatusDto {
                status: CredentialStatus::Missing,
                authentication_required: true,
                secure_persistence_available: secure_available,
                session_only_available: true,
            }),
            CredentialStorage::Session => Ok(CredentialStatusDto {
                status: if self.session_contains(profile_id)? {
                    CredentialStatus::SessionOnly
                } else {
                    CredentialStatus::Missing
                },
                authentication_required: true,
                secure_persistence_available: secure_available,
                session_only_available: true,
            }),
            CredentialStorage::Secure => {
                match self.secure.read(&SecretLocator::ai_profile(profile_id)) {
                    Ok(Some(_secret)) => Ok(CredentialStatusDto {
                        status: CredentialStatus::SavedSecurely,
                        authentication_required: true,
                        secure_persistence_available: true,
                        session_only_available: true,
                    }),
                    Ok(None) => Ok(CredentialStatusDto {
                        status: CredentialStatus::Missing,
                        authentication_required: true,
                        secure_persistence_available: secure_available,
                        session_only_available: true,
                    }),
                    Err(SecretStoreError::Unavailable | SecretStoreError::Locked) => {
                        Ok(CredentialStatusDto {
                            status: CredentialStatus::SecureStoreUnavailable,
                            authentication_required: true,
                            secure_persistence_available: false,
                            session_only_available: true,
                        })
                    }
                    Err(_) => {
                        logging::error("ai_secret_store_error", "ai_secure_store_failed");
                        Ok(CredentialStatusDto {
                            status: CredentialStatus::SecureStoreUnavailable,
                            authentication_required: true,
                            secure_persistence_available: false,
                            session_only_available: true,
                        })
                    }
                }
            }
        }
    }

    pub fn resolve(
        &self,
        profile_id: &str,
        auth_mode: AuthMode,
        storage: CredentialStorage,
    ) -> Result<Option<SecretValue>, CommandError> {
        if !auth_mode.requires_credential() {
            return Ok(None);
        }

        match storage {
            CredentialStorage::None => Err(CommandError::new(
                "ai_credential_required",
                "This AI provider profile needs a credential.",
            )),
            CredentialStorage::Session => self
                .take_session_copy(profile_id)?
                .map(Some)
                .ok_or_else(|| {
                    CommandError::new(
                        "ai_credential_required",
                        "This session-only AI credential expired when ostudy closed. Enter it again.",
                    )
                }),
            CredentialStorage::Secure => match self.secure.read(&SecretLocator::ai_profile(profile_id)) {
                Ok(Some(secret)) => Ok(Some(secret)),
                Ok(None) => Err(CommandError::new(
                    "ai_credential_required",
                    "The securely saved AI credential is missing. Enter it again.",
                )),
                Err(SecretStoreError::Unavailable | SecretStoreError::Locked) => Err(
                    CommandError::new(
                        "ai_secure_store_unavailable",
                        "The operating-system credential store is unavailable.",
                    ),
                ),
                Err(_) => Err(secret_command_error(
                    "ai_secure_store_failed",
                    "The operating-system credential store could not be read.",
                )),
            },
        }
    }

    pub fn set(
        &self,
        profile_id: &str,
        credential: String,
        persistence: CredentialPersistence,
        previous_storage: CredentialStorage,
    ) -> Result<(), CommandError> {
        validate_credential(&credential)?;
        let locator = SecretLocator::ai_profile(profile_id);
        match persistence {
            CredentialPersistence::Secure => {
                self.secure
                    .write(&locator, credential.as_bytes())
                    .map_err(map_write_error)?;
                self.remove_session(profile_id)?;
            }
            CredentialPersistence::Session => {
                if previous_storage == CredentialStorage::Secure {
                    self.secure.delete(&locator).map_err(map_delete_error)?;
                }
                self.insert_session(profile_id, credential.into_bytes())?;
            }
        }
        Ok(())
    }

    pub fn clear(&self, profile_id: &str, storage: CredentialStorage) -> Result<(), CommandError> {
        match storage {
            CredentialStorage::None | CredentialStorage::Session => self.remove_session(profile_id),
            CredentialStorage::Secure => {
                self.secure
                    .delete(&SecretLocator::ai_profile(profile_id))
                    .map_err(map_delete_error)?;
                self.remove_session(profile_id)
            }
        }
    }

    fn session_contains(&self, profile_id: &str) -> Result<bool, CommandError> {
        let guard = self.session.lock().map_err(|_| secret_lock_error())?;
        Ok(guard.contains_key(profile_id))
    }

    fn take_session_copy(&self, profile_id: &str) -> Result<Option<SecretValue>, CommandError> {
        let guard = self.session.lock().map_err(|_| secret_lock_error())?;
        Ok(guard
            .get(profile_id)
            .map(|value| SecretValue::new(value.as_bytes().to_vec())))
    }

    fn insert_session(&self, profile_id: &str, bytes: Vec<u8>) -> Result<(), CommandError> {
        let mut guard = self.session.lock().map_err(|_| secret_lock_error())?;
        guard.insert(profile_id.to_string(), SecretValue::new(bytes));
        Ok(())
    }

    fn remove_session(&self, profile_id: &str) -> Result<(), CommandError> {
        let mut guard = self.session.lock().map_err(|_| secret_lock_error())?;
        guard.remove(profile_id);
        Ok(())
    }
}

fn validate_credential(value: &str) -> Result<(), CommandError> {
    if value.trim().is_empty() || value.len() > MAX_CREDENTIAL_BYTES {
        return Err(CommandError::new(
            "invalid_ai_credential",
            format!("Enter a non-empty AI credential no longer than {MAX_CREDENTIAL_BYTES} bytes."),
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(CommandError::new(
            "invalid_ai_credential",
            "AI credentials cannot contain control characters.",
        ));
    }
    Ok(())
}

fn map_write_error(error: SecretStoreError) -> CommandError {
    match error {
        SecretStoreError::Unavailable | SecretStoreError::Locked => CommandError::new(
            "ai_secure_store_unavailable",
            "Secure credential storage is unavailable. Use a session-only credential instead.",
        ),
        _ => secret_command_error(
            "ai_secure_store_failed",
            "The operating-system credential could not be saved.",
        ),
    }
}

fn map_delete_error(error: SecretStoreError) -> CommandError {
    match error {
        SecretStoreError::Unavailable | SecretStoreError::Locked => CommandError::new(
            "ai_secure_store_unavailable",
            "The operating-system credential store is unavailable, so ostudy cannot confirm credential removal.",
        ),
        _ => secret_command_error(
            "ai_secure_store_failed",
            "The operating-system credential could not be removed.",
        ),
    }
}

fn secret_lock_error() -> CommandError {
    CommandError::new(
        "ai_secret_lock_poisoned",
        "The in-memory AI credential state is unavailable. Restart ostudy.",
    )
}

fn secret_command_error(code: &str, message: &str) -> CommandError {
    logging::error("ai_secret_store_error", code);
    CommandError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_credentials_are_never_exposed_by_status() {
        let manager = SecretManager::default();
        manager
            .set(
                "0123456789abcdef0123456789abcdef",
                "sentinel-secret".to_string(),
                CredentialPersistence::Session,
                CredentialStorage::None,
            )
            .expect("session credential should save");
        let status = manager
            .status(
                "0123456789abcdef0123456789abcdef",
                AuthMode::Bearer,
                CredentialStorage::Session,
            )
            .expect("status should load");
        assert_eq!(status.status, CredentialStatus::SessionOnly);
        let serialized = serde_json::to_string(&status).expect("status should serialize");
        assert!(!serialized.contains("sentinel-secret"));
        assert!(manager
            .resolve(
                "0123456789abcdef0123456789abcdef",
                AuthMode::Bearer,
                CredentialStorage::Session,
            )
            .is_ok());
    }

    #[test]
    fn session_storage_reports_missing_after_restart_without_reading_secure_status() {
        let manager = SecretManager::default();
        let status = manager
            .status(
                "fedcba9876543210fedcba9876543210",
                AuthMode::Bearer,
                CredentialStorage::Session,
            )
            .expect("status should load without touching the secure store");
        assert_eq!(status.status, CredentialStatus::Missing);
    }

    #[test]
    fn credential_storage_round_trips_database_values() {
        for storage in [
            CredentialStorage::None,
            CredentialStorage::Secure,
            CredentialStorage::Session,
        ] {
            assert_eq!(
                CredentialStorage::from_db(storage.as_str()).unwrap(),
                storage
            );
        }
        assert!(CredentialStorage::from_db("plaintext").is_err());
    }

    #[test]
    fn rejects_empty_and_control_character_credentials() {
        assert!(validate_credential("").is_err());
        assert!(validate_credential("bad\nkey").is_err());
    }
}
