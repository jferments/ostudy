import type { AnnotationShape } from "./documentAnnotations";
import { invoke } from "./transport";

/** What marks are drawn on: a lesson's lecture slides or a document study guide's designed pages. */
export type InkMaterial =
  | { kind: "lesson"; lessonId: string }
  | { kind: "studyGuide"; documentId: string };

export interface InkMark {
  id: string;
  /** The slide key, or the guide section's topic id. */
  pageKey: string;
  shape: AnnotationShape;
  revision: number;
  createdAt: number;
  updatedAt: number;
}

/**
 * The new state of one mark. A put creates it or replaces all of it; deleting
 * one that is already gone succeeds, so a request can be repeated.
 */
export type InkChange =
  | { op: "put"; id: string; pageKey: string; shape: AnnotationShape }
  | { op: "delete"; id: string };

export interface AppliedInkChanges {
  marks: InkMark[];
  deletedIds: string[];
}

/** Every mark on the material, earlier marks first. */
export async function getMaterialInk(material: InkMaterial): Promise<InkMark[]> {
  const result = await invoke<{ marks: InkMark[] }>("get_material_ink", { input: { material } });
  return result.marks;
}

export async function applyMaterialInkChanges(material: InkMaterial, changes: InkChange[]): Promise<AppliedInkChanges> {
  return invoke<AppliedInkChanges>("apply_material_ink_changes", { input: { material, changes } });
}
