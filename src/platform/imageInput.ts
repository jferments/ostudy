import { stageImageCollectionFile } from "../ipc/imageCollectionFiles";
import type { StagedExternalFile } from "../ipc/externalFiles";

export const IMAGE_FILE_ACCEPT = "image/png,image/jpeg,image/webp,.png,.jpg,.jpeg,.webp";
export const MAX_IMAGE_FILE_BYTES = 25 * 1024 * 1024;

/** File input, a local file drop, and an explicit paste share one admission path. */
export async function stageBrowserImage(file: File, operationId: string): Promise<StagedExternalFile> {
  if (file.size === 0 || file.size > MAX_IMAGE_FILE_BYTES) {
    throw new Error("Choose a nonempty PNG, JPEG, or WebP image no larger than 25 MB.");
  }
  const dataBase64 = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error(`Could not read ${file.name || "the pasted image"}.`));
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
    reader.readAsDataURL(file);
  });
  return stageImageCollectionFile({
    operationId,
    displayName: file.name || "Clipboard image.png",
    dataBase64,
  });
}

/** Do not import pasted HTML, text, or a remote image URL behind the user's back. */
export function clipboardImageFiles(data: DataTransfer): File[] {
  const files = Array.from(data.files).filter(isImageFile);
  if (files.length > 0) return files;
  return Array.from(data.items)
    .filter((item) => item.kind === "file" && item.type.startsWith("image/"))
    .flatMap((item) => {
      const file = item.getAsFile();
      return file ? [file] : [];
    });
}

function isImageFile(file: File): boolean {
  return file.type.startsWith("image/") || /\.(png|jpe?g|webp)$/iu.test(file.name);
}
