import {
  publishPreparedExport,
  selectAndStageExternalImport,
  sharePreparedExport,
  type ExternalImportPurpose,
  type PublishReceipt,
  type ShareLaunch,
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

export function shareExport(preparedExportId: string): Promise<ShareLaunch> {
  return sharePreparedExport(preparedExportId);
}
