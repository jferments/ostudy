import type { ShowToast } from "../components/Toast";
import { usePlatformCapabilities } from "../platform/PlatformProvider";
import { DocumentsDevelopmentStub } from "./DocumentsDevelopmentStub";
import { DocumentsScreen } from "./DocumentsScreen";
import { providerStyles as styles } from "./documents/presentation";

interface DocumentsSectionProps {
  refreshDecks: () => Promise<unknown>;
  onPracticeCards: (cardIds: string[]) => Promise<void>;
  onStudySubject?: (subjectId: string) => void;
  onOpenSubject?: (subjectId: string | null) => void;
  showToast: ShowToast;
  /** Opens AI setup from a Make action while no model is set. */
  onSetUpAi?: () => void;
  /** Shows one subject's documents, or No subject's (`null`); every document, grouped by subject, when left out. */
  subjectId?: string | null;
  /** Opens a notebook at one of its pages, such as one a page was clipped into. */
  onOpenNotebook?: (notebookId: string, pageId: string) => void;
}

export function DocumentsSection({
  refreshDecks, onPracticeCards, onStudySubject, onOpenSubject, showToast, onSetUpAi, subjectId, onOpenNotebook,
}: DocumentsSectionProps) {
  const capabilities = usePlatformCapabilities();
  const documentProcessingAvailable = capabilities.native === null || capabilities.native.services.nativePdfRender.state === "available";
  if (!documentProcessingAvailable) return <DocumentsDevelopmentStub />;
  return <div className={styles.section} data-tour-anchor="documents.upload">
    <DocumentsScreen
      refreshDecks={refreshDecks}
      onPracticeCards={onPracticeCards}
      onStudySubject={onStudySubject}
      onOpenSubject={onOpenSubject}
      showToast={showToast}
      onSetUpAi={onSetUpAi}
      subjectId={subjectId}
      onOpenNotebook={onOpenNotebook}
    />
  </div>;
}
