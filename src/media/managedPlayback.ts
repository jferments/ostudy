import { readMediaAsset } from "../ipc/assets";

const ASSET_ID_PATTERN = /^[0-9a-f]{32}$/u;

export type ManagedMediaKind = "audio" | "video";

export async function createManagedMediaObjectUrl(
  assetId: string,
  kind: ManagedMediaKind,
): Promise<string> {
  if (!ASSET_ID_PATTERN.test(assetId)) {
    throw new Error("The managed media asset identifier is invalid.");
  }

  const bytes = await readMediaAsset(assetId);
  if (!(bytes instanceof ArrayBuffer) || bytes.byteLength === 0) {
    throw new Error("The managed media asset returned no bytes.");
  }

  // Native admission has already inspected and re-verified these exact bytes.
  // The WebView repeats only the small amount of container identification needed
  // to give the Blob an explicit MIME type. WebKitGTK will otherwise reject some
  // blob media with MEDIA_ERR_SRC_NOT_SUPPORTED.
  const mimeType = playbackMimeType(bytes, kind);
  return URL.createObjectURL(new Blob([bytes], { type: mimeType }));
}

export function revokeManagedMediaObjectUrl(url: string | null): void {
  if (url !== null) URL.revokeObjectURL(url);
}

export function playbackMimeType(bytes: ArrayBuffer, kind: ManagedMediaKind): string {
  const data = new Uint8Array(bytes);
  if (kind === "video") {
    if (containsIsoBmffFileType(data)) return "video/mp4";
    throw new Error("The managed video container is not supported for playback.");
  }

  if (startsWithAscii(data, 0, "RIFF") && startsWithAscii(data, 8, "WAVE")) {
    return "audio/wav";
  }
  if (startsWithAscii(data, 0, "OggS")) return "audio/ogg";
  if (startsWithAscii(data, 0, "ID3")) return "audio/mpeg";
  if (looksLikeAdtsAac(data)) return "audio/aac";
  if (looksLikeMp3Frame(data)) return "audio/mpeg";
  if (containsIsoBmffFileType(data)) return "audio/mp4";

  throw new Error("The managed audio container is not supported for playback.");
}

function startsWithAscii(data: Uint8Array, offset: number, text: string): boolean {
  if (offset < 0 || offset + text.length > data.length) return false;
  for (let index = 0; index < text.length; index += 1) {
    if (data[offset + index] !== text.charCodeAt(index)) return false;
  }
  return true;
}

function looksLikeAdtsAac(data: Uint8Array): boolean {
  return data.length >= 7
    && data[0] === 0xff
    && (data[1]! & 0xf6) === 0xf0
    && ((data[2]! >> 2) & 0x0f) !== 0x0f;
}

function looksLikeMp3Frame(data: Uint8Array): boolean {
  if (data.length < 4 || data[0] !== 0xff || (data[1]! & 0xe0) !== 0xe0) return false;
  const versionBits = (data[1]! >> 3) & 0x03;
  const layerBits = (data[1]! >> 1) & 0x03;
  const bitrateIndex = data[2]! >> 4;
  const sampleRateIndex = (data[2]! >> 2) & 0x03;
  return versionBits !== 0x01
    && layerBits === 0x01
    && bitrateIndex !== 0
    && bitrateIndex !== 0x0f
    && sampleRateIndex !== 0x03;
}

function containsIsoBmffFileType(data: Uint8Array): boolean {
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  let offset = 0;
  for (let boxes = 0; boxes < 64 && offset + 8 <= data.length; boxes += 1) {
    const size32 = view.getUint32(offset, false);
    const kindIsFtyp = startsWithAscii(data, offset + 4, "ftyp");
    let headerSize = 8;
    let boxSize: number;

    if (size32 === 1) {
      if (offset + 16 > data.length) return false;
      const extended = view.getBigUint64(offset + 8, false);
      if (extended > BigInt(Number.MAX_SAFE_INTEGER)) return false;
      boxSize = Number(extended);
      headerSize = 16;
    } else if (size32 === 0) {
      boxSize = data.length - offset;
    } else {
      boxSize = size32;
    }

    if (boxSize < headerSize || offset + boxSize > data.length) return false;
    if (kindIsFtyp) return true;
    offset += boxSize;
  }
  return false;
}
