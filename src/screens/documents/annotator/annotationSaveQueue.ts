import { getErrorMessage } from "../../../ipc/common";
import {
  MAX_ANNOTATION_CHANGES_PER_REQUEST,
  type DocumentAnnotationChange,
} from "../../../ipc/documentAnnotations";
import type { AnnotationEdit } from "./annotationHistory";

export type SaveState =
  | { state: "idle" }
  | { state: "pending" }
  | { state: "saving" }
  | { state: "saved" }
  | { state: "failed"; message: string };

interface SaveQueueOptions {
  documentId: string;
  send: (documentId: string, changes: DocumentAnnotationChange[]) => Promise<unknown>;
  onState: (state: SaveState) => void;
  /** How long edits gather before they are written. */
  delayMs?: number;
}

/**
 * Writes the annotator's edits in the background. Each annotation's latest
 * state replaces any unsaved earlier one, so a burst of edits becomes one small
 * write, and requests go one at a time. Writes are whole states, so a request
 * repeated after a failure cannot apply anything twice.
 */
export class AnnotationSaveQueue {
  private readonly pending = new Map<string, { change: DocumentAnnotationChange; version: number }>();
  private version = 0;
  private running: Promise<boolean> | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(private readonly options: SaveQueueOptions) {}

  record(edits: readonly AnnotationEdit[]): void {
    for (const edit of edits) {
      this.version += 1;
      const change: DocumentAnnotationChange = edit.after
        ? { op: "put", id: edit.id, pageNumber: edit.after.pageNumber, shape: edit.after.shape }
        : { op: "delete", id: edit.id };
      this.pending.set(edit.id, { change, version: this.version });
    }
    if (this.pending.size === 0) return;
    if (!this.running) this.options.onState({ state: "pending" });
    this.schedule();
  }

  hasUnsaved(): boolean {
    return this.pending.size > 0;
  }

  /** Writes everything recorded so far; resolves false if a write failed. */
  flush(): Promise<boolean> {
    this.clearTimer();
    this.running ??= this.drain().finally(() => { this.running = null; });
    return this.running;
  }

  /** Stops the timer; recorded edits stay until the next flush. */
  dispose(): void {
    this.clearTimer();
  }

  private schedule(): void {
    this.clearTimer();
    this.timer = setTimeout(() => { void this.flush(); }, this.options.delayMs ?? 400);
  }

  private clearTimer(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
  }

  private async drain(): Promise<boolean> {
    while (this.pending.size > 0) {
      const batch = [...this.pending.entries()].slice(0, MAX_ANNOTATION_CHANGES_PER_REQUEST);
      this.options.onState({ state: "saving" });
      try {
        await this.options.send(this.options.documentId, batch.map(([, entry]) => entry.change));
      } catch (error) {
        this.options.onState({ state: "failed", message: getErrorMessage(error) });
        return false;
      }
      // An annotation edited again while this request was out stays pending.
      for (const [id, entry] of batch) {
        if (this.pending.get(id)?.version === entry.version) this.pending.delete(id);
      }
    }
    this.options.onState({ state: "saved" });
    return true;
  }
}
