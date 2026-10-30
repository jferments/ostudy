import { useCallback, useRef, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { getErrorMessage } from "../../ipc/common";
import { publishPreparedExport, releasePreparedExport } from "../../ipc/externalFiles";
import { prepareNotebookPdf, type BoardPrint } from "../../ipc/notebooks";

function count(value: number, noun: string): string {
  return `${value} ${noun}${value === 1 ? "" : "s"}`;
}

/**
 * Saving a notebook as a PDF. What is pending is saved first, so the file
 * holds everything on screen. Boards go in as `boards` says.
 */
export function useNotebookPdfExport(
  notebookId: string,
  flush: () => Promise<boolean>,
  showToast: ShowToast,
): { exporting: boolean; exportPdf: (boards?: BoardPrint) => Promise<void> } {
  const [exporting, setExporting] = useState(false);
  const running = useRef(false);

  const exportPdf = useCallback(async (boards?: BoardPrint) => {
    if (running.current) return;
    running.current = true;
    setExporting(true);
    let preparedExportId: string | null = null;
    try {
      if (!await flush()) {
        showToast("error", "Some notes are not saved yet, so the PDF would leave them out. Try again once they are saved.");
        return;
      }
      const prepared = boards ? await prepareNotebookPdf(notebookId, undefined, boards) : await prepareNotebookPdf(notebookId);
      preparedExportId = prepared.preparedExport.id;
      const receipt = await publishPreparedExport(preparedExportId);
      if (receipt) {
        const missing = prepared.missingPictureCount > 0
          ? ` ${count(prepared.missingPictureCount, "picture")} could not be read and ${prepared.missingPictureCount === 1 ? "was" : "were"} left out.`
          : "";
        showToast("success", `Saved ${count(prepared.pageCount, "page")} as a PDF.${missing}`);
      }
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      if (preparedExportId) await releasePreparedExport(preparedExportId).catch(() => undefined);
      running.current = false;
      setExporting(false);
    }
  }, [flush, notebookId, showToast]);

  return { exporting, exportPdf };
}
