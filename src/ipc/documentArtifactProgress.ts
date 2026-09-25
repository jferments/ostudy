import { invoke } from "@tauri-apps/api/core";
import type { DocumentArtifactJob } from "./studyGuides";

/** One durable latest-job snapshot, not one IPC call per document or topic. */
export async function listLatestDocumentStudyGuideJobs(): Promise<DocumentArtifactJob[]> {
  return invoke<DocumentArtifactJob[]>("list_latest_document_study_guide_jobs");
}
