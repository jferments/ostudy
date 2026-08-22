import { invoke } from "@tauri-apps/api/core";
import { createOperationId } from "./common";

export type ExternalImportPurpose =
  | "image"
  | "pdf_document"
  | "document"
  | "study_content"
  | "library_backup";

export type ExternalFileKind =
  | "png"
  | "jpeg"
  | "gif"
  | "webp"
  | "pdf"
  | "doc"
  | "docx"
  | "ppt"
  | "pptx"
  | "odt"
  | "odp"
  | "rtf"
  | "epub"
  | "excel"
  | "ods"
  | "csv"
  | "studydeck"
  | "generated_json"
  | "ostudybackup";

export interface StagedExternalFile {
  id: string;
  purpose: ExternalImportPurpose;
  detectedKind: ExternalFileKind;
  displayName: string;
  mediaType: string;
  byteSize: number;
  sha256: string;
  expiresAt: number;
}

export interface PreparedExport {
  id: string;
  purpose: "studydeck" | "library_backup";
  suggestedFilename: string;
  mediaType: string;
  byteSize: number;
  sha256: string;
  expiresAt: number;
}

export interface PublishReceipt {
  preparedExportId: string;
  displayName: string;
  destinationKind: string;
  completedAt: number;
}

export interface ShareLaunch {
  preparedExportId: string;
  launched: boolean;
  destinationKind: string;
  launchedAt: number;
}

export interface IncomingShareSummary {
  token: string;
  displayName: string;
  reportedMediaType: string | null;
  byteSize: number;
}

export async function selectAndStageExternalImport(
  purpose: ExternalImportPurpose,
  operationId = createOperationId(),
): Promise<StagedExternalFile | null> {
  return invoke<StagedExternalFile | null>("stream_select_and_stage_external_import", {
    input: { operationId, purpose },
  });
}

export async function releaseStagedExternalImport(stagedFileId: string): Promise<void> {
  return invoke<void>("release_staged_external_import", { input: { stagedFileId } });
}

export async function publishPreparedExport(
  preparedExportId: string,
): Promise<PublishReceipt | null> {
  return invoke<PublishReceipt | null>("stream_publish_prepared_export", {
    input: { preparedExportId },
  });
}

export async function sharePreparedExport(preparedExportId: string): Promise<ShareLaunch> {
  return invoke<ShareLaunch>("share_prepared_export", {
    input: { preparedExportId },
  });
}

export async function releasePreparedExport(preparedExportId: string): Promise<void> {
  return invoke<void>("release_prepared_export", { input: { preparedExportId } });
}

export async function listIncomingShares(): Promise<IncomingShareSummary[]> {
  return invoke<IncomingShareSummary[]>("list_incoming_shares");
}

export async function stageIncomingShare(
  token: string,
  operationId = createOperationId(),
): Promise<StagedExternalFile> {
  return invoke<StagedExternalFile>("stage_incoming_share", {
    input: { token, operationId },
  });
}

export async function dismissIncomingShare(token: string): Promise<void> {
  return invoke<void>("dismiss_incoming_share", { input: { token } });
}
