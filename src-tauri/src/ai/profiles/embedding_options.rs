//! Per-profile embedding configuration. Task defaults select a profile; they do
//! not become a second owner for endpoint, model, credentials, or dimensions.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    error::CommandError,
    util::{unix_time_millis, validate_entity_id},
};

use super::{super::catalog::ProviderKind, repository::load_profile};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingInputPolicy {
    Automatic,
    Symmetric,
    QwenRetrieval,
    SearchTypes,
}

impl EmbeddingInputPolicy {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Symmetric => "symmetric",
            Self::QwenRetrieval => "qwen_retrieval",
            Self::SearchTypes => "search_types",
        }
    }

    fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "automatic" => Ok(Self::Automatic),
            "symmetric" => Ok(Self::Symmetric),
            "qwen_retrieval" => Ok(Self::QwenRetrieval),
            "search_types" => Ok(Self::SearchTypes),
            _ => Err(CommandError::new(
                "invalid_embedding_options",
                "The saved embedding input policy is invalid.",
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmbeddingOptions {
    pub profile_id: String,
    pub dimensions: Option<usize>,
    pub input_policy: EmbeddingInputPolicy,
    pub revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveEmbeddingOptionsInput {
    pub profile_id: String,
    pub expected_revision: i64,
    pub dimensions: Option<usize>,
    pub input_policy: EmbeddingInputPolicy,
}

pub(crate) fn save_embedding_options(
    database: &Database,
    input: SaveEmbeddingOptionsInput,
) -> Result<EmbeddingOptions, CommandError> {
    validate_entity_id(&input.profile_id, "embedding profile")?;
    if input.expected_revision < 0
        || input
            .dimensions
            .is_some_and(|value| !(32..=4096).contains(&value))
    {
        return Err(CommandError::new(
            "invalid_embedding_options",
            "Choose provider-default dimensions or a supported size between 32 and 4096.",
        ));
    }
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let profile = load_profile(&transaction, &input.profile_id)?;
        if !matches!(profile.provider_kind, ProviderKind::Openrouter | ProviderKind::LlamaCpp) {
            return Err(CommandError::new(
                "ai_embeddings_unsupported",
                "This provider does not have an implemented embedding adapter.",
            ));
        }
        let current = load_in_connection(&transaction, &input.profile_id)?;
        // Repeating an acknowledged or ambiguous save with identical values is
        // idempotent. A different change must match the observed revision.
        if current.dimensions == input.dimensions && current.input_policy == input.input_policy {
            return Ok(current);
        }
        if current.revision != input.expected_revision {
            return Err(CommandError::new(
                "embedding_options_changed",
                "Embedding options changed. Reload them before saving another configuration.",
            ));
        }
        let revision = current.revision + 1;
        transaction
            .execute(
                "INSERT INTO ai_embedding_options (profile_id, dimensions, input_policy, revision, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(profile_id) DO UPDATE SET dimensions = excluded.dimensions,
                     input_policy = excluded.input_policy, revision = excluded.revision, updated_at = excluded.updated_at",
                params![input.profile_id, input.dimensions, input.input_policy.as_str(), revision, unix_time_millis()?],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(EmbeddingOptions {
            profile_id: input.profile_id,
            dimensions: input.dimensions,
            input_policy: input.input_policy,
            revision,
        })
    })
}

pub(crate) fn load_embedding_options(
    database: &Database,
    profile_id: &str,
) -> Result<EmbeddingOptions, CommandError> {
    validate_entity_id(profile_id, "embedding profile")?;
    database.with_connection(|connection| {
        load_profile(connection, profile_id)?;
        load_in_connection(connection, profile_id)
    })
}

pub(super) fn load_in_connection(
    connection: &Connection,
    profile_id: &str,
) -> Result<EmbeddingOptions, CommandError> {
    let row = connection
        .query_row(
            "SELECT dimensions, input_policy, revision FROM ai_embedding_options WHERE profile_id = ?1",
            [profile_id],
            |row| Ok((row.get::<_, Option<usize>>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    match row {
        Some((dimensions, policy, revision)) => Ok(EmbeddingOptions {
            profile_id: profile_id.to_string(),
            dimensions,
            input_policy: EmbeddingInputPolicy::from_db(&policy)?,
            revision,
        }),
        None => Ok(EmbeddingOptions {
            profile_id: profile_id.to_string(),
            dimensions: None,
            input_policy: EmbeddingInputPolicy::Automatic,
            revision: 0,
        }),
    }
}
