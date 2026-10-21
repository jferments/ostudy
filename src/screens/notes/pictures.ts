/** The size an image file draws at, or null when it cannot be read. */
export function naturalSize(url: string): Promise<{ width: number; height: number } | null> {
  return new Promise((resolve) => {
    const image = new Image();
    image.onload = () => resolve(image.naturalWidth > 0 && image.naturalHeight > 0
      ? { width: image.naturalWidth, height: image.naturalHeight }
      : null);
    image.onerror = () => resolve(null);
    image.src = url;
  });
}

/**
 * A picture's size on a page: its own size in points, made smaller to fit
 * within the page's margins and a little over half its height, never larger.
 * An image whose size is unknown starts as a square.
 */
export function fitPicture(
  size: { width: number; height: number } | null,
  page: { width: number; height: number },
): { width: number; height: number } {
  const maxWidth = Math.max(24, page.width - 96);
  const maxHeight = Math.max(24, page.height * 0.6);
  const own = size ?? { width: 240, height: 240 };
  const scale = Math.min(1, maxWidth / own.width, maxHeight / own.height);
  return { width: Math.max(1, Math.round(own.width * scale)), height: Math.max(1, Math.round(own.height * scale)) };
}
