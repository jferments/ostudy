import { useState } from "react";

import { useAiFeatures } from "../../controllers/AiFeaturesProvider";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { Dialog } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";

interface TourStop {
  icon: IconName;
  title: string;
  body: string;
}

const STOPS: readonly TourStop[] = [
  {
    icon: "library",
    title: "Library keeps each subject together",
    body: "A subject holds one course's flashcards and documents. Library shows every subject as a tile with what it holds and how many cards are due.",
  },
  {
    icon: "cards",
    title: "Write cards your way",
    body: "In a subject's Flashcards page, write cards with formatting, formulas and images, hide parts of a picture with image occlusion, and sort cards into decks and tags. Decks from Anki come in as they are.",
  },
  {
    icon: "calendar-check",
    title: "Review a little every day",
    body: "Home shows what is due today. oStudy schedules each card with FSRS, so cards you know come back rarely and cards you miss come back soon.",
  },
  {
    icon: "highlighter",
    title: "Read and mark up documents",
    body: "Add PDFs and other documents to a subject, highlight and write on them, and turn a marked passage into a card that links back to its page.",
  },
  {
    icon: "sticky-note",
    title: "Keep notebooks",
    body: "Notes holds your own notebooks, written and drawn with the same pens, with links to cards, documents and other notes.",
  },
  {
    icon: "analytics",
    title: "See your progress",
    body: "Progress shows how much you studied, how well you remembered, how strong your cards are and what is coming up.",
  },
];

const AI_STOP: TourStop = {
  icon: "sparkles",
  title: "Add AI when you want more",
  body: "With an AI service, oStudy adds a Tutor that answers from your material, and makes study guides, lessons, podcasts and flashcards from your documents.",
};

/**
 * A short tour of oStudy's main places, one stop at a time. The last stop is
 * AI: what it adds, and Set up AI while it is off.
 */
export function ProgramTour({ onClose, onSetUpAi }: { onClose: () => void; onSetUpAi: () => void }) {
  const ai = useAiFeatures().enabled;
  const stops = [...STOPS, AI_STOP];
  const [index, setIndex] = useState(0);
  const stop = stops[index] ?? AI_STOP;
  const last = index === stops.length - 1;
  return (
    <Dialog
      title="Tour of oStudy"
      description={`${index + 1} of ${stops.length}`}
      width="medium"
      onClose={onClose}
      actions={
        <>
          {index > 0 ? (
            <Button variant="secondary" className="tw:me-auto" onClick={() => setIndex(index - 1)}>Back</Button>
          ) : (
            <Button variant="secondary" className="tw:me-auto" onClick={onClose}>Close</Button>
          )}
          {last && !ai ? (
            <Button
              onClick={() => {
                onClose();
                onSetUpAi();
              }}
            >
              <Icon name="sparkles" size={16} />Set up AI
            </Button>
          ) : null}
          <Button variant="primary" onClick={() => (last ? onClose() : setIndex(index + 1))}>
            {last ? "Done" : "Next"}
          </Button>
        </>
      }
    >
      <div data-testid="program-tour-stop" className="tw:grid tw:justify-items-center tw:gap-4 tw:py-4 tw:text-center">
        <span aria-hidden="true" className="tw:grid tw:size-16 tw:place-items-center tw:rounded-full tw:bg-accent tw:text-accent-foreground">
          <Icon name={stop.icon} size={28} />
        </span>
        <h3 className="tw:m-0 tw:text-section tw:font-semibold">{stop.title}</h3>
        <p className="tw:m-0 tw:max-w-[32rem] tw:text-muted-foreground">{stop.body}</p>
      </div>
    </Dialog>
  );
}
