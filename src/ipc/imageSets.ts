import { invoke } from "./transport";

export interface InspectedImage {
  originalFilename: string;
  mediaType: string;
  byteSize: number;
  sha256: string;
  width: number;
  height: number;
  pixelSha256: string;
  /** Region coordinates refer to these native-normalized, EXIF-upright pixels. */
  pixelContract: string;
}

export interface ImageSelectionInspection {
  /** Opaque selection identity; equal bytes or filenames do not merge identities. */
  id: string;
  image: InspectedImage | null;
  error: { code: string; message: string } | null;
}

export interface ImageSetInspection {
  maximumCount: number;
  maximumSetBytes: number;
  maximumSetPixels: number;
  members: ImageSelectionInspection[];
  /** Preview guidance only. Source publication must repeat native admission. */
  canImport: boolean;
  decodedImages: number;
  reusedImages: number;
}

/** Read-only inspection. This does not consume selection leases or start AI work. */
export async function inspectImageSet(
  stagedFileIds: readonly string[],
): Promise<ImageSetInspection> {
  return invoke<ImageSetInspection>("inspect_image_set", {
    input: { stagedFileIds: [...stagedFileIds] },
  });
}

export interface ImageSetInclusionCrop {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface ImportImageSetInput {
  /** Retain this identity when recovering an interrupted acknowledgement. */
  operationId: string;
  title: string;
  members: {
    stagedFileId: string;
    quarterTurns: 0 | 1 | 2 | 3;
    /** Source inclusion in native upright coordinates, not a display-only crop. */
    inclusion: ImageSetInclusionCrop | null;
  }[];
}

export interface ImportedImageSet {
  documentId: string;
  revision: number;
}

/** Retain originals and the ordered manifest; no OCR or generation is started. */
export async function importImageSet(input: ImportImageSetInput): Promise<ImportedImageSet> {
  return invoke<ImportedImageSet>("import_image_set", { input });
}

export type ImageRegionRole = "learning" | "metadata" | "interface" | "notice" | "outside_selection" | "unresolved";
export type ImageRegionOverride = "include" | "exclude" | "metadata";

export interface ImageRegionDecision {
  id: string;
  text: string;
  bounds: ImageSetInclusionCrop | null;
  automaticRole: ImageRegionRole;
  role: ImageRegionRole;
  reason: string;
  overrideAction: ImageRegionOverride | null;
  equivalentTo: string | null;
}

export interface ImageMemberProcessing {
  memberId: string;
  width: number;
  height: number;
  ocrAvailable: boolean;
  visualWithheld: boolean;
  /** Bounds are source pixels, not normalized crop coordinates. */
  regions: ImageRegionDecision[];
}

export interface ImageMemberDetails {
  id: string;
  originalFilename: string;
  mediaType: string;
  byteSize: number;
  width: number;
  height: number;
  quarterTurns: 0 | 1 | 2 | 3;
  inclusion: ImageSetInclusionCrop | null;
  processing: ImageMemberProcessing | null;
}

export interface ImageSetDetails {
  documentId: string;
  title: string;
  revision: number;
  filterRevision: number;
  includeInterface: boolean;
  prepared: boolean;
  members: ImageMemberDetails[];
}

export interface ImageSetEditResult extends ImportedImageSet {
  filterRevision: number;
}

export interface SetImageFiltersInput {
  operationId: string;
  documentId: string;
  expectedRevision: number;
  expectedFilterRevision: number;
  includeInterface: boolean;
  overrides: { regionId: string; action: ImageRegionOverride | null }[];
}

export async function getImageSet(documentId: string): Promise<ImageSetDetails> {
  return invoke<ImageSetDetails>("get_image_set", { input: { documentId } });
}

/** Local OCR and projection only; does not select or contact an AI provider. */
export async function prepareImageSet(documentId: string, retryUnreadable = false): Promise<ImageSetEditResult> {
  return invoke<ImageSetEditResult>("prepare_image_set", { input: { documentId, retryUnreadable } });
}

export async function setImageSetFilters(input: SetImageFiltersInput): Promise<ImageSetEditResult> {
  return invoke<ImageSetEditResult>("set_image_set_filters", { input });
}

export interface ImageSetPreview {
  revision: number;
  width: number;
  height: number;
  dataUrl: string;
}

/** The viewport changes only the inspection view, never source inclusion. */
export async function getImageSetPreview(input: {
  documentId: string;
  memberId: string;
  expectedRevision: number;
  viewport: ImageSetInclusionCrop | null;
}): Promise<ImageSetPreview> {
  return invoke<ImageSetPreview>("get_image_set_preview", { input });
}

export interface AmendImageSetInput {
  operationId: string;
  documentId: string;
  expectedRevision: number;
  expectedFilterRevision: number;
  /** All retained member identities, once each, in the requested order. */
  members: {
    memberId: string;
    quarterTurns: 0 | 1 | 2 | 3;
    inclusion: ImageSetInclusionCrop | null;
  }[];
  append: ImportImageSetInput["members"];
}

/** Save source edits without performing OCR, interpretation, or generation. */
export async function amendImageSet(input: AmendImageSetInput): Promise<ImageSetEditResult> {
  return invoke<ImageSetEditResult>("amend_image_set", { input });
}
