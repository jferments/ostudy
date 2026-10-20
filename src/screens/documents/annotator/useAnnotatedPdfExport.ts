import { useCallback, useRef, useState } from "react";

import type { ShowToast } from "../../../components/Toast";
import { getErrorMessage } from "../../../ipc/common";
import { prepareAnnotatedPdf } from "../../../ipc/documentAnnotations";
import { publishPreparedExport, releasePreparedExport } from "../../../ipc/externalFiles";

function count(value: number, noun: string): string {
  return `${value} ${noun}${value === 1 ? "" : "s"}`;
}

/**
 * Saving a copy of the PDF with its annotations drawn in. Pending annotations
 * are saved first, so the copy holds everything on screen; the stored
 * document never changes.
 */
export function useAnnotatedPdfExport(
  documentId: string,
  flush: () => Promise<boolean>,
  showToast: ShowToast,
): { exporting: boolean; exportPdf: () => Promise<void> } {
  const [exporting, setExporting] = useState(false);
  const running = useRef(false);

  const exportPdf = useCallback(async () => {
    if (running.current) return;
    running.current = true;
    setExporting(true);
    let preparedExportId: string | null = null;
    try {
      if (!await flush()) {
        showToast("error", "Some annotations are not saved yet, so the copy would leave them out. Try again once they are saved.");
        return;
      }
      const prepared = await prepareAnnotatedPdf(documentId);
      preparedExportId = prepared.preparedExport.id;
      const receipt = await publishPreparedExport(preparedExportId);
      if (receipt) {
        showToast("success", `Saved a copy with ${count(prepared.annotationCount, "annotation")} on ${count(prepared.pageCount, "page")}.`);
      }
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      if (preparedExportId) await releasePreparedExport(preparedExportId).catch(() => undefined);
      running.current = false;
      setExporting(false);
    }
  }, [documentId, flush, showToast]);

  return { exporting, exportPdf };
}
