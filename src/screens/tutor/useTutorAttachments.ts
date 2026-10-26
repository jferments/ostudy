import { useCallback, useEffect, useRef, useState } from "react";

import { useRequestLane } from "../../controllers/requestLane";
import { beginAssetDraft, cancelAssetDraft, importImage } from "../../ipc/assets";
import { createOperationId, getErrorMessage } from "../../ipc/common";
import { getTutorImageInput, type TutorAttachment, type TutorAttachmentInput } from "../../ipc/tutor";
import { IMAGE_FILE_ACCEPT, stageBrowserImage } from "../../platform/imageInput";

/** Most pictures and files one message carries, and how long a file can be (native `tutor/contracts.rs`). */
const MAX_ATTACHED_IMAGES = 4;
const MAX_ATTACHMENTS = 8;
const MAX_ATTACHED_TEXT_CHARS = 30_000;
const MAX_ATTACHED_TEXT_TOTAL = 60_000;

/** What the composer's file picker offers: pictures and plain text files. */
export const ATTACHMENT_ACCEPT = `${IMAGE_FILE_ACCEPT},.txt,.md,.markdown,.csv,text/plain,text/markdown,text/csv`;

/** A picture or file waiting in the composer to go with the next question. */
export interface ComposerAttachment {
  key: string;
  kind: "image" | "text";
  name: string;
  /** A picture is added (staged in its own import draft) before it can be sent. */
  ready: boolean;
  assetId?: string;
  /** The import draft a new picture is staged in; none once a send kept it. */
  draftId?: string;
  text?: string;
}

const IMAGE_NAME = /\.(png|jpe?g|webp)$/iu;
const TEXT_NAME = /\.(txt|md|markdown|csv)$/iu;
// A text file is read whole before its length is checked.
const MAX_TEXT_FILE_BYTES = 1024 * 1024;

function isImage(file: File): boolean {
  return ["image/png", "image/jpeg", "image/webp"].includes(file.type) || IMAGE_NAME.test(file.name);
}

function isText(file: File): boolean {
  return ["text/plain", "text/markdown", "text/csv"].includes(file.type) || TEXT_NAME.test(file.name);
}

/** The files in a drop or paste that can be attached, and whether any could not. */
export function attachableFiles(files: readonly File[]): File[] {
  return files.filter((file) => isImage(file) || isText(file));
}

/** A saved message's attachments, to send again with its question. */
export function resendAttachments(attachments: readonly TutorAttachment[]): TutorAttachmentInput[] {
  return attachments.map((attachment) => attachment.kind === "image"
    ? { kind: "image", assetId: attachment.assetId }
    : { kind: "text", name: attachment.name, text: attachment.text });
}

/**
 * The composer's attachments. A picture is staged in an import draft of its
 * own as soon as it is added, so sending only names it; removing it, or
 * leaving Tutor before sending, cancels that draft.
 */
export function useTutorAttachments(profileId: string | null) {
  const [items, setItemsState] = useState<ComposerAttachment[]>([]);
  const itemsRef = useRef<ComposerAttachment[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [imagesAccepted, setImagesAccepted] = useState<boolean | null>(null);
  const [checks, setChecks] = useState(0);
  const lane = useRequestLane();
  const alive = useRef(true);

  const setItems = useCallback((update: (current: ComposerAttachment[]) => ComposerAttachment[]) => {
    itemsRef.current = update(itemsRef.current);
    setItemsState(itemsRef.current);
  }, []);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      for (const item of itemsRef.current) {
        if (item.draftId) void cancelAssetDraft(item.draftId).catch(() => undefined);
      }
    };
  }, []);

  // Read again whenever a picture is added: the model may have changed in Settings.
  useEffect(() => {
    const token = lane.begin();
    getTutorImageInput(profileId).then(
      (accepted) => { if (lane.isCurrent(token)) setImagesAccepted(accepted); },
      () => { if (lane.isCurrent(token)) setImagesAccepted(null); },
    );
  }, [lane, profileId, checks]);

  async function addImage(key: string, file: File): Promise<void> {
    let draftId: string | null = null;
    try {
      draftId = (await beginAssetDraft("import")).id;
      const staged = await stageBrowserImage(file, createOperationId());
      const asset = await importImage(staged.id, draftId);
      if (!alive.current || !itemsRef.current.some((item) => item.key === key)) {
        void cancelAssetDraft(draftId).catch(() => undefined);
        return;
      }
      const kept = draftId;
      setItems((current) => current.map((item) => item.key === key
        ? { ...item, ready: true, assetId: asset.id, draftId: kept }
        : item));
    } catch (failure: unknown) {
      if (draftId) void cancelAssetDraft(draftId).catch(() => undefined);
      if (!alive.current) return;
      setItems((current) => current.filter((item) => item.key !== key));
      setError(`${file.name || "The picture"} couldn't be attached: ${getErrorMessage(failure)}`);
    }
  }

  async function addText(key: string, file: File): Promise<void> {
    const refuse = (message: string) => {
      setItems((current) => current.filter((item) => item.key !== key));
      setError(message);
    };
    try {
      const text = file.size > MAX_TEXT_FILE_BYTES ? null : await file.text();
      if (!alive.current) return;
      if (text === null || text.length > MAX_ATTACHED_TEXT_CHARS) {
        refuse(`${file.name} is too long to attach. A file can hold up to ${MAX_ATTACHED_TEXT_CHARS.toLocaleString()} characters.`);
        return;
      }
      if (!text.trim()) {
        refuse(`${file.name} is empty.`);
        return;
      }
      const others = itemsRef.current.reduce((total, item) => total + (item.text?.length ?? 0), 0);
      if (others + text.length > MAX_ATTACHED_TEXT_TOTAL) {
        refuse(`${file.name} doesn't fit with the other files. Files sent together can hold up to ${MAX_ATTACHED_TEXT_TOTAL.toLocaleString()} characters.`);
        return;
      }
      setItems((current) => current.map((item) => item.key === key ? { ...item, ready: true, text } : item));
    } catch (failure: unknown) {
      if (alive.current) refuse(`${file.name} couldn't be read: ${getErrorMessage(failure)}`);
    }
  }

  /** Adds pictures and text files, in order, within a message's limits. */
  function add(files: readonly File[]): void {
    setError(null);
    let images = itemsRef.current.filter((item) => item.kind === "image").length;
    let count = itemsRef.current.length;
    for (const file of files) {
      const kind = isImage(file) ? "image" : isText(file) ? "text" : null;
      if (!kind) {
        setError(`${file.name} can't be attached. Attach a picture (PNG, JPEG or WebP) or a text file.`);
        continue;
      }
      if (count >= MAX_ATTACHMENTS || (kind === "image" && images >= MAX_ATTACHED_IMAGES)) {
        setError(kind === "image" && images >= MAX_ATTACHED_IMAGES
          ? `A message can carry up to ${MAX_ATTACHED_IMAGES} pictures.`
          : `A message can carry up to ${MAX_ATTACHMENTS} attachments.`);
        continue;
      }
      count += 1;
      if (kind === "image") images += 1;
      const key = createOperationId();
      const name = file.name || "Pasted picture.png";
      setItems((current) => [...current, { key, kind, name, ready: false }]);
      if (kind === "image") {
        setChecks((value) => value + 1);
        void addImage(key, file);
      } else {
        void addText(key, file);
      }
    }
  }

  function remove(key: string): void {
    const item = itemsRef.current.find((entry) => entry.key === key);
    if (item?.draftId) void cancelAssetDraft(item.draftId).catch(() => undefined);
    setItems((current) => current.filter((entry) => entry.key !== key));
  }

  /** What to send with the next question. */
  function inputs(): TutorAttachmentInput[] {
    return itemsRef.current.filter((item) => item.ready).map((item) => item.kind === "image"
      ? { kind: "image", assetId: item.assetId ?? "", ...(item.draftId ? { draftId: item.draftId } : {}) }
      : { kind: "text", name: item.name, text: item.text ?? "" });
  }

  /**
   * After a send was admitted: an answered question takes its attachments
   * with it, and an unanswered one leaves them to send again, its pictures
   * now kept rather than staged.
   */
  function settle(answered: boolean): void {
    setError(null);
    setItems((current) => answered ? [] : current.map((item) => ({ ...item, draftId: undefined })));
  }

  const hasImages = items.some((item) => item.kind === "image");
  return {
    items,
    error,
    dismissError: () => setError(null),
    /** False when the chat's model can't see pictures; null until known. */
    imagesAccepted,
    /** Pictures are attached that the chat's model can't see. */
    imagesBlocked: hasImages && imagesAccepted === false,
    adding: items.some((item) => !item.ready),
    add,
    remove,
    inputs,
    settle,
  };
}

export type TutorAttachments = ReturnType<typeof useTutorAttachments>;
