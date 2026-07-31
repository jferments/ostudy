import { convertFileSrc } from "@tauri-apps/api/core";

import type { Asset, RichDocument } from "../ipc";

export const EMPTY_RICH_DOCUMENT: RichDocument = {
  type: "doc",
  content: [{ type: "paragraph" }],
};

export function cloneDocument(document: RichDocument): RichDocument {
  return structuredClone(document);
}

export function canonicalizeDocument(document: RichDocument): RichDocument {
  const copy = cloneDocument(document);
  walkMutable(copy, (node) => {
    if (node.type !== "image" || !isRecord(node.attrs)) {
      return;
    }
    const assetId = node.attrs.assetId;
    if (typeof assetId === "string" && assetId !== "") {
      node.attrs.src = `study-asset://${assetId}`;
    }
    for (const key of ["width", "height"] as const) {
      const value = node.attrs[key];
      const numeric =
        typeof value === "number"
          ? value
          : typeof value === "string" && value.trim() !== ""
            ? Number(value)
            : Number.NaN;

      // Tiptap may emit dimensions as numbers, strings, or null. Persist only
      // the bounded numeric form accepted by the Rust schema validator. This
      // keeps editor output and durable card content on one explicit contract.
      node.attrs[key] =
        Number.isFinite(numeric) && numeric >= 1 && numeric <= 10_000
          ? numeric
          : null;
    }
  });
  return copy;
}

export function hydrateDocument(
  document: RichDocument,
  assets: readonly Asset[],
): RichDocument {
  const copy = cloneDocument(document);
  const byId = new Map(assets.map((asset) => [asset.id, asset]));

  walkMutable(copy, (node) => {
    if (node.type !== "image" || !isRecord(node.attrs)) {
      return;
    }
    const assetId = node.attrs.assetId;
    if (typeof assetId !== "string") {
      return;
    }
    const asset = byId.get(assetId);
    if (asset) {
      node.attrs.src = convertFileSrc(asset.filePath);
      if (typeof node.attrs.alt !== "string" || node.attrs.alt.trim() === "") {
        node.attrs.alt = asset.originalFilename;
      }
    }
  });

  return copy;
}

export function collectAssetIds(document: RichDocument): string[] {
  const ids = new Set<string>();
  walk(document, (node) => {
    if (node.type !== "image" || !isRecord(node.attrs)) {
      return;
    }
    const assetId = node.attrs.assetId;
    if (typeof assetId === "string" && assetId !== "") {
      ids.add(assetId);
    }
  });
  return [...ids];
}

export function isDocumentEmpty(
  text: string,
  document: RichDocument,
): boolean {
  return text.trim() === "" && collectAssetIds(document).length === 0;
}

function walk(value: unknown, visitor: (record: Record<string, unknown>) => void) {
  if (Array.isArray(value)) {
    for (const item of value) {
      walk(item, visitor);
    }
    return;
  }
  if (!isRecord(value)) {
    return;
  }
  visitor(value);
  for (const nested of Object.values(value)) {
    walk(nested, visitor);
  }
}

function walkMutable(
  value: unknown,
  visitor: (record: Record<string, unknown>) => void,
) {
  if (Array.isArray(value)) {
    for (const item of value) {
      walkMutable(item, visitor);
    }
    return;
  }
  if (!isRecord(value)) {
    return;
  }
  visitor(value);
  for (const nested of Object.values(value)) {
    walkMutable(nested, visitor);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
