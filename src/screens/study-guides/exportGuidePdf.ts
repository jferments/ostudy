import type { ShowToast } from "../../components/Toast";
import { getErrorMessage } from "../../ipc/common";
import { publishPreparedExport, releasePreparedExport } from "../../ipc/externalFiles";
import { getSettings } from "../../ipc/settings";
import { prepareDocumentStudyGuidePdf } from "../../ipc/studyGuides";
import { prepareCollectionStudyGuidePdf } from "../../ipc/studyMaterials";
import type { ConfirmOptions } from "../../ui/overlays/overlayContext";
import { guidePdfPreferences } from "./guidePdfPreferences";

export interface GuidePdfTarget {
  kind: "document" | "collection";
  id: string;
}

/**
 * Prepares a guide's PDF and hands it to the save dialog. The reader and the
 * Study guides card both export this way, so they ask and report the same.
 */
export async function exportGuidePdf(
  target: GuidePdfTarget,
  confirm: (options: ConfirmOptions) => Promise<boolean>,
  showToast: ShowToast,
): Promise<void> {
  let preparedExportId: string | null = null;
  try {
    // The look and the notes are Settings choices, read fresh for each export.
    const { theme, includeNotes } = guidePdfPreferences(await getSettings());
    const prepared = target.kind === "document"
      ? await prepareDocumentStudyGuidePdf(target.id, includeNotes, theme)
      : await prepareCollectionStudyGuidePdf(target.id, includeNotes, theme);
    preparedExportId = prepared.preparedExport.id;
    if (prepared.mathSourceFallback && !await confirm({
      title: "Export math as source notation?",
      description: "Some equations use notation outside the native PDF math subset and will remain readable LaTeX source. The study guide in oStudy keeps its full typeset math.",
      confirmLabel: "Export PDF",
      cancelLabel: "Cancel",
    })) return;
    const receipt = await publishPreparedExport(prepared.preparedExport.id);
    if (receipt) {
      const notes = prepared.annotationCount > 0 ? ` and ${prepared.annotationCount} note${prepared.annotationCount === 1 ? "" : "s"}` : "";
      const what = target.kind === "document" ? "study guide" : "the subject study guide";
      showToast("success", `Exported ${what} with ${prepared.sectionCount} section${prepared.sectionCount === 1 ? "" : "s"}${notes}.`);
    }
  } catch (error: unknown) {
    showToast("error", getErrorMessage(error));
  } finally {
    if (preparedExportId) await releasePreparedExport(preparedExportId).catch(() => undefined);
  }
}
