import { useEffect, useState } from "react";

import { AI_FEATURES_BUILT, useAiFeatures } from "../../controllers/AiFeaturesProvider";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
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

/** The Settings sections, as the drawing of Settings shows them. */
const SETTINGS_SECTIONS: ReadonlyArray<{ label: string; icon: IconName; ai?: boolean }> = [
  { label: "Appearance", icon: "view" },
  { label: "Review", icon: "schedule" },
  { label: "Card templates", icon: "template" },
  { label: "AI", icon: "sparkles", ai: true },
  { label: "Backup and sync", icon: "archive" },
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
 * The first thing a new install shows: what oStudy does on its own and what
 * an AI service adds, with Set up AI opening the setup wizard. Not now shows
 * where AI setup waits in Settings. It shows once per device, and not at
 * all when a service is already set up or the build has no AI.
 */
export function AiIntroduction({ ready, onSetUpAi }: {
  /** False while a practice session is running, so the introduction waits for it. */
  ready: boolean;
  onSetUpAi: () => void;
}) {
  const ai = useAiFeatures();
  const [step, setStep] = useState<"closed" | "welcome" | "later">("closed");

  useEffect(() => {
    if (!AI_FEATURES_BUILT || !ready || !ai.known || introductionShown()) return;
    rememberShown();
    if (!ai.hasServices) setStep("welcome");
  }, [ai.hasServices, ai.known, ready]);

  if (step === "closed") return null;
  const close = () => setStep("closed");
  const setUp = (
    <Button
      variant="primary"
      data-testid="ai-introduction-set-up"
      onClick={() => {
        close();
        onSetUpAi();
      }}
    >
      <Icon name="sparkles" size={16} />Set up AI
    </Button>
  );

  if (step === "later") {
    return (
      <Dialog
        title="AI is in Settings"
        width="medium"
        onClose={close}
        actions={
          <>
            <Button variant="secondary" className="tw:me-auto" onClick={close}>Got it</Button>
            {setUp}
          </>
        }
      >
        <div className="tw:grid tw:min-w-0 tw:gap-5">
          <p className="tw:m-0">
            Whenever you want the Tutor, study guides, lessons and podcasts, open <b>Settings</b>,
            choose <b>AI</b>, then <b>Set up AI</b>.
          </p>
          <SettingsMiniature />
        </div>
      </Dialog>
    );
  }

  return (
    <Dialog
      title="Welcome to oStudy"
      width="medium"
      onClose={() => setStep("later")}
      actions={
        <>
          <Button variant="secondary" className="tw:me-auto" onClick={() => setStep("later")}>Not now</Button>
          {setUp}
        </>
      }
    >
      <div className="tw:grid tw:min-w-0 tw:gap-5">
        <p className="tw:m-0">
          oStudy is a complete flashcard and note-taking app on its own: write and review cards on
          a schedule that adapts to your memory, import decks, read and mark up your documents, and
          keep notebooks that link it all together. Many of oStudy&apos;s features also use an AI
          service.
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
          Setting one up takes a few minutes and costs most learners a few dollars a month.
        </p>
      </div>
    </Dialog>
  );
}

/**
 * A small drawing of Settings with the way to AI highlighted: the Settings
 * button at the foot of the side rail, then the AI section. Drawn rather
 * than pictured, so it follows the theme and the section names.
 */
export function SettingsMiniature({ className }: { className?: string }) {
  return (
    <figure
      aria-label="Settings, with the AI section highlighted"
      data-testid="ai-settings-miniature"
      className={cn(
        "tw:m-0 tw:flex tw:min-w-0 tw:overflow-hidden tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-background",
        className,
      )}
    >
      <div aria-hidden="true" className="tw:flex tw:w-14 tw:shrink-0 tw:flex-col tw:items-center tw:gap-3 tw:border-0 tw:border-e tw:border-solid tw:border-border tw:bg-muted tw:py-3">
        {(["home", "library", "sticky-note", "analytics"] as const).map((icon) => (
          <Icon key={icon} name={icon} size={18} className="tw:text-muted-foreground" />
        ))}
        <span className="tw:mt-auto tw:grid tw:size-9 tw:place-items-center tw:rounded-md tw:bg-primary tw:text-primary-foreground tw:ring-3 tw:ring-primary/40">
          <Icon name="settings" size={18} />
        </span>
      </div>
      <div className="tw:grid tw:min-w-0 tw:flex-1 tw:content-start tw:gap-1 tw:p-3">
        <span aria-hidden="true" className="tw:mb-1 tw:text-label tw:font-semibold">Settings</span>
        <ul className="tw:m-0 tw:grid tw:list-none tw:gap-1 tw:p-0">
          {SETTINGS_SECTIONS.map((section) => (
            <li
              key={section.label}
              aria-current={section.ai ? "true" : undefined}
              className={cn(
                "tw:flex tw:items-center tw:gap-2 tw:rounded-md tw:px-2.5 tw:py-1.5 tw:text-label",
                section.ai
                  ? "tw:bg-accent tw:font-semibold tw:text-accent-foreground tw:ring-2 tw:ring-primary"
                  : "tw:text-muted-foreground",
              )}
            >
              <Icon name={section.icon} size={16} className="tw:shrink-0" />
              {section.label}
            </li>
          ))}
        </ul>
      </div>
    </figure>
  );
}
