import { stageImageCollectionFileChunk } from "../ipc/imageCollectionFiles";
import type { StagedExternalFile } from "../ipc/externalFiles";

export const IMAGE_FILE_ACCEPT = "image/png,image/jpeg,image/webp,.png,.jpg,.jpeg,.webp";
export const MAX_IMAGE_FILE_BYTES = 25 * 1024 * 1024;
export const IMAGE_UPLOAD_CHUNK_BYTES = 512 * 1024;

/** File input, a local file drop, and an explicit paste share one bounded admission path.
 * The browser never materializes the complete image as one base64 IPC payload. */
export async function stageBrowserImage(file: File, operationId: string): Promise<StagedExternalFile> {
  if (file.size === 0 || file.size > MAX_IMAGE_FILE_BYTES) {
    throw new Error("Choose a nonempty PNG, JPEG, or WebP image no larger than 25 MB.");
  }
  const displayName = file.name || "Clipboard image.png";
  let staged: StagedExternalFile | null = null;
  for (let offset = 0; offset < file.size; offset += IMAGE_UPLOAD_CHUNK_BYTES) {
    const end = Math.min(offset + IMAGE_UPLOAD_CHUNK_BYTES, file.size);
    const dataBase64 = await readBlobBase64(file.slice(offset, end), displayName);
    staged = await stageImageCollectionFileChunk({
      operationId,
      displayName,
      totalBytes: file.size,
      offset,
      dataBase64,
      finalChunk: end === file.size,
    });
  }
  if (!staged) throw new Error("The image upload ended before native staging completed.");
  return staged;
}

async function readBlobBase64(blob: Blob, displayName: string): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error(`Could not read ${displayName}.`));
    reader.onabort = () => reject(new Error("Image reading was interrupted."));
    reader.onload = () => {
      const value = reader.result;
      const separator = typeof value === "string" ? value.indexOf(",") : -1;
      if (typeof value !== "string" || separator < 0) {
        reject(new Error("The image could not be read."));
      } else {
        resolve(value.slice(separator + 1));
      }
    };
    reader.readAsDataURL(blob);
  });
}

/** File-list and item-only clipboard/drop payloads share one local-only path.
 * Do not import pasted HTML, text, or a remote image URL behind the user's back. */
export function dataTransferImageFiles(data: Pick<DataTransfer, "files" | "items">): File[] {
  const files = Array.from(data.files).filter(isImageFile);
  if (files.length > 0) return files;
  return Array.from(data.items)
    .filter((item) => item.kind === "file" && isImageMime(item.type))
    .flatMap((item) => {
      const file = item.getAsFile();
      return file && isImageFile(file) ? [file] : [];
    });
}

/** Preserve the original exported name for callers/tests written around paste semantics. */
export const clipboardImageFiles = dataTransferImageFiles;

interface ClipboardImageItem {
  readonly types: readonly string[];
  getType(type: string): Promise<Blob>;
}
interface ClipboardImageReader {
  read(): Promise<readonly ClipboardImageItem[]>;
}

/** Some WebKit/Linux clipboard paths do not expose screenshot bytes on ClipboardEvent.
 * A Ctrl/Cmd+V user gesture may therefore use the browser clipboard reader as a fallback. */
export async function readClipboardImageFiles(
  reader: ClipboardImageReader | null = clipboardReader(),
): Promise<File[]> {
  if (!reader) return [];
  const items = await reader.read();
  const files: File[] = [];
  for (const item of items) {
    const type = item.types.find(isImageMime);
    if (!type) continue;
    const blob = await item.getType(type);
    if (blob.size === 0) continue;
    const extension = imageExtension(type);
    files.push(new File([blob], files.length === 0
      ? `Clipboard image.${extension}`
      : `Clipboard image ${files.length + 1}.${extension}`, { type }));
  }
  return files;
}

function clipboardReader(): ClipboardImageReader | null {
  const clipboard = globalThis.navigator?.clipboard;
  return clipboard && typeof clipboard.read === "function"
    ? clipboard as ClipboardImageReader
    : null;
}

function isImageMime(type: string): boolean {
  return type === "image/png" || type === "image/jpeg" || type === "image/webp";
}

function imageExtension(type: string): string {
  return type === "image/jpeg" ? "jpg" : type.slice("image/".length);
}

function isImageFile(file: File): boolean {
  return isImageMime(file.type) || /\.(png|jpe?g|webp)$/iu.test(file.name);
}
