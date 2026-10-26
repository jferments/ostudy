import { useEffect, useState } from "react";

import { listAiProviderProfiles } from "../../ipc/aiProfiles";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { Dialog } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";

/** Set once the introduction has been shown on this device. */
export const AI_INTRODUCTION_STORAGE_KEY = "ostudy.aiIntroduction.v1";

const WITH_AI: ReadonlyArray<{ icon: IconName; text: string }> = [
  { icon: "chat", text: "A Tutor that answers questions from your own material" },
  { icon: "study-guide", text: "Study guides, lessons and podcasts made from your documents" },
  { icon: "cards", text: "Flashcards written from a document or a topic" },
  { icon: "image", text: "Figures and images in your documents read and explained" },
  { icon: "listen", text: "Lessons and Tutor answers read aloud" },
];

function introductionShown(): boolean {
  try {
    return window.localStorage.getItem(AI_INTRODUCTION_STORAGE_KEY) !== null;
  } catch {
    // Without storage the introduction would return at every start, so skip it.
    return true;
  }
}

function rememberShown(): void {
  try {
    window.localStorage.setItem(AI_INTRODUCTION_STORAGE_KEY, "shown");
  } catch {
    // Optional introduction storage must never block study.
  }
}

/**
 * The first thing a new install shows: what oStudy does without an AI
 * service and what one adds, with Set up AI opening the setup wizard. It
 * shows once per device, and not at all when a service is already set up.
 */
export function AiIntroduction({ ready, onSetUpAi }: {
  /** False while a practice session is running, so the introduction waits for it. */
  ready: boolean;
  onSetUpAi: () => void;
}) {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (!ready || introductionShown()) return;
    let live = true;
    listAiProviderProfiles().then(
      (profiles) => {
        if (!live) return;
        rememberShown();
        if (profiles.length === 0) setOpen(true);
      },
      // A failed read says nothing about the services, so ask again next start.
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [ready]);

  if (!open) return null;
  return (
    <Dialog
      title="Welcome to oStudy"
      width="medium"
      onClose={() => setOpen(false)}
      actions={
        <>
          <Button variant="secondary" className="tw:me-auto" onClick={() => setOpen(false)}>Not now</Button>
          <Button
            variant="primary"
            data-testid="ai-introduction-set-up"
            onClick={() => {
              setOpen(false);
              onSetUpAi();
            }}
          >
            <Icon name="sparkles" size={16} />Set up AI
          </Button>
        </>
      }
    >
      <div className="tw:grid tw:min-w-0 tw:gap-5">
        <p className="tw:m-0">
          Most of what oStudy can do needs an AI service. Without one, oStudy is a flashcard and
          note-taking app: you can write and review cards, import decks, read and mark up
          documents, and keep notebooks.
        </p>
        <div className="tw:grid tw:gap-2">
          <h3 className="tw:m-0 tw:text-section tw:font-semibold">With an AI service, oStudy adds</h3>
          <ul aria-label="What AI adds" className="tw:m-0 tw:grid tw:list-none tw:gap-0 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:p-0">
            {WITH_AI.map((item) => (
              <li key={item.text} className="tw:flex tw:items-center tw:gap-3 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:px-4 tw:py-3 tw:first:border-t-0">
                <Icon name={item.icon} size={18} className="tw:shrink-0 tw:text-primary" />
                <span className="tw:min-w-0 tw:flex-1">{item.text}</span>
              </li>
            ))}
          </ul>
        </div>
        <p className="tw:m-0 tw:text-label tw:text-muted-foreground">
          Setting one up takes a few minutes and costs most learners a few dollars a month. You can
          also do it later in Settings → AI.
        </p>
      </div>
    </Dialog>
  );
}
