import type { ShowToast } from "../../components/Toast";
import type { StudyGuidePdfTheme } from "../../ipc/studyGuides";
import { InlineWorkspace } from "../../ui/overlays/Dialog";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { InkEditor } from "../ink/InkEditor";
import { useDesignedSectionPage } from "./useDesignedSectionPage";

/** One guide section's designed page open for writing, laid out again for the editor to hold. */
export function GuidePageInk({ documentId, topicId, theme, contentKey, title, onClose, showToast }: {
  documentId: string;
  topicId: string;
  theme: StudyGuidePdfTheme;
  contentKey: string;
  title: string;
  onClose: () => void;
  showToast: ShowToast;
}) {
  const page = useDesignedSectionPage(documentId, topicId, theme, contentKey);
  if (page === null || page === "loading") {
    return (
      <InlineWorkspace presentation="page" title={title} description="Study guide page" closeLabel="Back to the guide" onClose={onClose}>
        {page === "loading" ? <LoadingState label="Laying out the page…" /> : (
          <div className="tw:p-page tw:compact:p-4">
            <InlineNotice kind="error">This page could not be laid out to write on.</InlineNotice>
          </div>
        )}
      </InlineWorkspace>
    );
  }
  return (
    <InkEditor
      material={{ kind: "studyGuide", documentId }}
      pages={[{ key: topicId, width: page.width, height: page.height, image: page.url }]}
      title={title}
      description="Study guide page"
      closeLabel="Back to the guide"
      onClose={onClose}
      showToast={showToast}
    />
  );
}
