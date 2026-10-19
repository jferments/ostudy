import { useEffect, useState } from "react";
import type { ShowToast } from "../components/Toast";
import { getAiTaskDefaults, listAiProviderProfiles, type AiProviderProfileSummary } from "../ipc/aiProfiles";
import { usePlatformCapabilities } from "../platform/PlatformProvider";
import { HelpTrigger } from "../ui/help/HelpProvider";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { DocumentsDevelopmentStub } from "./DocumentsDevelopmentStub";
import { DocumentsScreen } from "./DocumentsScreen";
import { providerStyles as styles } from "./documents/presentation";

interface DocumentsSectionProps {
  refreshDecks: () => Promise<unknown>;
  onPracticeCards: (cardIds: string[]) => Promise<void>;
  onStudyCollection?: (collectionId: string) => void;
  showToast: ShowToast;
}

export function DocumentsSection({ refreshDecks, onStudyCollection, showToast }: DocumentsSectionProps) {
  const capabilities = usePlatformCapabilities();
  const documentProcessingAvailable = capabilities.native === null || capabilities.native.services.nativePdfRender.state === "available";
  const [profile, setProfile] = useState<AiProviderProfileSummary | null>(null);
  const [isLoading, setIsLoading] = useState(documentProcessingAvailable);

  useEffect(() => {
    if (!documentProcessingAvailable) {
      setProfile(null);
      setIsLoading(false);
      return;
    }
    let active = true;
    setIsLoading(true);
    void Promise.all([getAiTaskDefaults(), listAiProviderProfiles()])
      .then(([defaults, profiles]) => {
        if (!active) return;
        const selected = defaults.flashcardGenerationProfileId
          ? profiles.find((candidate) => candidate.id === defaults.flashcardGenerationProfileId) : undefined;
        setProfile(selected ?? null);
      })
      .catch(() => { if (active) setProfile(null); })
      .finally(() => { if (active) setIsLoading(false); });
    return () => { active = false; };
  }, [documentProcessingAvailable]);

  if (!documentProcessingAvailable) return <DocumentsDevelopmentStub />;
  // The model itself lives in Settings; the page only says when none is set up.
  const providerNotice = !isLoading && !profile ? (
    <InlineNotice kind="warning" className="tw:mb-5">
      Set up an AI provider in Settings to make cards and guides from documents.
      <HelpTrigger topicId="documents.provider" label="Help with document AI profiles" />
    </InlineNotice>
  ) : null;
  return <div className={styles.section} data-tour-anchor="documents.upload">
    <DocumentsScreen
      refreshDecks={refreshDecks}
      onStudyCollection={onStudyCollection}
      showToast={showToast}
      headerSupplement={providerNotice}
    />
  </div>;
}
