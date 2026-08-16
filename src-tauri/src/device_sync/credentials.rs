use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    error::CommandError,
    platform::{SecretLocator, SecretStoreError, SecureSecretStore},
    util::{unix_time_millis, validate_entity_id},
};

const MAX_CREDENTIAL_BYTES: usize = 64 * 1024;
const MAX_CREDENTIAL_RECORDS: usize = 64;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CredentialRecord {
    pub profile_id: String,
    pub value_base64: String,
}

pub(crate) fn collect_credentials(
    database: &Database,
    secret_store: Arc<dyn SecureSecretStore>,
) -> Result<Vec<CredentialRecord>, CommandError> {
    let profile_ids = database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT id
                 FROM ai_provider_profiles
                 WHERE deleted_at IS NULL AND credential_storage = 'secure'
                 ORDER BY id
                 LIMIT ?1",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map(
                [i64::try_from(MAX_CREDENTIAL_RECORDS).unwrap_or(i64::MAX)],
                |row| row.get::<_, String>(0),
            )
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)
    })?;
    let mut output = Vec::new();
    for profile_id in profile_ids {
        validate_entity_id(&profile_id, "AI profile")?;
        let Some(secret) = secret_store
            .read(&SecretLocator::ai_profile(&profile_id))
            .map_err(secret_error)?
        else {
            continue;
        };
        if secret.as_bytes().is_empty() || secret.as_bytes().len() > MAX_CREDENTIAL_BYTES {
            return Err(CommandError::new(
                "device_sync_credential_invalid",
                "A saved AI credential has an unsupported size.",
            ));
        }
        output.push(CredentialRecord {
            profile_id,
            value_base64: BASE64_STANDARD.encode(secret.as_bytes()),
        });
    }
    Ok(output)
}

pub(crate) fn apply_credentials(
    database: &Database,
    secret_store: Arc<dyn SecureSecretStore>,
    records: &[CredentialRecord],
) -> Result<usize, CommandError> {
    if records.len() > MAX_CREDENTIAL_RECORDS {
        return Err(CommandError::new(
            "device_sync_credential_invalid",
            "The paired device sent too many AI credentials.",
        ));
    }
    let mut decoded = Vec::with_capacity(records.len());
    for record in records {
        validate_entity_id(&record.profile_id, "AI profile")?;
        let bytes = BASE64_STANDARD.decode(&record.value_base64).map_err(|_| {
            CommandError::new(
                "device_sync_credential_invalid",
                "A synchronized AI credential is not valid base64.",
            )
        })?;
        if bytes.is_empty() || bytes.len() > MAX_CREDENTIAL_BYTES {
            return Err(CommandError::new(
                "device_sync_credential_invalid",
                "A synchronized AI credential has an unsupported size.",
            ));
        }
        decoded.push((record.profile_id.clone(), bytes));
    }

    let existing = database.with_connection(|connection| {
        let mut profile_ids = Vec::new();
        for (profile_id, _) in &decoded {
            let exists: bool = connection
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM ai_provider_profiles
                         WHERE id = ?1 AND deleted_at IS NULL
                     )",
                    [profile_id],
                    |row| row.get::<_, i64>(0),
                )
                .map(|value| value == 1)
                .map_err(CommandError::database)?;
            if exists {
                profile_ids.push(profile_id.clone());
            }
        }
        Ok(profile_ids)
    })?;

    for (profile_id, bytes) in &decoded {
        if existing.contains(profile_id) {
            secret_store
                .write(&SecretLocator::ai_profile(profile_id), bytes)
                .map_err(secret_error)?;
        }
    }
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection.transaction().map_err(CommandError::database)?;
        for profile_id in &existing {
            transaction
                .execute(
                    "UPDATE ai_provider_profiles
                     SET credential_storage = 'secure',
                         credential_epoch = credential_epoch + 1,
                         revision = revision + 1,
                         updated_at = ?1
                     WHERE id = ?2 AND deleted_at IS NULL",
                    params![now, profile_id],
                )
                .map_err(CommandError::database)?;
            transaction
                .execute(
                    "DELETE FROM ai_provider_probe_results WHERE profile_id = ?1",
                    [profile_id],
                )
                .map_err(CommandError::database)?;
            transaction
                .execute(
                    "DELETE FROM ai_provider_capabilities WHERE profile_id = ?1",
                    [profile_id],
                )
                .map_err(CommandError::database)?;
        }
        transaction.commit().map_err(CommandError::database)
    })?;
    for (_, bytes) in &mut decoded {
        bytes.fill(0);
    }
    Ok(existing.len())
}

fn secret_error(error: SecretStoreError) -> CommandError {
    let message = match error {
        SecretStoreError::Unavailable => {
            "The operating-system secure store is unavailable for credential transfer."
        }
        SecretStoreError::Locked => {
            "Unlock the operating-system secure store before transferring credentials."
        }
        SecretStoreError::PermissionDenied => {
            "The operating system denied access to synchronized credentials."
        }
        SecretStoreError::Invalidated => {
            "A synchronized credential was invalidated by the operating system."
        }
        SecretStoreError::Failed => "The synchronized credential could not be stored securely.",
    };
    CommandError::new("device_sync_secure_store_failed", message)
}
