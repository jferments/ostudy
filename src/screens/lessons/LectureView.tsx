import { useEffect, useId, useRef, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import { publishPreparedExport, releasePreparedExport } from "../../ipc/externalFiles";
import { getLessonLecture, prepareLessonLecturePptx, type Lecture, type LectureSlide } from "../../ipc/lessons";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { ItemTrigger } from "../../ui/primitives/Item";
import { useNarrationPlayer } from "./useNarrationPlayer";
import { useVoiceAvailable } from "./useVoiceAvailable";

interface LectureViewProps {
  lessonId: string;
  revisionNumber: number;
  lessonTitle: string;
  /** The step the lesson was on, so the lecture opens at its slide. */
  stepKey: string | null;
  onClose: () => void;
}

/**
 * A lesson as slides, each step's narration as its speaker notes. Play
 * lecture speaks from the current step slide and moves on as each one ends;
 * choosing another slide stops it.
 */
export function LectureView({ lessonId, revisionNumber, lessonTitle, stepKey, onClose }: LectureViewProps) {
  const lane = useRequestLane();
  const [lecture, setLecture] = useState<Lecture | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const [index, setIndex] = useState(0);
  const [lecturing, setLecturing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState<{ kind: "success" | "error"; text: string } | null>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const titleId = useId();
  const voice = useNarrationPlayer(lessonId, revisionNumber);
  const voiceAvailable = useVoiceAvailable();
  // The step only chooses the slide the lecture opens at.
  const openingStep = useRef(stepKey);

  useEffect(() => {
    void latestResult(lane, () => getLessonLecture(lessonId, revisionNumber)).then(
      (value) => {
        if (value === undefined) return;
        setLecture(value);
        const opening = value.slides.findIndex((slide) => slide.stepKey !== null && slide.stepKey === openingStep.current);
        setIndex(opening < 0 ? 0 : opening);
        setReadError(null);
      },
      (error: unknown) => setReadError(getErrorMessage(error)),
    );
  }, [lane, lessonId, revisionNumber]);

  if (readError || !lecture) {
    return (
      <LectureFrame lessonTitle={lessonTitle} onClose={onClose}>
        {readError ? (
          <InlineNotice kind="error">This lecture could not open: {readError}</InlineNotice>
        ) : (
          <LoadingState label="Laying out the slides…" />
        )}
      </LectureFrame>
    );
  }

  const slides = lecture.slides;
  const slide = slides[index];
  if (!slide) return null;
  const spoken = (item: LectureSlide | undefined) => item?.kind === "step" && item.notes.trim() !== "";
  const canLecture = slides.some((item) => spoken(item));

  const speakFrom = (from: number) => {
    let next = from;
    while (slides[next]?.kind === "step" && !spoken(slides[next])) next += 1;
    const item = slides[next];
    if (!spoken(item) || !item?.stepKey) {
      setLecturing(false);
      return;
    }
    setIndex(next);
    voice.play(item.stepKey, () => speakFrom(next + 1));
  };

  const startLecture = () => {
    setLecturing(true);
    // From a title slide, the lecture starts at the first step.
    speakFrom(slide.kind === "step" ? index : slides.findIndex((item) => spoken(item)));
  };

  const stopLecture = () => {
    voice.stop();
    setLecturing(false);
  };

  // The deck is written natively, handed to the save dialog, then released;
  // closing the dialog without saving is not an error.
  const saveDeck = async () => {
    setSaving(true);
    setSaved(null);
    let preparedId: string | null = null;
    try {
      const prepared = await prepareLessonLecturePptx(lessonId, revisionNumber);
      preparedId = prepared.preparedExport.id;
      const receipt = await publishPreparedExport(preparedId);
      if (receipt) setSaved({ kind: "success", text: `Saved ${receipt.displayName}.` });
    } catch (error: unknown) {
      setSaved({ kind: "error", text: `The slides were not saved: ${getErrorMessage(error)}` });
    } finally {
      if (preparedId) await releasePreparedExport(preparedId).catch(() => undefined);
      setSaving(false);
    }
  };

  const show = (next: number) => {
    stopLecture();
    setIndex(next);
    headingRef.current?.focus();
  };

  return (
    <LectureFrame lessonTitle={lessonTitle} onClose={onClose}>
      <header className="tw:flex tw:flex-wrap tw:items-end tw:justify-between tw:gap-3">
        <div className="tw:flex tw:min-w-0 tw:flex-col tw:gap-1">
          <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
            {slides.length} slides · the notes are the narration
          </p>
          <h1 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight tw:break-words">
            {lecture.title} · lecture
          </h1>
        </div>
        <div className="tw:flex tw:flex-wrap tw:gap-2">
          <Button className="tw:rounded-full" disabled={saving} onClick={() => void saveDeck()}>
            <Icon name="download" size={16} /> {saving ? "Saving…" : "Save as .pptx"}
          </Button>
          {/* A failed voice leaves the lecture where it stopped. */}
          {lecturing && voice.listening !== "idle" ? (
            <Button className="tw:rounded-full" onClick={stopLecture}>
              <Icon name="stop" size={16} /> {voice.listening === "preparing" ? "Preparing…" : "Stop"}
            </Button>
          ) : voiceAvailable ? (
            <Button variant="primary" className="tw:rounded-full" disabled={!canLecture} onClick={startLecture}>
              <Icon name="listen" size={16} /> Play lecture
            </Button>
          ) : null}
        </div>
      </header>
      {saved ? <InlineNotice kind={saved.kind}>{saved.text}</InlineNotice> : null}
      {voice.error ? <InlineNotice kind="error">The lecture could not be read aloud: {voice.error}</InlineNotice> : null}
      <div className="tw:grid tw:items-start tw:gap-6 tw:wide:grid-cols-[15rem_minmax(0,1fr)] tw:medium:grid-cols-[13rem_minmax(0,1fr)]">
        <nav
          aria-label="Slides"
          className="tw:hidden tw:rounded-lg tw:bg-(--ostudy-color-canvas-inset) tw:p-3 tw:wide:block tw:medium:block"
        >
          <ol className="tw:m-0 tw:flex tw:list-none tw:flex-col tw:gap-0.5 tw:p-0">
            {slides.map((item, position) => (
              <li key={`${item.kind}-${item.stepKey ?? position}`}>
                <ItemTrigger
                  aria-current={position === index ? "step" : undefined}
                  onClick={() => show(position)}
                  className={cn(
                    "tw:w-full tw:items-center tw:gap-2.5 tw:rounded-md tw:px-2 tw:py-2 tw:text-start tw:text-small tw:outline-none tw:hover:bg-accent/40 tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50",
                    position === index && "tw:bg-card tw:font-semibold tw:shadow-sm",
                  )}
                >
                  <span aria-hidden="true" className="tw:w-5 tw:shrink-0 tw:text-caption tw:tabular-nums tw:text-muted-foreground">
                    {position + 1}
                  </span>
                  <span className="tw:min-w-0 tw:flex-1 tw:break-words">{item.title}</span>
                </ItemTrigger>
              </li>
            ))}
          </ol>
        </nav>
        <article aria-labelledby={titleId} className="tw:flex tw:min-w-0 tw:flex-col tw:gap-4">
          <section
            data-testid="lecture-slide"
            className="tw:flex tw:aspect-video tw:flex-col tw:gap-4 tw:overflow-auto tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-8 tw:shadow-sm tw:compact:p-5"
          >
            <h2
              id={titleId}
              ref={headingRef}
              tabIndex={-1}
              className={cn(
                "tw:m-0 tw:font-semibold tw:tracking-tight tw:break-words tw:outline-none",
                slide.kind === "title" ? "tw:text-title tw:compact:text-section" : "tw:text-section",
              )}
            >
              {slide.title}
            </h2>
            {slide.points.length > 0 ? (
              <ul className="tw:m-0 tw:flex tw:flex-col tw:gap-2 tw:ps-5 tw:text-body-large">
                {slide.points.map((point, position) => (
                  <li key={position} className="tw:break-words">
                    {point}
                  </li>
                ))}
              </ul>
            ) : null}
          </section>
          {slide.notes.trim() ? (
            <section aria-label="Speaker notes" className="tw:flex tw:flex-col tw:gap-1">
              <h3 className="tw:m-0 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground">
                Speaker notes
              </h3>
              <p className="tw:m-0 tw:whitespace-pre-line tw:break-words">{slide.notes}</p>
            </section>
          ) : null}
          <div className="tw:flex tw:items-center tw:justify-between tw:gap-3 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:py-3">
            <Button className="tw:rounded-full" disabled={index === 0} onClick={() => show(index - 1)}>
              <Icon name="chevron-left" size={16} /> Back
            </Button>
            <span className="tw:text-small tw:tabular-nums tw:text-muted-foreground">
              Slide {index + 1} of {slides.length}
            </span>
            <Button
              variant="primary"
              className="tw:rounded-full"
              disabled={index === slides.length - 1}
              onClick={() => show(index + 1)}
            >
              Next <Icon name="chevron-right" size={16} />
            </Button>
          </div>
        </article>
      </div>
      <audio ref={voice.audioRef} className="tw:hidden" />
    </LectureFrame>
  );
}

function LectureFrame({
  lessonTitle,
  onClose,
  children,
}: {
  lessonTitle: string;
  onClose: () => void;
  children: React.ReactNode;
}) {
  return (
    <div data-testid="lecture-view" className="tw:flex tw:flex-col tw:gap-4">
      <nav aria-label="Breadcrumb">
        <Button size="small" variant="text" className="tw:ps-0" onClick={onClose}>
          <Icon name="chevron-left" size={16} /> {lessonTitle}
        </Button>
      </nav>
      {children}
    </div>
  );
}
