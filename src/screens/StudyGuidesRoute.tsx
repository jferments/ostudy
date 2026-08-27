import type { ShowToast } from "../components/Toast";
import { PageHeader, PageScaffold } from "../ui/patterns/WorkspacePatterns";
import { StudyGuideLibrarySection } from "./StudyGuideLibrarySection";

interface StudyGuidesRouteProps {
  showToast: ShowToast;
  onPracticeCards: (cardIds: string[]) => Promise<void>;
}

export function StudyGuidesRoute({
  showToast,
  onPracticeCards,
}: StudyGuidesRouteProps) {
  return (
    <PageScaffold width="wide" aria-label="Study Guides">
      <PageHeader
        title="Study Guides"
        icon="study-guide"
        description="Read generated document outlines, add notes, or study the flashcards linked to a topic."
        compact
      />
      <StudyGuideLibrarySection
        showToast={showToast}
        onPracticeCards={onPracticeCards}
        heading="Generated guides"
      />
    </PageScaffold>
  );
}
