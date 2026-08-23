import { useEffect, useState } from "react";

import type { ShowToast } from "../components/Toast";
import {
  getAiTaskDefaults,
  listAiProviderProfiles,
  type AiProviderProfileSummary,
} from "../ipc/aiProfiles";
import { usePlatformCapabilities } from "../platform/PlatformProvider";
import { HelpTrigger } from "../ui/help/HelpProvider";
import { Icon } from "../ui/icons/Icon";
import { Surface } from "../ui/primitives/Surface";
import { DocumentsDevelopmentStub } from "./DocumentsDevelopmentStub";
import { DocumentsScreen } from "./DocumentsScreen";
import styles from "./DocumentsSection.module.css";

interface DocumentsSectionProps {
  refreshDecks: () => Promise<unknown>;
  onPracticeCards: (cardIds: string[]) => Promise<void>;
  showToast: ShowToast;
}

export function DocumentsSection({ refreshDecks, showToast }: DocumentsSectionProps) {
  const capabilities = usePlatformCapabilities();
  const documentProcessingAvailable =
    capabilities.native === null ||
    capabilities.native.services.nativePdfRender.state === "available";
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
          ? profiles.find((candidate) => candidate.id === defaults.flashcardGenerationProfileId)
          : undefined;
        setProfile(selected ?? null);
      })
      .catch(() => {
        if (active) setProfile(null);
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
    };
  }, [documentProcessingAvailable]);

  if (!documentProcessingAvailable) {
    return <DocumentsDevelopmentStub />;
  }

  const modelLabel = isLoading
    ? "Loading…"
    : profile?.modelId || profile?.displayName || "Not configured";

  const providerPanel = (
    <Surface
      as="aside"
      level={2}
      material="translucent"
      className={styles.providerPanel}
      aria-label="Current Flashcard Generation model"
    >
      <span className={styles.icon} aria-hidden="true">
        <Icon name="sparkles" size={21} />
      </span>
      <div className={styles.providerCopy}>
        <p className={styles.label}>Current Flashcard Generation model</p>
        <strong>{modelLabel}</strong>
        <small>
          {isLoading
            ? "Reading the current task default"
            : profile
              ? "Used for document flashcards and study guides"
              : "Configure a provider before generating document learning artifacts"}
        </small>
      </div>
      <div className={styles.providerActions}>
        <HelpTrigger topicId="documents.provider" label="Help with document AI profiles" />
      </div>
    </Surface>
  );

  return (
    <div className={styles.section} data-tour-anchor="documents.upload">
      <DocumentsScreen
        refreshDecks={refreshDecks}
        showToast={showToast}
        headerSupplement={providerPanel}
      />
    </div>
  );
}
