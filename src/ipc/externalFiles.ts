import { invoke } from "./transport";
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
  | "anki_apkg"
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
  purpose: "studydeck" | "library_backup" | "study_guide_pdf" | "collection_study_guide_pdf" | "lecture_pptx" | "annotated_pdf";
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

/** A chosen file that could not be staged, and why. */
export interface SkippedExternalFile {
  displayName: string;
  message: string;
}

export interface StagedExternalFiles {
  files: StagedExternalFile[];
  skipped: SkippedExternalFile[];
}

/**
 * Lets the person choose several files at once and stages each one, in the
 * order chosen. Empty when they cancel. Where the picker takes one file at a
 * time, at most one comes back.
 */
export async function selectAndStageExternalImports(
  purpose: ExternalImportPurpose,
  operationId = createOperationId(),
): Promise<StagedExternalFiles> {
  return invoke<StagedExternalFiles>("stream_select_and_stage_external_imports", {
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
