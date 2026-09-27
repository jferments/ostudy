import { invoke } from "./transport";
import type { StagedExternalFile } from "./externalFiles";

/** Browser-granted bytes only; never accept a local path or fetch a remote URL. */
export async function stageImageCollectionFile(input: {
  operationId: string;
  displayName: string;
  dataBase64: string;
}): Promise<StagedExternalFile> {
  return invoke<StagedExternalFile>("stage_image_collection_file", { input });
}

export async function stageImageCollectionFileChunk(input: {
  operationId: string;
  displayName: string;
  totalBytes: number;
  offset: number;
  dataBase64: string;
  finalChunk: boolean;
}): Promise<StagedExternalFile | null> {
  return invoke<StagedExternalFile | null>("stage_image_collection_file_chunk", { input });
}
