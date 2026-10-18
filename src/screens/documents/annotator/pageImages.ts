import { useEffect, useState } from "react";

import { readDocumentSourceAsset } from "../../../ipc/studyGuides";

export interface PageImage {
  url: string | null;
  failed: boolean;
}

type Listener = (image: PageImage) => void;

interface Entry {
  url: string | null;
  failed: boolean;
  /** Pages showing the image now. */
  users: number;
  listeners: Set<Listener>;
  state: "queued" | "reading" | "done";
}

const NO_IMAGE: PageImage = { url: null, failed: false };

/**
 * One document's rendered pages as object URLs. Pages near the screen hold
 * their images; a few recently shown ones stay ready for scrolling back, and
 * the rest are released. Only a few reads run at once, so a fast scroll does
 * not queue a whole document; pages scrolled past before their turn are
 * never read.
 */
export class PageImageCache {
  private readonly entries = new Map<string, Entry>();
  private readonly queue: string[] = [];
  private reading = 0;
  private disposed = false;

  constructor(
    private readonly documentId: string,
    private readonly read: (documentId: string, assetId: string) => Promise<ArrayBuffer> = readDocumentSourceAsset,
    private readonly limit = 24,
    private readonly concurrency = 3,
  ) {}

  /** Holds a page image; the listener hears when it is ready. Returns the release. */
  acquire(assetId: string, listener: Listener): () => void {
    let entry = this.entries.get(assetId);
    if (entry) {
      // Most recently used last, so eviction starts with the oldest.
      this.entries.delete(assetId);
      this.entries.set(assetId, entry);
    } else {
      entry = { url: null, failed: false, users: 0, listeners: new Set(), state: "queued" };
      this.entries.set(assetId, entry);
      this.queue.push(assetId);
    }
    const held = entry;
    held.users += 1;
    held.listeners.add(listener);
    if (held.state === "done") listener({ url: held.url, failed: held.failed });
    this.pump();
    let released = false;
    return () => {
      if (released) return;
      released = true;
      held.users -= 1;
      held.listeners.delete(listener);
      this.evict();
    };
  }

  dispose(): void {
    this.disposed = true;
    for (const entry of this.entries.values()) if (entry.url) URL.revokeObjectURL(entry.url);
    this.entries.clear();
    this.queue.length = 0;
  }

  private pump(): void {
    while (!this.disposed && this.reading < this.concurrency) {
      const assetId = this.queue.shift();
      if (assetId === undefined) return;
      const entry = this.entries.get(assetId);
      if (!entry || entry.state !== "queued") continue;
      if (entry.users === 0) {
        this.entries.delete(assetId);
        continue;
      }
      entry.state = "reading";
      this.reading += 1;
      void this.read(this.documentId, assetId)
        .then((bytes) => {
          if (!this.disposed && this.entries.get(assetId) === entry) {
            entry.url = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
          }
        }, () => {
          entry.failed = true;
        })
        .finally(() => {
          this.reading -= 1;
          entry.state = "done";
          if (this.disposed) return;
          for (const listener of entry.listeners) listener({ url: entry.url, failed: entry.failed });
          this.evict();
          this.pump();
        });
    }
  }

  private evict(): void {
    let excess = this.entries.size - this.limit;
    for (const [assetId, entry] of this.entries) {
      if (excess <= 0) return;
      if (entry.users > 0 || entry.state === "reading") continue;
      if (entry.url) URL.revokeObjectURL(entry.url);
      this.entries.delete(assetId);
      excess -= 1;
    }
  }
}

/** A page's image while `active`; nothing while the page is far off screen. */
export function usePageImage(cache: PageImageCache | null, assetId: string, active: boolean): PageImage {
  const [image, setImage] = useState<{ key: string; image: PageImage } | null>(null);
  useEffect(() => {
    if (!cache || !active) return undefined;
    const release = cache.acquire(assetId, (next) => setImage({ key: assetId, image: next }));
    return () => {
      release();
      // A released image may be revoked; never show its old URL again.
      setImage(null);
    };
  }, [cache, assetId, active]);
  return active && image?.key === assetId ? image.image : NO_IMAGE;
}
