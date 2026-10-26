import { useState, type ReactNode } from "react";

import { AI_FEATURES_BUILT, useAiFeatures } from "../../controllers/AiFeaturesProvider";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { Checkbox } from "../../ui/primitives/FormControls";

/** Set once the learner asks Home not to suggest AI again. */
export const HOME_AI_TIP_STORAGE_KEY = "ostudy.homeAiTip.v1";

function aiTipHidden(): boolean {
  try {
    return window.localStorage.getItem(HOME_AI_TIP_STORAGE_KEY) !== null;
  } catch {
    return false;
  }
}

function rememberAiTipHidden(): void {
  try {
    window.localStorage.setItem(HOME_AI_TIP_STORAGE_KEY, "hidden");
  } catch {
    // Optional tip storage must never block study; the tip just hides for now.
  }
}

interface HomeGettingStartedProps {
  onSetUpAi: () => void;
  /** Present on the first-run welcome, which also suggests a deck and the tour. */
  onImportDeck?: () => void;
  onTakeTour?: () => void;
}

/**
 * Ways to begin beside Home's welcome: bringing in Anki or Quizlet decks, a tour of
 * oStudy, and, while no AI service is set up, what AI adds with Set up AI and
 * Don't show this again. On a Home that already has subjects only the AI tip
 * shows, until it is set up or hidden.
 */
export function HomeGettingStarted({ onSetUpAi, onImportDeck, onTakeTour }: HomeGettingStartedProps) {
  const ai = useAiFeatures();
  const [aiTipShown, setAiTipShown] = useState(() => !aiTipHidden());
  const [dontShowAgain, setDontShowAgain] = useState(false);
  const showAiTip = AI_FEATURES_BUILT && ai.known && !ai.enabled && aiTipShown;
  if (!showAiTip && !onImportDeck && !onTakeTour) return null;

  return (
    <ul aria-label="Getting started" data-testid="home-getting-started" className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0 tw:medium:grid-cols-2 tw:wide:grid-cols-3">
      {onImportDeck ? (
        <StartTile icon="import" title="Bring your flashcards" action={<Button onClick={onImportDeck}><Icon name="import" size={16} />Import a deck</Button>}>
          Import decks from Anki or Quizlet, or from another oStudy library, and review them here.
        </StartTile>
      ) : null}
      {onTakeTour ? (
        <StartTile icon="lightbulb" title="Take a tour" action={<Button onClick={onTakeTour}><Icon name="play" size={16} />Start the tour</Button>}>
          A short look at subjects, cards, review, documents, notes and progress.
        </StartTile>
      ) : null}
      {showAiTip ? (
        <StartTile
          icon="sparkles"
          title="Want more? Add AI"
          action={
            <div className="tw:grid tw:justify-items-start tw:gap-1">
              <Button onClick={onSetUpAi}><Icon name="sparkles" size={16} />Set up AI</Button>
              <label className="tw:flex tw:min-h-11 tw:cursor-pointer tw:items-center tw:gap-2 tw:text-small tw:text-muted-foreground">
                <Checkbox
                  checked={dontShowAgain}
                  onCheckedChange={(checked) => {
                    setDontShowAgain(checked);
                    if (checked) {
                      rememberAiTipHidden();
                      setAiTipShown(false);
                    }
                  }}
                />
                Don&apos;t show this again
              </label>
            </div>
          }
        >
          An AI service adds a Tutor, and makes study guides, lessons, podcasts and flashcards from
          your documents. Set it up any time in Settings → AI.
        </StartTile>
      ) : null}
    </ul>
  );
}

function StartTile({ icon, title, action, children }: { icon: IconName; title: string; action: ReactNode; children: ReactNode }) {
  return (
    <li className="tw:grid tw:min-w-0 tw:content-start tw:gap-3 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:p-4 tw:text-start">
      <span className="tw:flex tw:items-center tw:gap-2.5">
        <span aria-hidden="true" className="tw:grid tw:size-9 tw:shrink-0 tw:place-items-center tw:rounded-lg tw:bg-accent tw:text-accent-foreground">
          <Icon name={icon} size={18} />
        </span>
        <h3 className="tw:m-0 tw:text-body tw:font-semibold">{title}</h3>
      </span>
      <p className="tw:m-0 tw:text-small tw:text-muted-foreground">{children}</p>
      <div>{action}</div>
    </li>
  );
}
