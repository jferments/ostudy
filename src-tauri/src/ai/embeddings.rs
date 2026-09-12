//! Hosted embedding tasks. The application stores/searches returned vectors;
//! it never loads an embedding model. Provider connections own credentials while
//! task assignments choose one model on that connection.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    database::Database, error::CommandError, task_cancellation::CancellationSignal,
    util::validate_entity_id,
};

use super::{
    catalog::AiTaskKind,
    profiles::{
        embedding_options::{load_embedding_options, EmbeddingOptions},
        load_profile_request, load_task_profile_request, validate_model_id, ProfileRequestSnapshot,
    },
    providers::embeddings::{
        decode_embedding_models, decode_embeddings, embedding_models_url,
        prepare_embedding_request, resolved_policy, EmbeddingModelChoice, EmbeddingPurpose,
        EMBEDDING_TRANSPORT_VERSION, MAX_EMBEDDING_RESPONSE_BYTES,
    },
    AiRuntimeState,
};

pub(crate) use super::providers::embeddings::EmbeddingPurpose as Purpose;

#[derive(Clone, Debug)]
pub(crate) struct EmbeddingContext {
    pub request: ProfileRequestSnapshot,
    pub options: EmbeddingOptions,
    pub space_id: String,
}

impl EmbeddingContext {
    pub(crate) fn consent_identity(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}",
            self.request.profile.id,
            self.request.profile.model_id,
            self.request.profile.revision,
            self.request.profile.credential_epoch,
            self.options.revision,
            self.space_id,
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmbeddingProfileInput {
    pub profile_id: String,
    #[serde(default)]
    pub model_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingTestResult {
    pub profile_id: String,
    pub model_id: String,
    pub profile_revision: i64,
    pub options_revision: i64,
    pub dimensions: usize,
    pub space_id: String,
    pub query_and_document_tested: bool,
}

pub(crate) fn load_default_embedding_context(
    database: &Database,
) -> Result<EmbeddingContext, CommandError> {
    let request = load_task_profile_request(database, AiTaskKind::Embeddings)?;
    context_for_request(database, request)
}

fn context_for_request(
    database: &Database,
    request: ProfileRequestSnapshot,
) -> Result<EmbeddingContext, CommandError> {
    request.profile.validate_for_provider_request()?;
    let options = load_embedding_options(database, &request.profile.id, &request.profile.model_id)?;
    let profile = &request.profile;
    let space_contract = serde_json::to_string(&(
        EMBEDDING_TRANSPORT_VERSION,
        "ostudy-retrieval-text-v1",
        "l2-normalized-f32-v1",
        profile.provider_kind.as_str(),
        &profile.base_url,
        &profile.model_id,
        options.dimensions,
        resolved_policy(&profile.model_id, options.input_policy),
    ))
    .map_err(CommandError::serialization)?;
    Ok(EmbeddingContext {
        space_id: format!("{:x}", Sha256::digest(space_contract.as_bytes())),
        request,
        options,
    })
}

pub(crate) fn ensure_context_current(
    database: &Database,
    context: &EmbeddingContext,
) -> Result<(), CommandError> {
    let current = load_profile_request(database, &context.request.profile.id)?;
    let options = load_embedding_options(
        database,
        &context.request.profile.id,
        &context.request.profile.model_id,
    )?;
    if current.profile.revision != context.request.profile.revision
        || current.profile.credential_epoch != context.request.profile.credential_epoch
        || current.profile.base_url != context.request.profile.base_url
        || current.credential_storage != context.request.credential_storage
        || options != context.options
    {
        return Err(CommandError::new(
            "embedding_profile_changed",
            "The embedding provider or model options changed during this request. Review the selection and prepare again.",
        ));
    }
    context.request.profile.validate_for_provider_request()
}

pub(crate) async fn embed(
    database: &Database,
    runtime: &AiRuntimeState,
    context: &EmbeddingContext,
    purpose: Purpose,
    inputs: &[String],
    cancellation: &CancellationSignal,
) -> Result<Vec<Vec<f32>>, CommandError> {
    ensure_context_current(database, context)?;
    check_canceled(cancellation)?;
    let profile = &context.request.profile;
    let prepared = prepare_embedding_request(profile, &context.options, purpose, inputs)?;
    let credential = runtime.secrets.resolve(
        &profile.id,
        profile.auth_mode,
        context.request.credential_storage,
    )?;
    let bearer = credential
        .as_ref()
        .map(|secret| secret.as_utf8())
        .transpose()?;
    let response = tokio::select! {
        _ = cancellation.canceled() => return Err(canceled()),
        response = runtime.gateway.post_json(
            prepared.url,
            bearer,
            &prepared.body,
            profile.request_timeout_ms,
            MAX_EMBEDDING_RESPONSE_BYTES,
        ) => response.map_err(|failure| CommandError::new(failure.safe.code, failure.safe.message))?,
    };
    ensure_context_current(database, context)?;
    check_canceled(cancellation)?;
    decode_embeddings(&response, inputs.len(), context.options.dimensions)
}

#[tauri::command]
pub async fn list_embedding_models(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: EmbeddingProfileInput,
) -> Result<Vec<EmbeddingModelChoice>, CommandError> {
    validate_entity_id(&input.profile_id, "embedding provider")?;
    let request = load_profile_request(database.inner(), &input.profile_id)?;
    let profile = &request.profile;
    let url = embedding_models_url(profile)?;
    let credential =
        runtime
            .secrets
            .resolve(&profile.id, profile.auth_mode, request.credential_storage)?;
    let bearer = credential
        .as_ref()
        .map(|secret| secret.as_utf8())
        .transpose()?;
    let response = runtime
        .gateway
        .get_json(
            url,
            bearer,
            profile.request_timeout_ms,
            MAX_EMBEDDING_RESPONSE_BYTES,
        )
        .await
        .map_err(|failure| CommandError::new(failure.safe.code, failure.safe.message))?;
    let current = load_profile_request(database.inner(), &input.profile_id)?;
    if current.profile.revision != profile.revision
        || current.profile.credential_epoch != profile.credential_epoch
    {
        return Err(CommandError::new(
            "embedding_profile_changed",
            "The provider connection changed while its embedding models were loading.",
        ));
    }
    decode_embedding_models(&response)
}

#[tauri::command]
pub async fn test_embedding_profile(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: EmbeddingProfileInput,
) -> Result<EmbeddingTestResult, CommandError> {
    validate_entity_id(&input.profile_id, "embedding provider")?;
    let model_id = validate_model_id(input.model_id.as_deref().unwrap_or_default())?;
    if model_id.is_empty() {
        return Err(CommandError::new(
            "invalid_embedding_options",
            "Choose an embedding model before testing it.",
        ));
    }
    let mut request = load_profile_request(database.inner(), &input.profile_id)?;
    request.profile.model_id.clone_from(&model_id);
    request.profile.reasoning_effort = "none".to_string();
    let context = context_for_request(database.inner(), request)?;
    let cancellation = CancellationSignal::new();
    let documents = embed(
        database.inner(),
        runtime.inner(),
        &context,
        EmbeddingPurpose::Document,
        &[
            "A triangle has three sides.".to_string(),
            "Water freezes at its freezing point.".to_string(),
        ],
        &cancellation,
    )
    .await?;
    let query = embed(
        database.inner(),
        runtime.inner(),
        &context,
        EmbeddingPurpose::Query,
        &["How many sides does a triangle have?".to_string()],
        &cancellation,
    )
    .await?;
    let dimensions = documents[0].len();
    if query[0].len() != dimensions {
        return Err(CommandError::new(
            "embedding_dimensions_mismatch",
            "This provider returned incompatible query and document vector sizes.",
        ));
    }
    Ok(EmbeddingTestResult {
        profile_id: context.request.profile.id,
        model_id,
        profile_revision: context.request.profile.revision,
        options_revision: context.options.revision,
        dimensions,
        space_id: context.space_id,
        query_and_document_tested: true,
    })
}

fn check_canceled(cancellation: &CancellationSignal) -> Result<(), CommandError> {
    if cancellation.is_canceled() {
        Err(canceled())
    } else {
        Ok(())
    }
}

fn canceled() -> CommandError {
    CommandError::new("ai_request_canceled", "The embedding request was canceled.")
}
