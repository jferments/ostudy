import { invoke } from "@tauri-apps/api/core";

import { createOperationId } from "./common";
import type { EditorDraftCommitRef } from "./drafts";

export type AiProviderKind =
  | "llama_cpp"
  | "deepseek"
  | "openrouter"
  | "ollama"
  | "openai"
  | "gemini"
  | "claude";
export type AiAuthMode = "none" | "bearer";
export type AiNetworkScope = "local" | "lan" | "cloud";
export type AiCredentialPersistence = "secure" | "session";
export type AiCredentialStatus =
  | "not_required"
  | "missing"
  | "saved_securely"
  | "session_only"
  | "secure_store_unavailable";
export type AiProfileConfigurationStatus = "ready" | "incomplete" | "disabled" | "planned";
export type AiProbeOverallStatus = "verified" | "partial" | "failed";
export type AiProbePhaseStatus = "passed" | "failed" | "skipped";
export type AiCapabilityStatus = "supported" | "unsupported" | "unknown";
export type AiCapabilitySource = "declared" | "probed";
export type AiReasoningEffort =
  | "automatic"
  | "none"
  | "minimal"
  | "low"
  | "medium"
  | "high"
  | "xhigh"
  | "max";

export interface AiProviderCatalogEntry {
  providerKind: AiProviderKind;
  displayName: string;
  description: string;
  implementationStatus: "available" | "planned";
  defaultBaseUrl: string;
  baseUrlEditable: boolean;
  authenticationRequired: boolean;
  modelIdRequired: boolean;
  modelDiscoverySupported: boolean;
  chatTemplatePolicy: "server_model" | "provider_managed";
  defaultTimeoutMs: number;
}

export interface AiCredentialStatusInfo {
  status: AiCredentialStatus;
  authenticationRequired: boolean;
  securePersistenceAvailable: boolean;
  sessionOnlyAvailable: boolean;
}

export interface AiProviderProfileSummary {
  id: string;
  displayName: string;
  providerKind: AiProviderKind;
  providerLabel: string;
  modelId: string;
  /** Current native responses always include this; optionality preserves old test/import fixtures. */
  reasoningEffort?: AiReasoningEffort;
  enabled: boolean;
  networkScope: AiNetworkScope;
  credentialStatus: AiCredentialStatusInfo;
  configurationStatus: AiProfileConfigurationStatus;
  revision: number;
  updatedAt: number;
}

export interface AiProviderProfileDetail extends AiProviderProfileSummary {
  baseUrl: string;
  baseUrlEditable: boolean;
  authMode: AiAuthMode;
  allowInsecureHttp: boolean;
  requireZeroDataRetention?: boolean;
  requestTimeoutMs: number;
  credentialEpoch: number;
  createdAt: number;
}

export interface AiProfileDraft {
  displayName: string;
  providerKind: AiProviderKind;
  baseUrl: string;
  modelId: string;
  reasoningEffort: AiReasoningEffort;
  authMode: AiAuthMode;
  enabled: boolean;
  allowInsecureHttp: boolean;
  requireZeroDataRetention?: boolean;
  requestTimeoutMs: number;
}

export type AiProfileWriteInput = Omit<AiProfileDraft, "reasoningEffort"> & {
  reasoningEffort?: AiReasoningEffort;
  editorDraft?: EditorDraftCommitRef;
};

export interface AiTaskDefaults {
  flashcardGenerationProfileId: string | null;
}

export interface AiProviderModelReasoning {
  supportedEfforts: Exclude<AiReasoningEffort, "automatic">[] | null;
  defaultEffort: Exclude<AiReasoningEffort, "automatic"> | null;
  defaultEnabled: boolean | null;
  supportsMaxTokens: boolean;
  mandatory: boolean;
}

export interface AiProviderModel {
  id: string;
  displayName: string;
  ownedBy: string | null;
  contextWindowTokens?: number | null;
  reasoning?: AiProviderModelReasoning | null;
}

export interface AiProviderModels {
  profileId: string;
  profileRevision: number;
  providerKind: AiProviderKind;
  models: AiProviderModel[];
  retrievedAt: number;
}

export interface SafeAiError {
  code: string;
  message: string;
  retryable: boolean;
  retryAfterSeconds: number | null;
}

export interface AiProbePhase {
  phase: string;
  label: string;
  status: AiProbePhaseStatus;
  message: string;
  latencyMs: number | null;
  error: SafeAiError | null;
}

export interface AiCapability {
  key: string;
  status: AiCapabilityStatus;
  source: AiCapabilitySource;
  value: unknown | null;
}

export interface AiProviderProbeResult {
  profileId: string;
  profileRevision: number;
  credentialEpoch: number;
  testedModelId: string;
  adapterRevision: number;
  resultSchemaVersion: number;
  overallStatus: AiProbeOverallStatus;
  phases: AiProbePhase[];
  capabilities: AiCapability[];
  testedAt: number;
  persisted: boolean;
  stale: boolean;
}

export interface DeleteAiProviderProfileResult {
  profileId: string;
  deleted: boolean;
}

export async function getAiProviderCatalog(): Promise<AiProviderCatalogEntry[]> {
  return invoke<AiProviderCatalogEntry[]>("get_ai_provider_catalog");
}

export async function listAiProviderProfiles(): Promise<AiProviderProfileSummary[]> {
  return invoke<AiProviderProfileSummary[]>("list_ai_provider_profiles");
}

export async function getAiProviderProfile(profileId: string): Promise<AiProviderProfileDetail> {
  return invoke<AiProviderProfileDetail>("get_ai_provider_profile", { profileId });
}

export async function createAiProviderProfile(
  input: AiProfileWriteInput,
  operationId = createOperationId(),
): Promise<AiProviderProfileDetail> {
  return invoke<AiProviderProfileDetail>("create_ai_provider_profile", {
    input: { ...input, reasoningEffort: input.reasoningEffort ?? "low", operationId },
  });
}

export async function updateAiProviderProfile(
  profileId: string,
  revision: number,
  input: AiProfileWriteInput,
  operationId = createOperationId(),
): Promise<AiProviderProfileDetail> {
  return invoke<AiProviderProfileDetail>("update_ai_provider_profile", {
    input: {
      operationId,
      profileId,
      revision,
      displayName: input.displayName,
      baseUrl: input.baseUrl,
      modelId: input.modelId,
      reasoningEffort: input.reasoningEffort ?? "low",
      authMode: input.authMode,
      enabled: input.enabled,
      allowInsecureHttp: input.allowInsecureHttp,
      requireZeroDataRetention: input.requireZeroDataRetention ?? false,
      requestTimeoutMs: input.requestTimeoutMs,
      editorDraft: input.editorDraft,
    },
  });
}

export async function deleteAiProviderProfile(
  profileId: string,
  revision: number,
  operationId = createOperationId(),
): Promise<DeleteAiProviderProfileResult> {
  return invoke<DeleteAiProviderProfileResult>("delete_ai_provider_profile", {
    input: { profileId, revision, operationId },
  });
}

export async function setAiProviderCredential(
  profileId: string,
  revision: number,
  credential: string,
  persistence: AiCredentialPersistence,
  operationId = createOperationId(),
): Promise<AiProviderProfileDetail> {
  return invoke<AiProviderProfileDetail>("set_ai_provider_credential", {
    input: { profileId, revision, credential, persistence, operationId },
  });
}

export async function clearAiProviderCredential(
  profileId: string,
  revision: number,
  operationId = createOperationId(),
): Promise<AiProviderProfileDetail> {
  return invoke<AiProviderProfileDetail>("clear_ai_provider_credential", {
    input: { profileId, revision, operationId },
  });
}

export async function getAiTaskDefaults(): Promise<AiTaskDefaults> {
  return invoke<AiTaskDefaults>("get_ai_task_defaults");
}

export async function setFlashcardGenerationProfile(
  profileId: string | null,
  operationId = createOperationId(),
): Promise<AiTaskDefaults> {
  return invoke<AiTaskDefaults>("set_ai_task_default", {
    input: { taskKind: "flashcard_generation", profileId, operationId },
  });
}

export async function listAiProviderModels(
  profileId: string,
  revision: number,
): Promise<AiProviderModels> {
  return invoke<AiProviderModels>("list_ai_provider_models", {
    input: { profileId, revision },
  });
}

export async function testAiProviderProfile(
  profileId: string,
  revision: number,
): Promise<AiProviderProbeResult> {
  return invoke<AiProviderProbeResult>("test_ai_provider_profile", {
    input: { profileId, revision },
  });
}

export async function getAiProviderProbeResult(
  profileId: string,
): Promise<AiProviderProbeResult | null> {
  return invoke<AiProviderProbeResult | null>("get_ai_provider_probe_result", { profileId });
}
