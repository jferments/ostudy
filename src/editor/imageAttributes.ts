export const IMAGE_SIZES = ["small", "medium", "large", "full", "original"] as const;
export const IMAGE_ALIGNMENTS = ["left", "center", "right"] as const;

export type ImageSize = (typeof IMAGE_SIZES)[number];
export type ImageAlignment = (typeof IMAGE_ALIGNMENTS)[number];

export function normalizeImageSize(value: unknown): ImageSize {
  return IMAGE_SIZES.includes(value as ImageSize) ? (value as ImageSize) : "medium";
}

export function normalizeImageAlignment(value: unknown): ImageAlignment {
  return IMAGE_ALIGNMENTS.includes(value as ImageAlignment)
    ? (value as ImageAlignment)
    : "center";
}
