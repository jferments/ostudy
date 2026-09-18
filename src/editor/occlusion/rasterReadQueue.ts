/** Serialize raster reads without retaining decoded images. Superseded queued
 * reads are skipped before entering the native single-image worker. Durable
 * imports/reveals must never use this cancellation policy.
 */
export class RasterReadQueue {
  private tail: Promise<void> = Promise.resolve();

  read<T>(isCurrent: () => boolean, operation: () => Promise<T>): Promise<T | undefined> {
    const result = this.tail.then(() => isCurrent() ? operation() : undefined);
    this.tail = result.then(() => undefined, () => undefined);
    return result;
  }
}

export const occlusionRasterReads = new RasterReadQueue();
