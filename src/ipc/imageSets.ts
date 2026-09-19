import { invoke } from "@tauri-apps/api/core";

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
