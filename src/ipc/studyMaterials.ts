import { invoke } from "@tauri-apps/api/core";

export type StudyTargetKind = "document" | "collection";
export type StudyTargetArtifactKind = "flashcards" | "study_guide";

export interface StudyTargetRef {
  kind: StudyTargetKind;
  id: string;
}

export interface StudyTargetOutputPolicy {
  flashcards: boolean;
  studyGuide: boolean;
}

export interface StudyTargetArtifactStatus {
  kind: StudyTargetArtifactKind;
  state: string;
  artifactId: string | null;
  totalUnits: number;
  completedUnits: number;
  safeErrorCode: string;
  safeErrorMessage: string;
}

export interface StudyTargetBundleStatus {
  targetId: string;
  target: StudyTargetRef;
  title: string;
  revision: number;
  sourceSnapshotId: string;
  sourceChanged: boolean;
  deckId: string | null;
  guideId: string | null;
  guideRevision: number | null;
  latestOperationId: string | null;
  latestBatchState: string | null;
  flashcards: StudyTargetArtifactStatus;
  studyGuide: StudyTargetArtifactStatus;
}

export interface GetStudyTargetBundleInput {
  target: StudyTargetRef;
}

export interface BeginStudyTargetBundleInput {
  operationId: string;
  target: StudyTargetRef;
  outputPolicy?: StudyTargetOutputPolicy;
  recreateArtifacts?: StudyTargetArtifactKind[];
}

export async function getStudyTargetBundle(
  input: GetStudyTargetBundleInput,
): Promise<StudyTargetBundleStatus | null> {
  return invoke<StudyTargetBundleStatus | null>("get_study_target_bundle", { input });
}

export async function beginStudyTargetBundle(
  input: BeginStudyTargetBundleInput,
): Promise<StudyTargetBundleStatus> {
  return invoke<StudyTargetBundleStatus>("begin_study_target_bundle", { input });
}

export interface ComposeStudyTargetBundleInput {
  operationId: string;
}

export async function composeStudyTargetBundle(
  input: ComposeStudyTargetBundleInput,
): Promise<StudyTargetBundleStatus> {
  return invoke<StudyTargetBundleStatus>("compose_study_target_bundle", { input });
}
