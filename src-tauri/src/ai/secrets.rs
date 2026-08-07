use std::{collections::HashMap, sync::Mutex};

use keyring::v1::{Entry, Error as KeyringError};
use serde::{Deserialize, Serialize};

use crate::{error::CommandError, logging};

use super::catalog::AuthMode;

const KEYRING_SERVICE: &str = "io.github.jferments.ostudy.ai";
const KEYRING_AVAILABILITY_ACCOUNT: &str = "availability-probe";
const MAX_CREDENTIAL_BYTES: usize = 8192;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialPersistence {
    Secure,
    Session,
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

pub struct SecretValue(Vec<u8>);

impl SecretValue {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn as_utf8(&self) -> Result<&str, CommandError> {
        std::str::from_utf8(&self.0).map_err(|_| {
            CommandError::new(
                "ai_credential_invalid",
                "The saved AI credential is not valid UTF-8 text.",
            )
        })
    }
}

impl Drop for SecretValue {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

pub struct SecretManager {
    session: Mutex<HashMap<String, SecretValue>>,
}

impl Default for SecretManager {
    fn default() -> Self {
        Self {
            session: Mutex::new(HashMap::new()),
        }
    }
}

impl SecretManager {
    pub fn status(
        &self,
        profile_id: &str,
        auth_mode: AuthMode,
    ) -> Result<CredentialStatusDto, CommandError> {
        if !auth_mode.requires_credential() {
            return Ok(CredentialStatusDto {
                status: CredentialStatus::NotRequired,
                authentication_required: false,
                secure_persistence_available: secure_store_available(),
                session_only_available: false,
            });
        }

        if self.session_contains(profile_id)? {
            return Ok(CredentialStatusDto {
                status: CredentialStatus::SessionOnly,
                authentication_required: true,
                secure_persistence_available: secure_store_available(),
                session_only_available: true,
            });
        }

        let secure_available = secure_store_available();
        let status = if !secure_available {
            CredentialStatus::SecureStoreUnavailable
        } else {
            match read_secure(profile_id) {
                Ok(Some(_secret)) => CredentialStatus::SavedSecurely,
                Ok(None) => CredentialStatus::Missing,
                Err(SecretStoreError::Unavailable) => CredentialStatus::SecureStoreUnavailable,
                Err(SecretStoreError::Failed) => {
                    return Err(secret_command_error(
                        "ai_secure_store_failed",
                        "The operating-system credential store could not be read.",
                    ));
                }
            }
        };

        Ok(CredentialStatusDto {
            status,
            authentication_required: true,
            secure_persistence_available: secure_available,
            session_only_available: true,
        })
    }

    pub fn resolve(
        &self,
        profile_id: &str,
        auth_mode: AuthMode,
    ) -> Result<Option<SecretValue>, CommandError> {
        if !auth_mode.requires_credential() {
            return Ok(None);
        }
        if let Some(secret) = self.take_session_copy(profile_id)? {
            return Ok(Some(secret));
        }
        match read_secure(profile_id) {
            Ok(Some(secret)) => Ok(Some(secret)),
            Ok(None) => Err(CommandError::new(
                "ai_credential_required",
                "This AI provider profile needs a credential.",
            )),
            Err(SecretStoreError::Unavailable) => Err(CommandError::new(
                "ai_secure_store_unavailable",
                "The operating-system credential store is unavailable. Enter a session-only credential.",
            )),
            Err(SecretStoreError::Failed) => Err(secret_command_error(
                "ai_secure_store_failed",
                "The operating-system credential store could not be read.",
            )),
        }
    }

    pub fn set(
        &self,
        profile_id: &str,
        credential: String,
        persistence: CredentialPersistence,
    ) -> Result<(), CommandError> {
        validate_credential(&credential)?;
        match persistence {
            CredentialPersistence::Secure => {
                if !secure_store_available() {
                    return Err(CommandError::new(
                        "ai_secure_store_unavailable",
                        "Secure credential storage is unavailable. Use a session-only credential instead.",
                    ));
                }
                write_secure(profile_id, credential.as_bytes()).map_err(map_write_error)?;
                self.remove_session(profile_id)?;
            }
            CredentialPersistence::Session => {
                self.insert_session(profile_id, credential.into_bytes())?;
            }
        }
        Ok(())
    }

    pub fn clear(&self, profile_id: &str) -> Result<(), CommandError> {
        self.remove_session(profile_id)?;
        if !secure_store_available() {
            return Ok(());
        }
        match delete_secure(profile_id) {
            Ok(()) => Ok(()),
            Err(SecretStoreError::Unavailable) => Err(CommandError::new(
                "ai_secure_store_unavailable",
                "The operating-system credential store is unavailable.",
            )),
            Err(SecretStoreError::Failed) => Err(secret_command_error(
                "ai_secure_store_failed",
                "The operating-system credential could not be removed.",
            )),
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
            .map(|value| SecretValue(value.as_bytes().to_vec())))
    }

    fn insert_session(&self, profile_id: &str, bytes: Vec<u8>) -> Result<(), CommandError> {
        let mut guard = self.session.lock().map_err(|_| secret_lock_error())?;
        guard.insert(profile_id.to_string(), SecretValue(bytes));
        Ok(())
    }

    fn remove_session(&self, profile_id: &str) -> Result<(), CommandError> {
        let mut guard = self.session.lock().map_err(|_| secret_lock_error())?;
        guard.remove(profile_id);
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum SecretStoreError {
    Unavailable,
    Failed,
}

fn secure_store_available() -> bool {
    Entry::new(KEYRING_SERVICE, KEYRING_AVAILABILITY_ACCOUNT).is_ok()
}

fn account_name(profile_id: &str) -> String {
    format!("profile/{profile_id}/api-key")
}

fn read_secure(profile_id: &str) -> Result<Option<SecretValue>, SecretStoreError> {
    let entry = Entry::new(KEYRING_SERVICE, &account_name(profile_id)).map_err(classify_error)?;
    match entry.get_secret() {
        Ok(secret) => Ok(Some(SecretValue(secret))),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(error) => Err(classify_error(error)),
    }
}

fn write_secure(profile_id: &str, secret: &[u8]) -> Result<(), SecretStoreError> {
    let entry = Entry::new(KEYRING_SERVICE, &account_name(profile_id)).map_err(classify_error)?;
    entry.set_secret(secret).map_err(classify_error)
}

fn delete_secure(profile_id: &str) -> Result<(), SecretStoreError> {
    let entry = Entry::new(KEYRING_SERVICE, &account_name(profile_id)).map_err(classify_error)?;
    match entry.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(error) => Err(classify_error(error)),
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

fn validate_credential(value: &str) -> Result<(), CommandError> {
    if value.trim().is_empty() || value.len() > MAX_CREDENTIAL_BYTES {
        return Err(CommandError::new(
            "invalid_ai_credential",
            format!(
                "Enter a non-empty AI credential no longer than {MAX_CREDENTIAL_BYTES} bytes."
            ),
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
        SecretStoreError::Unavailable => CommandError::new(
            "ai_secure_store_unavailable",
            "Secure credential storage is unavailable. Use a session-only credential instead.",
        ),
        SecretStoreError::Failed => secret_command_error(
            "ai_secure_store_failed",
            "The operating-system credential could not be saved.",
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
            )
            .expect("session credential should save");
        let status = manager
            .status(
                "0123456789abcdef0123456789abcdef",
                AuthMode::Bearer,
            )
            .expect("status should load");
        assert_eq!(status.status, CredentialStatus::SessionOnly);
        let serialized = serde_json::to_string(&status).expect("status should serialize");
        assert!(!serialized.contains("sentinel-secret"));
    }

    #[test]
    fn rejects_empty_and_control_character_credentials() {
        assert!(validate_credential("").is_err());
        assert!(validate_credential("bad\nkey").is_err());
    }
}
