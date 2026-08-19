#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content)


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    if old not in text:
        if new in text:
            return
        raise RuntimeError(f"{path}: missing anchor {old[:160]!r}")
    if text.count(old) != 1:
        raise RuntimeError(f"{path}: ambiguous anchor {old[:160]!r}")
    write(path, text.replace(old, new, 1))


write(
    "src-tauri/src/ai/setup.rs",
    r'''//! Ephemeral AI-provider setup sessions.
//!
//! A setup session is deliberately not an AI provider profile. Provider/model
//! discovery happens against native in-memory metadata plus a session-only
//! credential. SQLite receives a profile only after the learner chooses a model
//! and commits setup. Abandoned setup therefore leaves neither an incomplete
//! profile nor plaintext credentials behind.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

use super::{
    catalog::{AuthMode, ProviderKind, MAX_TIMEOUT_MS, MIN_TIMEOUT_MS},
    endpoint_policy::validate_endpoint,
    profiles::{self, AiProviderProfileDetailDto, ProfileSnapshot, SetupProfileCommitInput},
    providers::{self, ProviderModel},
    secrets::{CredentialPersistence, CredentialStorage},
    AiRuntimeState,
};

const SETUP_TTL_MS: i64 = 15 * 60 * 1_000;

#[derive(Clone)]
pub(crate) struct PendingAiProviderSetup {
    pub profile_id: String,
    pub operation_id: String,
    pub provider_kind: ProviderKind,
    pub base_url: String,
    pub auth_mode: AuthMode,
    pub allow_insecure_http: bool,
    pub require_zero_data_retention: bool,
    pub request_timeout_ms: i64,
    pub persistence: CredentialPersistence,
    pub model_ids: Vec<String>,
    pub created_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BeginAiProviderSetupInput {
    pub provider_kind: ProviderKind,
    pub base_url: String,
    pub credential: String,
    pub persistence: CredentialPersistence,
    pub allow_insecure_http: bool,
    #[serde(default)]
    pub require_zero_data_retention: bool,
    pub request_timeout_ms: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommitAiProviderSetupInput {
    pub setup_id: String,
    pub display_name: String,
    pub model_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderSetupResultDto {
    pub setup_id: String,
    pub models: Vec<ProviderModel>,
}

#[tauri::command]
pub async fn begin_ai_provider_setup(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: BeginAiProviderSetupInput,
) -> Result<AiProviderSetupResultDto, CommandError> {
    if !input.provider_kind.implemented() {
        return Err(CommandError::new(
            "ai_provider_not_implemented",
            "This AI service is not implemented in this ostudy build.",
        ));
    }
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&input.request_timeout_ms) {
        return Err(CommandError::new(
            "invalid_ai_timeout",
            "The AI inactivity timeout is outside the supported range.",
        ));
    }
    if input.require_zero_data_retention && input.provider_kind != ProviderKind::Openrouter {
        return Err(CommandError::new(
            "invalid_ai_privacy_configuration",
            "Zero-data-retention routing is available only for OpenRouter profiles.",
        ));
    }

    let credential_present = !input.credential.trim().is_empty();
    let authentication_required = input.provider_kind.default_auth_mode().requires_credential();
    if authentication_required && !credential_present {
        return Err(CommandError::new(
            "ai_credential_required",
            "Enter an API key before retrieving models from this service.",
        ));
    }
    let auth_mode = if credential_present {
        AuthMode::Bearer
    } else {
        AuthMode::None
    };
    let endpoint = validate_endpoint(
        input.provider_kind,
        &input.base_url,
        input.allow_insecure_http,
    )?;
    if let Some(fixed) = input.provider_kind.fixed_base_url() {
        let fixed_endpoint = validate_endpoint(input.provider_kind, fixed, false)?;
        if endpoint.normalized_url != fixed_endpoint.normalized_url {
            return Err(CommandError::new(
                "ai_provider_endpoint_fixed",
                "This cloud provider uses its fixed official API endpoint.",
            ));
        }
    }

    prune_expired(runtime.inner())?;
    let (setup_id, profile_id, operation_id) = database.with_connection(|connection| {
        Ok((
            generate_id(connection)?,
            generate_id(connection)?,
            generate_id(connection)?,
        ))
    })?;

    if auth_mode.requires_credential() {
        runtime.secrets.set(
            &setup_id,
            input.credential,
            CredentialPersistence::Session,
            CredentialStorage::None,
        )?;
    }

    let snapshot = ProfileSnapshot {
        id: setup_id.clone(),
        provider_kind: input.provider_kind,
        base_url: endpoint.normalized_url.clone(),
        model_id: String::new(),
        auth_mode,
        enabled: true,
        allow_insecure_http: input.allow_insecure_http,
        require_zero_data_retention: input.require_zero_data_retention,
        request_timeout_ms: input.request_timeout_ms,
        credential_epoch: 0,
        revision: 1,
    };
    let bearer = runtime
        .secrets
        .resolve(&setup_id, auth_mode, CredentialStorage::Session)?;
    let models = match providers::list_models(
        &runtime.gateway,
        &snapshot,
        bearer
            .as_ref()
            .map(|secret| secret.as_utf8())
            .transpose()?,
    )
    .await
    {
        Ok(models) => models,
        Err(error) => {
            let _ = runtime.secrets.clear(&setup_id, CredentialStorage::Session);
            return Err(CommandError::new(error.safe.code, error.safe.message));
        }
    };

    let pending = PendingAiProviderSetup {
        profile_id,
        operation_id,
        provider_kind: input.provider_kind,
        base_url: endpoint.normalized_url,
        auth_mode,
        allow_insecure_http: input.allow_insecure_http,
        require_zero_data_retention: input.require_zero_data_retention,
        request_timeout_ms: input.request_timeout_ms,
        persistence: input.persistence,
        model_ids: models.iter().map(|model| model.id.clone()).collect(),
        created_at: unix_time_millis()?,
    };
    runtime
        .setup_sessions
        .lock()
        .map_err(|_| CommandError::new("ai_setup_state_unavailable", "AI setup state is unavailable."))?
        .insert(setup_id.clone(), pending);

    Ok(AiProviderSetupResultDto { setup_id, models })
}

#[tauri::command]
pub fn commit_ai_provider_setup(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: CommitAiProviderSetupInput,
) -> Result<AiProviderProfileDetailDto, CommandError> {
    validate_entity_id(&input.setup_id, "AI setup session")?;
    prune_expired(runtime.inner())?;
    let pending = runtime
        .setup_sessions
        .lock()
        .map_err(|_| CommandError::new("ai_setup_state_unavailable", "AI setup state is unavailable."))?
        .get(&input.setup_id)
        .cloned()
        .ok_or_else(|| {
            CommandError::new(
                "ai_setup_expired",
                "This AI setup session expired. Connect to the service again.",
            )
        })?;
    if !pending.model_ids.iter().any(|candidate| candidate == &input.model_id) {
        return Err(CommandError::new(
            "ai_setup_model_invalid",
            "Choose a model returned by the current setup session.",
        ));
    }
    let credential = runtime
        .secrets
        .resolve(
            &input.setup_id,
            pending.auth_mode,
            if pending.auth_mode.requires_credential() {
                CredentialStorage::Session
            } else {
                CredentialStorage::None
            },
        )?;
    let display_name = if input.display_name.trim().is_empty() {
        format!("{} — {}", pending.provider_kind.display_name(), input.model_id)
    } else {
        input.display_name
    };
    let profile_id = profiles::commit_ai_provider_setup_profile(
        database.inner(),
        runtime.inner(),
        SetupProfileCommitInput {
            profile_id: pending.profile_id.clone(),
            operation_id: pending.operation_id.clone(),
            display_name,
            provider_kind: pending.provider_kind,
            base_url: pending.base_url.clone(),
            model_id: input.model_id,
            auth_mode: pending.auth_mode,
            allow_insecure_http: pending.allow_insecure_http,
            require_zero_data_retention: pending.require_zero_data_retention,
            request_timeout_ms: pending.request_timeout_ms,
            persistence: pending.persistence,
        },
        credential
            .as_ref()
            .map(|secret| secret.as_utf8())
            .transpose()?,
    )?;
    let _ = runtime.secrets.clear(&input.setup_id, CredentialStorage::Session);
    runtime
        .setup_sessions
        .lock()
        .map_err(|_| CommandError::new("ai_setup_state_unavailable", "AI setup state is unavailable."))?
        .remove(&input.setup_id);
    profiles::get_ai_provider_profile(database, runtime, profile_id)
}

#[tauri::command]
pub fn cancel_ai_provider_setup(
    runtime: State<'_, AiRuntimeState>,
    setup_id: String,
) -> Result<(), CommandError> {
    validate_entity_id(&setup_id, "AI setup session")?;
    runtime
        .setup_sessions
        .lock()
        .map_err(|_| CommandError::new("ai_setup_state_unavailable", "AI setup state is unavailable."))?
        .remove(&setup_id);
    let _ = runtime.secrets.clear(&setup_id, CredentialStorage::Session);
    Ok(())
}

fn prune_expired(runtime: &AiRuntimeState) -> Result<(), CommandError> {
    let cutoff = unix_time_millis()?.saturating_sub(SETUP_TTL_MS);
    let expired = {
        let mut sessions = runtime
            .setup_sessions
            .lock()
            .map_err(|_| CommandError::new("ai_setup_state_unavailable", "AI setup state is unavailable."))?;
        let ids = sessions
            .iter()
            .filter(|(_, pending)| pending.created_at < cutoff)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in &ids {
            sessions.remove(id);
        }
        ids
    };
    for id in expired {
        let _ = runtime.secrets.clear(&id, CredentialStorage::Session);
    }
    Ok(())
}
''',
)

# Runtime owns setup metadata; credentials stay in SecretManager's zeroizing
# session store under the temporary setup id.
path = "src-tauri/src/ai/mod.rs"
text = read(path)
text = text.replace("pub(crate) mod profiles;\n", "pub(crate) mod profiles;\npub(crate) mod setup;\n", 1)
text = text.replace("use std::sync::{Arc, Mutex};", "use std::{collections::HashMap, sync::{Arc, Mutex}};", 1)
text = text.replace(
    "    pub(crate) profile_mutations: Mutex<()>,\n",
    "    pub(crate) profile_mutations: Mutex<()>,\n    pub(crate) setup_sessions: Mutex<HashMap<String, setup::PendingAiProviderSetup>>,\n",
    1,
)
text = text.replace(
    "            profile_mutations: Mutex::new(()),\n",
    "            profile_mutations: Mutex::new(()),\n            setup_sessions: Mutex::new(HashMap::new()),\n",
    1,
)
write(path, text)

# Commit a fully selected setup directly: no incomplete profile is durable, and
# the first complete profile becomes the task default in the same SQLite commit.
path = "src-tauri/src/ai/profiles.rs"
text = read(path)
insert_after = '''pub struct ClearAiProviderCredentialInput {
    pub operation_id: String,
    pub profile_id: String,
    pub revision: i64,
}
'''
setup_struct = r'''
#[derive(Clone, Debug)]
pub(crate) struct SetupProfileCommitInput {
    pub profile_id: String,
    pub operation_id: String,
    pub display_name: String,
    pub provider_kind: ProviderKind,
    pub base_url: String,
    pub model_id: String,
    pub auth_mode: AuthMode,
    pub allow_insecure_http: bool,
    pub require_zero_data_retention: bool,
    pub request_timeout_ms: i64,
    pub persistence: CredentialPersistence,
}
'''
if "SetupProfileCommitInput" not in text:
    if insert_after not in text:
        raise RuntimeError(f"{path}: setup struct insertion point missing")
    text = text.replace(insert_after, insert_after + setup_struct, 1)
helper_anchor = "fn create_profile(\n"
helper = r'''pub(crate) fn commit_ai_provider_setup_profile(
    database: &Database,
    runtime: &AiRuntimeState,
    input: SetupProfileCommitInput,
    credential: Option<&str>,
) -> Result<String, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_entity_id(&input.profile_id, "AI provider profile")?;
    let _guard = lock_profile_mutations(runtime)?;
    if let Some(replayed) = database.with_connection(|connection| {
        operation_replay(connection, &input.operation_id, "create_profile", None)
    })? {
        return replayed.ok_or_else(|| {
            CommandError::new("ai_operation_invalid", "The saved setup operation has no profile identity.")
        });
    }

    validate_provider_configuration(input.provider_kind, input.auth_mode)?;
    validate_privacy_configuration(input.provider_kind, input.require_zero_data_retention)?;
    let display_name = validate_display_name(input.display_name)?;
    let normalized_name = normalize_name(&display_name);
    let model_id = validate_model_id(input.model_id)?;
    validate_timeout(input.request_timeout_ms)?;
    let endpoint = validate_endpoint(
        input.provider_kind,
        &input.base_url,
        input.allow_insecure_http,
    )?;
    let storage = if input.auth_mode.requires_credential() {
        if credential.is_none() {
            return Err(CommandError::new(
                "ai_credential_required",
                "This AI service requires an API key.",
            ));
        }
        CredentialStorage::from(input.persistence)
    } else {
        CredentialStorage::None
    };

    database.with_connection(|connection| {
        ensure_profile_capacity(connection)?;
        ensure_name_available(connection, &normalized_name, None)
    })?;

    if let Some(credential) = credential {
        runtime.secrets.set(
            &input.profile_id,
            credential.to_string(),
            input.persistence,
            CredentialStorage::None,
        )?;
    }

    let profile_id = input.profile_id.clone();
    let database_result = database.with_connection(|connection| {
        let now = unix_time_millis()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO ai_provider_profiles (
                     id, display_name, normalized_name, provider_kind, base_url,
                     model_id, auth_mode, enabled, allow_insecure_http,
                     require_zero_data_retention, request_timeout_ms,
                     credential_storage, credential_epoch, revision,
                     created_at, updated_at, deleted_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9, ?10, ?11, ?12, 1, ?13, ?13, NULL)",
                params![
                    input.profile_id,
                    display_name,
                    normalized_name,
                    input.provider_kind.as_str(),
                    endpoint.normalized_url,
                    model_id,
                    input.auth_mode.as_str(),
                    bool_to_integer(input.allow_insecure_http),
                    bool_to_integer(input.require_zero_data_retention),
                    input.request_timeout_ms,
                    storage.as_str(),
                    if input.auth_mode.requires_credential() { 1_i64 } else { 0_i64 },
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        record_operation(
            &transaction,
            &input.operation_id,
            "create_profile",
            Some(&input.profile_id),
            now,
        )?;
        transaction
            .execute(
                "UPDATE ai_task_defaults
                 SET profile_id = CASE WHEN profile_id IS NULL THEN ?1 ELSE profile_id END,
                     updated_at = CASE WHEN profile_id IS NULL THEN ?2 ELSE updated_at END
                 WHERE task_kind = 'flashcard_generation'",
                params![input.profile_id, now],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(profile_id.clone())
    });

    if database_result.is_err() && storage != CredentialStorage::None {
        let _ = runtime.secrets.clear(&input.profile_id, storage);
    }
    database_result
}

'''
if "commit_ai_provider_setup_profile" not in text:
    if helper_anchor not in text:
        raise RuntimeError(f"{path}: create_profile helper anchor missing")
    text = text.replace(helper_anchor, helper + helper_anchor, 1)
write(path, text)

# Register native commands.
path = "src-tauri/src/lib.rs"
text = read(path)
invoke_anchor = "            ai::profiles::get_ai_provider_catalog,\n"
invoke_new = '''            ai::profiles::get_ai_provider_catalog,
            ai::setup::begin_ai_provider_setup,
            ai::setup::commit_ai_provider_setup,
            ai::setup::cancel_ai_provider_setup,
'''
if "ai::setup::begin_ai_provider_setup" not in text:
    if invoke_anchor not in text:
        raise RuntimeError(f"{path}: AI invoke anchor missing")
    text = text.replace(invoke_anchor, invoke_new, 1)
write(path, text)

# Typed frontend IPC. Credentials still cross the WebView boundary only once on
# the explicit write command and are never readable afterward.
path = "src/ipc/aiProfiles.ts"
text = read(path)
insert = '''
export interface AiProviderSetupResult {
  setupId: string;
  models: AiProviderModel[];
}

export async function beginAiProviderSetup(input: {
  providerKind: AiProviderKind;
  baseUrl: string;
  credential: string;
  persistence: AiCredentialPersistence;
  allowInsecureHttp: boolean;
  requireZeroDataRetention: boolean;
  requestTimeoutMs: number;
}): Promise<AiProviderSetupResult> {
  return invoke<AiProviderSetupResult>("begin_ai_provider_setup", { input });
}

export async function commitAiProviderSetup(input: {
  setupId: string;
  displayName: string;
  modelId: string;
}): Promise<AiProviderProfileDetail> {
  return invoke<AiProviderProfileDetail>("commit_ai_provider_setup", { input });
}

export async function cancelAiProviderSetup(setupId: string): Promise<void> {
  return invoke<void>("cancel_ai_provider_setup", { setupId });
}

'''
anchor = "export async function getAiProviderCatalog(): Promise<AiProviderCatalogEntry[]> {"
if "beginAiProviderSetup" not in text:
    if anchor not in text:
        raise RuntimeError(f"{path}: catalog IPC anchor missing")
    text = text.replace(anchor, insert + anchor, 1)
write(path, text)

# Controller: discovery is ephemeral; Save model is the durable commit.
path = "src/controllers/aiProfilesController.ts"
text = read(path)
if "beginAiProviderSetup" not in text:
    text = text.replace(
        "  clearAiProviderCredential,\n",
        "  beginAiProviderSetup,\n  cancelAiProviderSetup,\n  clearAiProviderCredential,\n  commitAiProviderSetup,\n",
        1,
    )
text = text.replace("  probeResult: AiProviderProbeResult | null;\n", "  probeResult: AiProviderProbeResult | null;\n  setupId: string | null;\n", 1)
state_anchor = "  const [probeResult, setProbeResult] = useState<AiProviderProbeResult | null>(null);\n"
if "const [setupId, setSetupId]" not in text:
    text = text.replace(state_anchor, state_anchor + '  const [setupId, setSetupId] = useState<string | null>(null);\n', 1)

# Replace the generated saveSetup implementation completely.
pattern = re.compile(r"  async function saveSetup\(\): Promise<void> \{.*?\n  \}\n\n  async function saveProfile", re.S)
replacement = r'''  async function saveSetup(): Promise<void> {
    if (!draft || mutating) return;
    const provider = catalog.find((candidate) => candidate.providerKind === draft.providerKind);
    if (!provider) return;
    setMutating(true);
    setErrorMessage(null);
    try {
      if (selectedProfile) {
        const normalizedDraft: AiProfileDraft = {
          ...draft,
          enabled: true,
          authMode:
            draft.providerKind === "llama_cpp"
              ? credentialDraft.trim() === "" && selectedProfile.authMode !== "bearer"
                ? "none"
                : "bearer"
              : "bearer",
        };
        let saved = await updateAiProviderProfile(
          selectedProfile.id,
          selectedProfile.revision,
          normalizedDraft,
        );
        if (credentialDraft.trim() !== "") {
          saved = await setAiProviderCredential(
            saved.id,
            saved.revision,
            credentialDraft,
            credentialPersistence,
          );
        }
        setSelectedProfile(saved);
        setDraft(draftFromProfile(saved));
        setCredentialDraft("");
        setProbeResult(null);
        await loadProfiles();
        showToast("success", "AI model settings saved.");
        if (canDiscoverModels(saved)) await discoverModelsForProfile(saved, true);
        return;
      }

      if (setupId) await cancelAiProviderSetup(setupId);
      const result = await beginAiProviderSetup({
        providerKind: draft.providerKind,
        baseUrl: draft.baseUrl,
        credential: credentialDraft,
        persistence: credentialPersistence,
        allowInsecureHttp: draft.allowInsecureHttp,
        requireZeroDataRetention: draft.requireZeroDataRetention ?? false,
        requestTimeoutMs: draft.requestTimeoutMs,
      });
      setSetupId(result.setupId);
      setModels(result.models);
      setModelsLoaded(true);
      setCredentialDraft("");
      if (result.models.length === 1 && result.models[0]) {
        setDraft((current) => current ? { ...current, modelId: result.models[0]!.id } : current);
      }
      showToast("success", "Connected to the AI service. Choose a model to finish setup.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setMutating(false);
    }
  }

  async function saveProfile'''
text, count = pattern.subn(replacement, text, count=1)
if count != 1:
    raise RuntimeError(f"{path}: generated saveSetup implementation not found")

# New-profile save commits the native setup before ordinary saved-profile edits.
save_marker = '''  async function saveProfile(): Promise<void> {
    if (!draft || mutating) return;
    setMutating(true);
    setErrorMessage(null);
    try {
      const wasExisting = selectedProfile !== null;
      const saved = selectedProfile
        ? await updateAiProviderProfile(selectedProfile.id, selectedProfile.revision, draft)
        : await createAiProviderProfile(draft);'''
new_save = '''  async function saveProfile(): Promise<void> {
    if (!draft || mutating) return;
    setMutating(true);
    setErrorMessage(null);
    try {
      const wasExisting = selectedProfile !== null;
      const saved = !selectedProfile && setupId
        ? await commitAiProviderSetup({
            setupId,
            displayName: draft.displayName,
            modelId: draft.modelId,
          })
        : selectedProfile
          ? await updateAiProviderProfile(selectedProfile.id, selectedProfile.revision, draft)
          : await createAiProviderProfile(draft);'''
if save_marker not in text:
    raise RuntimeError(f"{path}: saveProfile marker missing")
text = text.replace(save_marker, new_save, 1)
text = text.replace(
    "      setSelectedProfile(saved);\n      setDraft(draftFromProfile(saved));\n      setSelectedProfileId(saved.id);",
    "      setSelectedProfile(saved);\n      setSetupId(null);\n      setDraft(draftFromProfile(saved));\n      setSelectedProfileId(saved.id);",
    1,
)
# Native commit may assign the first default; reload it instead of issuing a second mutation.
text = text.replace(
    '''      if (!defaults.flashcardGenerationProfileId && saved.modelId) {
        const selectedDefaults = await setFlashcardGenerationProfile(saved.id);
        setDefaults(selectedDefaults);
      }''',
    '''      if (!defaults.flashcardGenerationProfileId && saved.modelId) {
        setDefaults(await getAiTaskDefaults());
      }''',
    1,
)
# Clear/cancel ephemeral setup when changing context.
text = text.replace(
    "  function selectProfile(profileId: string): void {\n",
    "  function selectProfile(profileId: string): void {\n    if (setupId) void cancelAiProviderSetup(setupId);\n    setSetupId(null);\n",
    1,
)
text = text.replace(
    "  function beginCreate(providerKind: AiProviderKind): void {\n",
    "  function beginCreate(providerKind: AiProviderKind): void {\n    if (setupId) void cancelAiProviderSetup(setupId);\n    setSetupId(null);\n",
    1,
)
text = text.replace(
    "  function discardDraft(): void {\n",
    "  function discardDraft(): void {\n    if (setupId) void cancelAiProviderSetup(setupId);\n    setSetupId(null);\n",
    1,
)
# A discovered-but-uncommitted model counts as dirty and must be guarded on navigation.
text = text.replace(
    "  const dirty = credentialDraft !== \"\" || profileDraftDirty;",
    "  const dirty = credentialDraft !== \"\" || profileDraftDirty || setupId !== null;",
    1,
)
# Publish setup id through controller.
text = text.replace("    probeResult,\n", "    probeResult,\n    setupId,\n", 1)
write(path, text)

# UI exposes model selection after ephemeral discovery even though there is no
# durable profile yet. Connection testing remains available only after commit.
path = "src/screens/settings/AiLlmSettings.tsx"
text = read(path)
text = text.replace("  const modelReady = Boolean(profile && draft?.modelId);", "  const modelReady = Boolean(profile && draft?.modelId);")
text = text.replace("              {!profile ? (", "              {!profile && !controller.setupId ? (", 1)
text = text.replace("              {profile ? (\n                <div className={aiStyles.modelStep}>", "              {profile || controller.setupId ? (\n                <div className={aiStyles.modelStep}>", 1)
text = text.replace(
    "                        !draft.modelId ||\n                        draft.modelId === profile.modelId",
    "                        !draft.modelId ||\n                        (profile !== null && draft.modelId === profile.modelId)",
    1,
)
text = text.replace(
    "                    <button\n                      type=\"button\"\n                      className=\"secondary-button\"\n                      disabled={\n                        controller.mutating ||\n                        controller.refreshingModels ||\n                        controller.dirty\n                      }\n                      onClick={() => void controller.refreshModels()}\n                    >\n                      {controller.refreshingModels ? \"Retrieving…\" : \"Refresh models\"}\n                    </button>",
    "                    {profile ? (\n                      <button\n                        type=\"button\"\n                        className=\"secondary-button\"\n                        disabled={\n                          controller.mutating ||\n                          controller.refreshingModels ||\n                          controller.dirty\n                        }\n                        onClick={() => void controller.refreshModels()}\n                      >\n                        {controller.refreshingModels ? \"Retrieving…\" : \"Refresh models\"}\n                      </button>\n                    ) : null}",
    1,
)
text = text.replace(
    "placeholder={credentialPlaceholder(profile?.credentialStatus, requiresKey)}",
    'placeholder={controller.setupId ? "Held securely for this setup" : credentialPlaceholder(profile?.credentialStatus, requiresKey)}',
    1,
)
write(path, text)

# Frontend tests/mock interfaces gain setup-session members. Existing tests that
# mock the controller remain source-compatible through explicit no-op stubs.
for test_path in [
    "src/screens/settings/AiLlmSettings.test.tsx",
    "src/screens/settings/DefaultAiModelSettings.test.tsx",
]:
    p = ROOT / test_path
    if not p.exists():
        continue
    test = p.read_text()
    test = test.replace("probeResult: null, dirty:", "probeResult: null, setupId: null, dirty:")
    p.write_text(test)

# Extend IPC contract tests with command names rather than credential contents.
path = "src/ipc/aiProfiles.test.ts"
text = read(path)
if "begin_ai_provider_setup" not in text:
    text = text.replace(
        'import {',
        'import {\n  beginAiProviderSetup,\n  cancelAiProviderSetup,\n  commitAiProviderSetup,',
        1,
    )
    anchor = "describe(\"aiProfiles IPC\", () => {\n"
    test = '''  it("uses native ephemeral setup commands", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ setupId: "a".repeat(32), models: [] });
    await beginAiProviderSetup({
      providerKind: "llama_cpp",
      baseUrl: "http://127.0.0.1:8080/",
      credential: "",
      persistence: "session",
      allowInsecureHttp: false,
      requireZeroDataRetention: false,
      requestTimeoutMs: 60_000,
    });
    expect(invoke).toHaveBeenLastCalledWith("begin_ai_provider_setup", expect.any(Object));
    vi.mocked(invoke).mockResolvedValueOnce({ id: "b".repeat(32) });
    await commitAiProviderSetup({ setupId: "a".repeat(32), displayName: "Local", modelId: "model" });
    expect(invoke).toHaveBeenLastCalledWith("commit_ai_provider_setup", expect.any(Object));
    vi.mocked(invoke).mockResolvedValueOnce(undefined);
    await cancelAiProviderSetup("a".repeat(32));
    expect(invoke).toHaveBeenLastCalledWith("cancel_ai_provider_setup", { setupId: "a".repeat(32) });
  });

'''
    if anchor not in text:
        raise RuntimeError(f"{path}: describe anchor missing")
    text = text.replace(anchor, anchor + test, 1)
write(path, text)

# Architecture docs make the durable boundary explicit.
path = "docs/architecture/ai-and-document-processing.md"
text = read(path)
section = '''
### Ephemeral provider setup

The normal Service → API key/server → Model wizard uses a native in-memory setup session. Model discovery runs with temporary connection metadata and a zeroizing session-only credential keyed by the setup ID. No `ai_provider_profiles` row exists until a model is selected. Committing setup writes the complete profile and, when applicable, its credential epoch; canceling or expiration clears the temporary secret. The first complete profile becomes the flashcard-generation default only when no default already exists.
'''
if "### Ephemeral provider setup" not in text:
    text += section
write(path, text)
