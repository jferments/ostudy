import { useEffect, useState } from "react";

import { getQuestionOfTheDay, type QuestionOfTheDay, type QuestionReason } from "../../ipc/home";
import { Button } from "../../ui/primitives/Button";
import { Panel } from "../../ui/primitives/Panel";
import { RecallCardSurface } from "../practice/RecallCardSurface";
import type { SubjectTile } from "./homeModel";

const REASONS: Record<QuestionReason, string | null> = {
  new: "Not tried yet",
  struggled: "One you've found hard",
  review: null,
};

interface QuestionOfTheDayPanelProps {
  refreshKey: number;
  subjects: readonly SubjectTile[];
  onPracticeSubject: (subjectId: string) => void;
}

/**
 * One flashcard a study day, chosen natively and leaning toward cards not
 * tried yet or found hard. Turning it over records nothing. Hidden while it
 * loads, when it cannot load and when no card can be studied.
 */
export function QuestionOfTheDayPanel({ refreshKey, subjects, onPracticeSubject }: QuestionOfTheDayPanelProps) {
  const question = useQuestionOfTheDay(refreshKey);
  const [revealedCard, setRevealedCard] = useState<string | null>(null);
  const [linkError, setLinkError] = useState<string | null>(null);
  if (!question) return null;

  const { card } = question;
  const revealed = revealedCard === card.cardId;
  const subject = subjects.find((tile) => tile.subjectId !== null && tile.subjectId === question.subjectId);
  const subjectId = subject?.subjectId ?? null;
  const description = [subject?.name ?? question.deckName, REASONS[question.reason]].filter(Boolean).join(" · ");

  return (
    <Panel
      title="Question of the day"
      description={description}
      data-testid="home-question"
      actions={
        revealed && subjectId !== null ? (
          <Button variant="text" onClick={() => onPracticeSubject(subjectId)}>
            Practice {subject?.name}
          </Button>
        ) : null
      }
    >
      <RecallCardSurface
        presentation={card}
        presentationKey={card.cardId}
        revealed={revealed}
        displayQuestionOnAnswer
        sideToggle
        onReveal={() => setRevealedCard(card.cardId)}
        onLinkError={setLinkError}
      />
      {linkError ? <p className="tw:m-0 tw:text-small tw:text-destructive">{linkError}</p> : null}
      {revealed ? null : (
        <div>
          <Button variant="secondary" onClick={() => setRevealedCard(card.cardId)}>
            Show answer
          </Button>
        </div>
      )}
    </Panel>
  );
}

/** Today's question, read again whenever Home's summary changes. */
function useQuestionOfTheDay(refreshKey: number): QuestionOfTheDay | null {
  const [question, setQuestion] = useState<QuestionOfTheDay | null>(null);
  useEffect(() => {
    let current = true;
    void getQuestionOfTheDay()
      .then((value) => {
        if (current) setQuestion(value);
      })
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [refreshKey]);
  return question;
}
