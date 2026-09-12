import { invoke } from "@tauri-apps/api/core";

export type EmbeddingInputPolicy = "automatic" | "symmetric" | "qwen_retrieval" | "search_types";

export interface EmbeddingOptions {
  profileId: string;
  modelId: string;
  dimensions: number | null;
  inputPolicy: EmbeddingInputPolicy;
  revision: number;
}

export interface EmbeddingModelChoice {
  id: string;
  name: string;
}

export interface EmbeddingTestResult {
  profileId: string;
  modelId: string;
  profileRevision: number;
  optionsRevision: number;
  dimensions: number;
  spaceId: string;
  queryAndDocumentTested: boolean;
}

export async function getEmbeddingOptions(profileId: string, modelId: string): Promise<EmbeddingOptions> {
  return invoke<EmbeddingOptions>("get_embedding_options", { profileId, modelId });
}

export async function saveEmbeddingOptions(input: {
  profileId: string;
  modelId: string;
  expectedRevision: number;
  dimensions: number | null;
  inputPolicy: EmbeddingInputPolicy;
}): Promise<EmbeddingOptions> {
  return invoke<EmbeddingOptions>("save_embedding_options", { input });
}

export async function listEmbeddingModels(profileId: string): Promise<EmbeddingModelChoice[]> {
  return invoke<EmbeddingModelChoice[]>("list_embedding_models", {
    input: { profileId, modelId: null },
  });
}

/** This probe sends only built-in non-personal query/document fixtures. */
export async function testEmbeddingProfile(
  profileId: string,
  modelId: string,
): Promise<EmbeddingTestResult> {
  return invoke<EmbeddingTestResult>("test_embedding_profile", {
    input: { profileId, modelId },
  });
}
