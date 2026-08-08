import { useEffect, useState } from "react";

import type { ShowToast } from "../components/Toast";
import {
  getAiTaskDefaults,
  listAiProviderProfiles,
  type AiProviderProfileSummary,
} from "../ipc/aiProfiles";
import { getErrorMessage } from "../ipc/common";
import { DocumentsScreen } from "./DocumentsScreen";
import styles from "./DocumentsSection.module.css";

interface DocumentsSectionProps {
  refreshDecks: () => Promise<unknown>;
  showToast: ShowToast;
}

export function DocumentsSection({ refreshDecks, showToast }: DocumentsSectionProps) {
  const [profile, setProfile] = useState<AiProviderProfileSummary | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void Promise.all([getAiTaskDefaults(), listAiProviderProfiles()])
      .then(([defaults, profiles]) => {
        if (!active) return;
        const selected = defaults.flashcardGenerationProfileId
          ? profiles.find((candidate) => candidate.id === defaults.flashcardGenerationProfileId)
          : undefined;
        setProfile(selected ?? null);
        setLoadError(null);
      })
      .catch((error: unknown) => {
        if (!active) return;
        setProfile(null);
        setLoadError(getErrorMessage(error));
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
      <aside className={styles.providerPanel} aria-label="Flashcard generation provider">
        <div>
          <p className={styles.label}>Flashcard generation provider</p>
          {isLoading ? (
            <strong>Loading provider…</strong>
          ) : profile ? (
            <>
              <strong>{profile.displayName}</strong>
              <p className={styles.details}>
                {profile.providerLabel} · {profile.modelId || "No saved model"} · {scopeLabel(profile)}
              </p>
            </>
          ) : (
            <>
              <strong>Not configured</strong>
              <p className={styles.details}>
                Set the Flashcard generation default in Settings → AI/LLM before uploading a PDF.
              </p>
            </>
          )}
        </div>
        {profile ? <p className={styles.destination}>{destinationMessage(profile)}</p> : null}
        {loadError ? <p className={styles.error}>Provider status could not be loaded: {loadError}</p> : null}
      </aside>

      <DocumentsScreen refreshDecks={refreshDecks} showToast={showToast} />
    </div>
  );
}

function scopeLabel(profile: AiProviderProfileSummary): string {
  switch (profile.networkScope) {
    case "local":
      return "Local computer";
    case "lan":
      return "Local network";
    case "cloud":
      return "Cloud";
  }
}

function destinationMessage(profile: AiProviderProfileSummary): string {
  if (profile.configurationStatus !== "ready") {
    return "This profile is not ready. Complete its endpoint, credential, and model configuration in Settings.";
  }
  switch (profile.networkScope) {
    case "local":
      return "Selected page text and supported page images stay on this computer's configured model server.";
    case "lan":
      return "Selected page text and supported page images will be sent to the configured local-network server.";
    case "cloud":
      return "Selected PDF page text and supported page images will be transmitted to this cloud provider.";
  }
}
