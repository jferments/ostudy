import {
  publishPreparedExport,
  selectAndStageExternalImport,
  type ExternalImportPurpose,
  type PublishReceipt,
  type StagedExternalFile,
} from "../ipc/externalFiles";

export function selectExternalImport(
  purpose: ExternalImportPurpose,
): Promise<StagedExternalFile | null> {
  return selectAndStageExternalImport(purpose);
}

export function publishExport(preparedExportId: string): Promise<PublishReceipt | null> {
  return publishPreparedExport(preparedExportId);
}
