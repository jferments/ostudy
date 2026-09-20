import { invoke } from "@tauri-apps/api/core";
import type { StagedExternalFile } from "./externalFiles";

/** Browser-granted bytes only; never accept a local path or fetch a remote URL. */
export async function stageImageCollectionFile(input: {
  operationId: string;
  displayName: string;
  dataBase64: string;
}): Promise<StagedExternalFile> {
  return invoke<StagedExternalFile>("stage_image_collection_file", { input });
}
