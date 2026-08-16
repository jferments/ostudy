//! Explicit, encrypted-only AI credential handoff for trusted device sync.
//!
//! Credential bytes never enter a device snapshot, prepared export, SQLite row,
//! or conflict preview file. A QR/LAN receiver keeps the decrypted records in a
//! bounded process-memory staging map until the user commits or discards the
//! associated merge preview. Committing writes each credential into the target
//! platform secure store and updates only the non-secret storage metadata.

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    error::CommandError,
    platform::{PlatformRuntime, SecretLocator, SecretStoreAvailability},
    util::{unix_time_millis, validate_entity_id},
};

const MAX_TRANSFERRED_CREDENTIALS: usize = 64;
const MAX_CREDENTIAL_BYTES: usize = 64 * 1024;
const MAX_TOTAL_CREDENTIAL_BYTES: usize = 1024 * 1024;

static STAGED_CREDENTIALS: LazyLock<Mutex<HashMap<String, Vec<TransferredCredential>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TransferredCredential {
    pub profile_id: String,
    pub value_base64: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CredentialPromotionResult {
    pub imported: usize,
    pub failed: usize,
}

pub(crate) fn collect_transferred_credentials(
    database: &Database,
    runtime: &PlatformRuntime,
) -> Result<Vec<TransferredCredential>, CommandError> {
    if runtime.secure_secret_store().availability() != SecretStoreAvailability::Available {
        return Err(CommandError::new(
            "sync_secure_store_unavailable",
            "Secure storage is unavailable, so API credentials cannot be transferred.",
        ));
    }
    let profile_ids = database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT id
                 FROM ai_provider_profiles
                 WHERE deleted_at IS NULL
                   AND credential_storage = 'secure'
                 ORDER BY id",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)
    })?;
    if profile_ids.len() > MAX_TRANSFERRED_CREDENTIALS {
        return Err(CommandError::new(
            "sync_credential_limit",
            "Too many saved AI credentials are eligible for one synchronization session.",
        ));
    }

    let store = runtime.secure_secret_store();
    let mut total = 0_usize;
    let mut records = Vec::new();
    for profile_id in profile_ids {
        validate_entity_id(&profile_id, "AI provider profile")?;
        let secret = store
            .read(&SecretLocator::ai_profile(&profile_id))
            .map_err(|_| {
                CommandError::new(
                    "sync_credential_read_failed",
                    "A selected AI credential could not be read from secure storage.",
                )
            })?;
        let Some(secret) = secret else {
            continue;
        };
        if secret.as_bytes().is_empty() || secret.as_bytes().len() > MAX_CREDENTIAL_BYTES {
            return Err(CommandError::new(
                "sync_credential_invalid",
                "A selected AI credential is empty or exceeds the transfer limit.",
            ));
        }
        total = total.saturating_add(secret.as_bytes().len());
        if total > MAX_TOTAL_CREDENTIAL_BYTES {
            return Err(CommandError::new(
                "sync_credential_limit",
                "The selected AI credentials exceed the total encrypted-transfer limit.",
            ));
        }
        records.push(TransferredCredential {
            profile_id,
            value_base64: BASE64_STANDARD.encode(secret.as_bytes()),
        });
    }
    Ok(records)
}

pub(crate) fn stage_received_credentials(
    preview_id: &str,
    credentials: Vec<TransferredCredential>,
) -> Result<usize, CommandError> {
    validate_entity_id(preview_id, "device sync preview")?;
    validate_credentials(&credentials)?;
    let count = credentials.len();
    let mut staged = staged_credentials();
    if let Some(mut previous) = staged.insert(preview_id.to_string(), credentials) {
        scrub_credentials(&mut previous);
    }
    Ok(count)
}

pub(crate) fn discard_received_credentials(preview_id: &str) {
    if let Some(mut credentials) = staged_credentials().remove(preview_id) {
        scrub_credentials(&mut credentials);
    }
}

pub(crate) fn promote_received_credentials(
    preview_id: &str,
    database: &Database,
    runtime: &PlatformRuntime,
) -> CredentialPromotionResult {
    let Some(mut credentials) = staged_credentials().remove(preview_id) else {
        return CredentialPromotionResult::default();
    };
    let store = runtime.secure_secret_store();
    if store.availability() != SecretStoreAvailability::Available {
        let failed = credentials.len();
        scrub_credentials(&mut credentials);
        return CredentialPromotionResult {
            imported: 0,
            failed,
        };
    }

    let mut result = CredentialPromotionResult::default();
    for credential in &mut credentials {
        let exists = database
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT 1
                         FROM ai_provider_profiles
                         WHERE id = ?1 AND deleted_at IS NULL",
                        [credential.profile_id.as_str()],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional()
                    .map(|value| value.is_some())
                    .map_err(CommandError::database)
            })
            .unwrap_or(false);
        if !exists {
            result.failed = result.failed.saturating_add(1);
            continue;
        }
        let mut value = match BASE64_STANDARD.decode(&credential.value_base64) {
            Ok(value) if !value.is_empty() && value.len() <= MAX_CREDENTIAL_BYTES => value,
            _ => {
                result.failed = result.failed.saturating_add(1);
                continue;
            }
        };
        let written = store
            .write(&SecretLocator::ai_profile(&credential.profile_id), &value)
            .is_ok();
        value.fill(0);
        if !written {
            result.failed = result.failed.saturating_add(1);
            continue;
        }
        let updated = database.with_connection(|connection| {
            let now = unix_time_millis()?;
            connection
                .execute(
                    "UPDATE ai_provider_profiles
                     SET credential_storage = 'secure',
                         credential_epoch = credential_epoch + 1,
                         revision = revision + 1,
                         updated_at = ?1
                     WHERE id = ?2 AND deleted_at IS NULL",
                    rusqlite::params![now, credential.profile_id],
                )
                .map_err(CommandError::database)
        });
        match updated {
            Ok(1) => result.imported = result.imported.saturating_add(1),
            _ => result.failed = result.failed.saturating_add(1),
        }
    }
    scrub_credentials(&mut credentials);
    result
}

fn validate_credentials(credentials: &[TransferredCredential]) -> Result<(), CommandError> {
    if credentials.len() > MAX_TRANSFERRED_CREDENTIALS {
        return Err(CommandError::new(
            "sync_credential_limit",
            "The encrypted synchronization payload contains too many credentials.",
        ));
    }
    let mut total = 0_usize;
    let mut seen = std::collections::BTreeSet::new();
    for credential in credentials {
        validate_entity_id(&credential.profile_id, "AI provider profile")?;
        if !seen.insert(&credential.profile_id) {
            return Err(CommandError::new(
                "sync_credential_invalid",
                "The encrypted synchronization payload contains a duplicate credential.",
            ));
        }
        let mut value = BASE64_STANDARD.decode(&credential.value_base64).map_err(|_| {
            CommandError::new(
                "sync_credential_invalid",
                "An encrypted synchronization credential is not valid base64.",
            )
        })?;
        let length = value.len();
        value.fill(0);
        if length == 0 || length > MAX_CREDENTIAL_BYTES {
            return Err(CommandError::new(
                "sync_credential_invalid",
                "An encrypted synchronization credential has an invalid size.",
            ));
        }
        total = total.saturating_add(length);
        if total > MAX_TOTAL_CREDENTIAL_BYTES {
            return Err(CommandError::new(
                "sync_credential_limit",
                "The encrypted synchronization credentials exceed the total size limit.",
            ));
        }
    }
    Ok(())
}

fn staged_credentials(
) -> MutexGuard<'static, HashMap<String, Vec<TransferredCredential>>> {
    STAGED_CREDENTIALS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn scrub_credentials(credentials: &mut [TransferredCredential]) {
    for credential in credentials {
        if let Ok(mut decoded) = BASE64_STANDARD.decode(&credential.value_base64) {
            decoded.fill(0);
        }
        credential.value_base64.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_is_bounded_and_discardable_without_persistence() {
        let preview_id = "11111111111111111111111111111111";
        let credential = TransferredCredential {
            profile_id: "22222222222222222222222222222222".to_string(),
            value_base64: BASE64_STANDARD.encode(b"secret"),
        };
        assert_eq!(
            stage_received_credentials(preview_id, vec![credential])
                .expect("credential should stage"),
            1
        );
        assert!(staged_credentials().contains_key(preview_id));
        discard_received_credentials(preview_id);
        assert!(!staged_credentials().contains_key(preview_id));
    }

    #[test]
    fn duplicate_profile_ids_are_rejected() {
        let credential = TransferredCredential {
            profile_id: "33333333333333333333333333333333".to_string(),
            value_base64: BASE64_STANDARD.encode(b"secret"),
        };
        assert!(validate_credentials(&[credential.clone(), credential]).is_err());
    }
}
