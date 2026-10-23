import type { AnnotationShape } from "./documentAnnotations";
import { invoke } from "./transport";

/** What marks are drawn on: a lesson's lecture slides or a document study guide's PDF pages. */
export type InkMaterial =
  | { kind: "lesson"; lessonId: string }
  | { kind: "studyGuide"; documentId: string };

export interface InkMark {
  id: string;
  /** The slide key, or the guide page's key (a section's topic id on older designed section pages). */
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

export interface MaterialInk {
  /** Earlier marks first. */
  marks: InkMark[];
  /** Links on a guide's pages whose document, page, guide, deck or notes page is gone. */
  unavailableLinkIds: string[];
}

/** Every mark on the material, earlier marks first. */
export async function getMaterialInk(material: InkMaterial): Promise<InkMark[]> {
  return (await readMaterialInk(material)).marks;
}

/** The material's marks and which of its links lead nowhere now. */
export async function readMaterialInk(material: InkMaterial): Promise<MaterialInk> {
  return invoke<MaterialInk>("get_material_ink", { input: { material } });
}

export async function applyMaterialInkChanges(material: InkMaterial, changes: InkChange[]): Promise<AppliedInkChanges> {
  return invoke<AppliedInkChanges>("apply_material_ink_changes", { input: { material, changes } });
}
