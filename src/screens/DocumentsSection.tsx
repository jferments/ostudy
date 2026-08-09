import { useEffect, useState } from "react";

import type { ShowToast } from "../components/Toast";
import {
  getAiTaskDefaults,
  listAiProviderProfiles,
  type AiProviderProfileSummary,
} from "../ipc/aiProfiles";
import { DocumentsScreen } from "./DocumentsScreen";
import styles from "./DocumentsSection.module.css";

interface DocumentsSectionProps {
  refreshDecks: () => Promise<unknown>;
  showToast: ShowToast;
}

export function DocumentsSection({ refreshDecks, showToast }: DocumentsSectionProps) {
  const [profile, setProfile] = useState<AiProviderProfileSummary | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let active = true;
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
  }, []);

  return (
    <div className={styles.section}>
      <aside className={styles.providerPanel} aria-label="Current Flashcard Generation model">
        <p className={styles.label}>Current Flashcard Generation model</p>
        <strong>
          {isLoading ? "Loading…" : profile?.modelId || profile?.displayName || "Not configured"}
        </strong>
      </aside>

      <DocumentsScreen refreshDecks={refreshDecks} showToast={showToast} />
    </div>
  );
}
