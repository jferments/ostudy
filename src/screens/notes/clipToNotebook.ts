import { beginAssetDraft, cancelAssetDraft } from "../../ipc/assets";
import type { ImageShape } from "../../ipc/documentAnnotations";
import {
  addNotebookPage,
  applyNotebookItemChanges,
  getNotebook,
  stageDocumentClip,
  type DocumentClipRegion,
  type NotebookContents,
  type NotebookPage,
} from "../../ipc/notebooks";
import { createAnnotationId, shapeBounds } from "../documents/annotator/annotationModel";
import { fitPicture } from "./pictures";

/** Space kept above a clip, between clips and at the foot of a page, in points. */
const CLIP_GAP = 24;
const PAGE_MARGIN = 48;
/** Line height of typed text, as a share of its size, for finding where a page's marks end. */
const LINE_HEIGHT = 1.4;

/** Where a clip landed: its notebook page, and the clip itself. */
export interface PlacedClip { notebookId: string; pageId: string; itemId: string; }

/**
 * Clips a region of a PDF page into a notebook: the region is drawn from the
 * PDF, then placed below what is already on the notebook's last page, on a
 * new page when it does not fit there. The picture names the page it came
 * from. A failure leaves nothing behind but an image draft that expires.
 */
export async function clipToNotebook(notebookId: string, region: DocumentClipRegion): Promise<PlacedClip> {
  const draftId = (await beginAssetDraft("import", notebookId)).id;
  try {
    const clip = await stageDocumentClip(draftId, region);
    let contents = await getNotebook(notebookId);
    let page = contents.pages[contents.pages.length - 1];
    if (!page) throw new Error("This notebook has no pages.");
    const size = fitPicture(clip, page);
    let top = nextFreeTop(contents, page);
    if (top + size.height > page.height - PAGE_MARGIN && top > PAGE_MARGIN) {
      contents = await addNotebookPage(notebookId, contents.pages.length + 1);
      page = contents.pages[contents.pages.length - 1]!;
      top = PAGE_MARGIN;
    }
    const shape: ImageShape = {
      kind: "image",
      x: Math.round((page.width - size.width) / 2),
      y: Math.round(Math.max(0, Math.min(top, page.height - size.height))),
      width: size.width,
      height: size.height,
      assetId: clip.asset.id,
      source: { documentId: region.documentId, pageNumber: region.pageNumber },
    };
    const itemId = createAnnotationId();
    await applyNotebookItemChanges(notebookId, [{ op: "put", id: itemId, pageId: page.id, shape, assetDraftId: draftId }]);
    return { notebookId, pageId: page.id, itemId };
  } catch (error) {
    void cancelAssetDraft(draftId).catch(() => undefined);
    throw error;
  }
}

/** The top of the free space below a page's marks. */
function nextFreeTop(contents: NotebookContents, page: NotebookPage): number {
  let bottom = PAGE_MARGIN - CLIP_GAP;
  for (const item of contents.items) {
    if (item.pageId !== page.id) continue;
    const shape = item.shape;
    const textHeight = shape.kind === "text" ? shape.fontSize * LINE_HEIGHT * Math.max(1, shape.text.split("\n").length) : 0;
    bottom = Math.max(bottom, shapeBounds(shape, textHeight).bottom);
  }
  return Math.ceil(bottom + CLIP_GAP);
}
