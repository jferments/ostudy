import { reportFrontendFailure } from "../ipc/diagnostics";
import { dataTransferImageFiles, readClipboardImageFiles } from "./imageInput";

interface PasteGesture {
  claimed: boolean;
}

/** Owns clipboard acquisition, not accepted uploads or document jobs. A completed
 * keyboard gesture survives a source-list rerender so its late native paste event
 * cannot admit the same images a second time. A new input gesture starts afresh. */
export class ImagePasteSession {
  private gesture: PasteGesture | null = null;
  private unbind: (() => void) | null = null;

  bind(accept: (files: File[]) => void, target: Window = window): () => void {
    this.unbind?.();
    let disposed = false;
    let fallbackTimer: ReturnType<typeof setTimeout> | null = null;

    const clearFallback = () => {
      if (fallbackTimer !== null) {
        clearTimeout(fallbackTimer);
        fallbackTimer = null;
      }
    };
    const nextGesture = () => {
      clearFallback();
      this.gesture = null;
    };
    const receivePaste = (event: ClipboardEvent) => {
      if (disposed || !event.clipboardData) return;
      const files = dataTransferImageFiles(event.clipboardData);
      if (files.length === 0) return;
      event.preventDefault();
      event.stopPropagation();
      if (this.gesture?.claimed) return;
      if (this.gesture) this.gesture.claimed = true;
      clearFallback();
      accept(files);
    };
    const receiveShortcut = (event: KeyboardEvent) => {
      if (disposed) return;
      nextGesture();
      if (event.altKey || (!event.ctrlKey && !event.metaKey) || event.key.toLowerCase() !== "v") return;
      const gesture: PasteGesture = { claimed: false };
      this.gesture = gesture;
      // Yield to the ordinary paste event; this is not a delay-based deduplicator.
      // Identity/claim checks protect both orderings even after the read starts.
      fallbackTimer = setTimeout(() => {
        fallbackTimer = null;
        if (disposed || this.gesture !== gesture || gesture.claimed) return;
        void readClipboardImageFiles().then((files) => {
          if (disposed || this.gesture !== gesture || gesture.claimed || files.length === 0) return;
          gesture.claimed = true;
          accept(files);
        }).catch((error: unknown) => {
          if (!disposed && this.gesture === gesture && !gesture.claimed) {
            reportFrontendFailure("clipboard_image_read_failed", error);
          }
        });
      }, 0);
    };

    target.addEventListener("paste", receivePaste, true);
    target.addEventListener("keydown", receiveShortcut, true);
    // Context-menu paste and the next deliberate shortcut are distinct gestures.
    target.addEventListener("pointerdown", nextGesture, true);
    target.addEventListener("contextmenu", nextGesture, true);
    target.addEventListener("blur", nextGesture);
    const dispose = () => {
      if (disposed) return;
      disposed = true;
      clearFallback();
      // Keep only a completed claim across same-workspace scope rebinding.
      // Every outstanding read is also fenced by its disposed subscription.
      if (!this.gesture?.claimed) this.gesture = null;
      target.removeEventListener("paste", receivePaste, true);
      target.removeEventListener("keydown", receiveShortcut, true);
      target.removeEventListener("pointerdown", nextGesture, true);
      target.removeEventListener("contextmenu", nextGesture, true);
      target.removeEventListener("blur", nextGesture);
      if (this.unbind === dispose) this.unbind = null;
    };
    this.unbind = dispose;
    return dispose;
  }
}
