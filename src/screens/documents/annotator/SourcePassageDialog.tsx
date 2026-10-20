import type { ShowToast } from "../../../components/Toast";
import { Dialog } from "../../../ui/overlays/Dialog";
import { Button } from "../../../ui/primitives/Button";
import { InlineNotice } from "../../../ui/primitives/InlineNotice";
import { AnnotatorBody, useAnnotatorLeave } from "./DocumentAnnotator";
import type { SourcePassage } from "../../../components/sourcePassage";
import { useDocumentAnnotator } from "./useDocumentAnnotator";

interface SourcePassageDialogProps {
  passage: SourcePassage;
  onClose: () => void;
  showToast: ShowToast;
}

/**
 * A card's source, open over whatever the learner was doing: the document's
 * pages at the cited page, which is outlined for a moment. It is the full
 * annotator, so a learner can mark the passage while reading it again;
 * closing saves those marks and returns to the card.
 */
export function SourcePassageDialog({ passage, onClose, showToast }: SourcePassageDialogProps) {
  const controller = useDocumentAnnotator(passage.documentId, showToast);
  const { leave, closing } = useAnnotatorLeave(controller);
  const failed = controller.saveState.state === "failed";
  return (
    <Dialog
      title={passage.title}
      description={`Page ${passage.pageNumber}`}
      presentation="fullscreen"
      width="workspace"
      padding="none"
      busy={closing}
      onClose={() => void leave(onClose)}
    >
      {failed ? (
        <InlineNotice kind="error" className="tw:mx-5 tw:compact:mx-4">
          <span className="tw:flex tw:flex-wrap tw:items-center tw:gap-2">
            Annotations not saved.
            <Button size="small" onClick={controller.retrySave}>Retry</Button>
          </span>
        </InlineNotice>
      ) : null}
      <AnnotatorBody
        documentId={passage.documentId}
        title={passage.title}
        controller={controller}
        showToast={showToast}
        focusPage={passage.pageNumber}
        focusAnnotationId={passage.annotationId}
      />
    </Dialog>
  );
}
