import { reportFrontendFailure } from "../ipc/diagnostics";
import { dataTransferImageFiles, readClipboardImageFiles } from "./imageInput";

interface PasteGesture {
  claimed: boolean;
  complete: boolean;
  delivered: boolean;
  canceled: boolean;
  files: File[];
}

/** Acquisition is scoped to the uploader, not the lifetime of accepted jobs.
 * Read inside the actual user gesture: deferring clipboard.read() to a timer
 * loses WebKit's paste authorization. Native events and async reads claim the
 * same gesture once; separate rapid gestures retain their original order. */
export class ImagePasteSession {
  private gesture: PasteGesture | null = null;
  private unbind: (() => void) | null = null;
  private requestPaste: (() => void) | null = null;

  paste(): void { this.requestPaste?.(); }

  bind(accept: (files: File[]) => void, target: Window = window,
    onError?: (message: string | null) => void, scope?: HTMLElement): () => void {
    this.unbind?.();
    let disposed = false;
    const queue: PasteGesture[] = [];
    const receives = (event: Event) => {
      const element = event.target instanceof Element ? event.target : null;
      if (scope && event.target instanceof Node && !scope.contains(event.target)) return false;
      // Text fields (including generation instructions) retain ordinary paste.
      return !element?.closest("input, textarea, [contenteditable], [role='textbox']")
        || !!element.closest("[data-image-paste-target]");
    };
    const begin = () => {
      const gesture: PasteGesture = { claimed: false, complete: false, delivered: false, canceled: false, files: [] };
      this.gesture = gesture;
      queue.push(gesture);
      return gesture;
    };
    const drain = () => {
      while (queue[0]?.complete) {
        const gesture = queue.shift();
        if (!gesture) break;
        gesture.delivered = true;
        const files = gesture.files;
        gesture.files = []; // Release browser bytes once the upload owner accepts them.
        if (files.length > 0 && !gesture.canceled) {
          onError?.(null);
          accept(files);
        }
      }
    };
    const receive = (gesture: PasteGesture, files: File[]) => {
      if (disposed || gesture.canceled || gesture.claimed) return;
      gesture.complete = true;
      if (files.length > 0) {
        gesture.claimed = true;
        if (gesture.delivered) {
          // An empty/denied async read does not disqualify a later native paste.
          onError?.(null);
          accept(files);
          return;
        }
        gesture.files = files;
      }
      drain();
    };
    const read = (gesture: PasteGesture, explicit: boolean) => {
      // Do not add a timeout, permission query, or awaited operation before this call.
      void readClipboardImageFiles().then((files) => {
        if (disposed || gesture.canceled || gesture.claimed) return;
        receive(gesture, files);
        if (explicit && files.length === 0 && this.gesture === gesture) {
          onError?.("No image was available to paste. Copy a screenshot, click the paste area, and press Ctrl+V (Cmd+V on Mac).");
        }
      }).catch((error: unknown) => {
        if (disposed || gesture.canceled || gesture.claimed) return;
        receive(gesture, []);
        if (this.gesture === gesture) {
          reportFrontendFailure("clipboard_image_read_failed", error);
          onError?.("Clipboard access was unavailable. Click the paste area and press Ctrl+V (Cmd+V on Mac), or use Select images.");
        }
      });
    };
    const receivePaste = (event: ClipboardEvent) => {
      if (disposed || !receives(event) || !event.clipboardData) return;
      const files = dataTransferImageFiles(event.clipboardData);
      if (files.length === 0) return;
      event.preventDefault();
      event.stopPropagation();
      const keyboardGesture = this.gesture;
      receive(keyboardGesture ?? begin(), files);
      if (!keyboardGesture) this.gesture = null;
    };
    const receiveShortcut = (event: KeyboardEvent) => {
      if (disposed || !receives(event) || event.repeat || event.altKey
        || (!event.ctrlKey && !event.metaKey) || event.key.toLowerCase() !== "v") return;
      // Keep the native paste action enabled; it may be the only available API.
      read(begin(), false);
    };
    const nextGesture = () => { this.gesture = null; };
    const loseFocus = () => {
      for (const gesture of queue) { gesture.canceled = true; gesture.files = []; }
      queue.length = 0;
      nextGesture();
    };
    this.requestPaste = () => { if (!disposed) read(begin(), true); };
    target.addEventListener("paste", receivePaste, true);
    target.addEventListener("keydown", receiveShortcut, true);
    target.addEventListener("pointerdown", nextGesture, true);
    target.addEventListener("contextmenu", nextGesture, true);
    target.addEventListener("blur", loseFocus);
    const dispose = () => {
      if (disposed) return;
      disposed = true;
      for (const gesture of queue) { gesture.canceled = true; gesture.files = []; }
      queue.length = 0;
      if (!this.gesture?.claimed) this.gesture = null;
      target.removeEventListener("paste", receivePaste, true);
      target.removeEventListener("keydown", receiveShortcut, true);
      target.removeEventListener("pointerdown", nextGesture, true);
      target.removeEventListener("contextmenu", nextGesture, true);
      target.removeEventListener("blur", loseFocus);
      if (this.unbind === dispose) { this.unbind = null; this.requestPaste = null; }
    };
    this.unbind = dispose;
    return dispose;
  }
}
