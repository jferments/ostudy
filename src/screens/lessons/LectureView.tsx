import { useEffect, useId, useMemo, useRef, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import { publishPreparedExport, releasePreparedExport } from "../../ipc/externalFiles";
import {
  answerLectureQuestion,
  getLessonLecture,
  getLessonLectureScript,
  narrateLecturePart,
  prepareLessonLecturePptx,
  writeLessonLectureScript,
  type AnsweredCheck,
  type Lecture,
  type LectureAnswer,
  type LectureScript,
  type LectureScriptSlide,
  type LectureSlide,
  type LessonCheckResult,
  type LessonStep,
} from "../../ipc/lessons";
import type { StudyGuidePdfTheme } from "../../ipc/studyGuides";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Dialog } from "../../ui/overlays/Dialog";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import { Button } from "../../ui/primitives/Button";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { RadioGroup } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { ItemTrigger } from "../../ui/primitives/Item";
import { SlideCards } from "./SlideCards";
import { StepCheck } from "./StepCheck";
import { useNarrationPlayer } from "./useNarrationPlayer";
import { InkEditor } from "../ink/InkEditor";
import { InkOverlay } from "../ink/InkOverlay";
import { useMaterialInk } from "../ink/useMaterialInk";
import { readSlideTheme, saveSlideTheme, useSlideDesigns } from "./useSlideDesigns";
import { useVoiceAvailable } from "./useVoiceAvailable";

const SLIDE_THEMES: ReadonlyArray<{ value: StudyGuidePdfTheme; name: string }> = [
  { value: "navy", name: "Navy" },
  { value: "sage", name: "Sage" },
  { value: "slate", name: "Slate" },
  { value: "plum", name: "Plum" },
];

interface LectureViewProps {
  lessonId: string;
  revisionNumber: number;
  lessonTitle: string;
  /** The lesson's subject, whose decks a slide's cards go to first. */
  subjectId: string;
  /** The step the lesson was on, so the lecture opens at its slide. */
  stepKey: string | null;
  /** The lesson's steps, whose checks are answered beside their slides. */
  steps: LessonStep[];
  answered: Record<string, AnsweredCheck>;
  onAnswered: (result: LessonCheckResult) => void;
  onClose: () => void;
  showToast: ShowToast;
}

/** A slide's designed picture is laid out at 720 by 405 points. */
const SLIDE_WIDTH = 720;
const SLIDE_HEIGHT = 405;

/** A question the teacher is asking; `resumeAt` is the slide the lecture goes on to. */
interface Asking {
  slide: LectureScriptSlide;
  resumeAt: number | null;
}

/**
 * A lesson as slides, laid out natively in one of the study guide themes;
 * the plain slides show while they are laid out or if that fails. A step's
 * check is answered beside its slide. Once the teacher's lecture is written, the teacher
 * talks over every slide and now and then stops to ask the class a question,
 * answers it and goes on; until then each step's narration is spoken. Play
 * lecture speaks from the current slide and moves on as each one ends;
 * choosing another slide stops it.
 */
export function LectureView({
  lessonId,
  revisionNumber,
  lessonTitle,
  subjectId,
  stepKey,
  steps,
  answered: answeredChecks,
  onAnswered,
  onClose,
  showToast,
}: LectureViewProps) {
  const lane = useRequestLane();
  const scriptLane = useRequestLane();
  const [lecture, setLecture] = useState<Lecture | null>(null);
  // Undefined until read; null when no teacher's lecture is written yet.
  const [script, setScript] = useState<LectureScript | null | undefined>(undefined);
  const [scriptError, setScriptError] = useState<string | null>(null);
  const [writing, setWriting] = useState(false);
  const [asking, setAsking] = useState<Asking | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const [index, setIndex] = useState(0);
  const [lecturing, setLecturing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState<{ kind: "success" | "error"; text: string } | null>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const titleId = useId();
  const voice = useNarrationPlayer(lessonId, revisionNumber);
  const voiceAvailable = useVoiceAvailable();
  const [theme, setTheme] = useState<StudyGuidePdfTheme>(readSlideTheme);
  const designs = useSlideDesigns(lessonId, revisionNumber, theme);
  const inkMaterial = useMemo(() => ({ kind: "lesson" as const, lessonId }), [lessonId]);
  const ink = useMaterialInk(inkMaterial);
  const [inking, setInking] = useState(false);
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

  useEffect(() => {
    void latestResult(scriptLane, () => getLessonLectureScript(lessonId, revisionNumber)).then(
      (value) => {
        if (value === undefined) return;
        setScript(value);
        setScriptError(null);
      },
      (error: unknown) => {
        setScript(null);
        setScriptError(`The teacher's lecture could not be read: ${getErrorMessage(error)}`);
      },
    );
  }, [scriptLane, lessonId, revisionNumber]);

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
  const taught = (item: LectureSlide | undefined) => script?.slides.find((part) => part.slideKey === item?.key);
  const canLecture = script ? true : slides.some((item) => spoken(item));
  const talk = taught(slide);
  const image = designs.images.get(slide.key);
  const step = steps.find((item) => item.key === slide.stepKey);
  const chooseTheme = (next: StudyGuidePdfTheme) => {
    setTheme(next);
    saveSlideTheme(next);
  };

  // The teacher talks over each slide in turn and stops after one that asks
  // the class a question; answering it goes on from the next slide.
  const teach = (from: number) => {
    const item = slides[from];
    const part = taught(item);
    if (!item || !part) {
      setLecturing(false);
      return;
    }
    setIndex(from);
    voice.speak(
      () => narrateLecturePart(lessonId, revisionNumber, item.key),
      () => (part.question ? setAsking({ slide: part, resumeAt: from + 1 }) : teach(from + 1)),
    );
  };

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
    if (script) {
      teach(index);
      return;
    }
    // From a title slide, the narration starts at the first step.
    speakFrom(slide.kind === "step" ? index : slides.findIndex((item) => spoken(item)));
  };

  const stopLecture = () => {
    voice.stop();
    setLecturing(false);
  };

  const closeQuestion = (goOn: boolean) => {
    const resumeAt = asking?.resumeAt ?? null;
    setAsking(null);
    if (goOn && resumeAt !== null) teach(resumeAt);
    else stopLecture();
  };

  // The teacher's reply is spoken only during a lecture; the lecture goes on
  // when it ends.
  const answered = (optionId: string) => {
    if (!asking || asking.resumeAt === null) return;
    const slideKey = asking.slide.slideKey;
    voice.speak(
      () => narrateLecturePart(lessonId, revisionNumber, slideKey, optionId),
      () => closeQuestion(true),
    );
  };

  const writeScript = () => {
    setWriting(true);
    setScriptError(null);
    writeLessonLectureScript(lessonId, revisionNumber).then(
      (value) => {
        setScript(value);
        setWriting(false);
      },
      (error: unknown) => {
        setScriptError(`The teacher's lecture was not written: ${getErrorMessage(error)}`);
        setWriting(false);
      },
    );
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

  if (inking) {
    const pages = slides.flatMap((item) => {
      const picture = designs.images.get(item.key);
      return picture ? [{ key: item.key, width: SLIDE_WIDTH, height: SLIDE_HEIGHT, image: picture }] : [];
    });
    return (
      <InkEditor material={inkMaterial} pages={pages} pageKey={slide.key} title={lecture.title}
        description="Lecture slides" closeLabel="Back to the lecture" showToast={showToast}
        onClose={() => {
          setInking(false);
          ink.reload();
        }} />
    );
  }

  return (
    <LectureFrame lessonTitle={lessonTitle} onClose={onClose}>
      <header className="tw:flex tw:flex-wrap tw:items-end tw:justify-between tw:gap-3">
        <div className="tw:flex tw:min-w-0 tw:flex-col tw:gap-1">
          <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
            {slides.length} slides · {script ? "the notes are the teacher's lecture" : "the notes are the narration"}
          </p>
          <h1 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight tw:break-words">
            {lecture.title} · lecture
          </h1>
        </div>
        <div className="tw:flex tw:flex-wrap tw:gap-2">
          <OverflowMenu
            label={`Theme: ${SLIDE_THEMES.find((option) => option.value === theme)?.name ?? "Navy"}`}
            triggerPresentation="text"
            className="tw:rounded-full"
            items={SLIDE_THEMES.map((option) => ({
              id: option.value,
              label: option.name,
              checked: option.value === theme,
              onSelect: () => chooseTheme(option.value),
            }))}
          />
          {script === null ? (
            <Button className="tw:rounded-full" disabled={writing || lecturing} onClick={writeScript}>
              <Icon name="sparkles" size={16} /> {writing ? "Writing the lecture…" : "Write the teacher's lecture"}
            </Button>
          ) : null}
          {designs.images.size === slides.length ? (
            <Button className="tw:rounded-full" disabled={lecturing} onClick={() => setInking(true)}>
              <Icon name="pen" size={16} /> Write on slides
            </Button>
          ) : null}
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
      {script === null && !scriptError ? (
        <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
          A teacher's lecture explains each slide in the teacher's own words and stops to ask the class a few questions.
          Writing it uses your AI connection once; it is kept for this version of the lesson.
        </p>
      ) : null}
      {scriptError ? <InlineNotice kind="error">{scriptError}</InlineNotice> : null}
      {saved ? <InlineNotice kind={saved.kind}>{saved.text}</InlineNotice> : null}
      {voice.error ? <InlineNotice kind="error">The lecture could not be read aloud: {voice.error}</InlineNotice> : null}
      <div className="tw:grid tw:items-start tw:gap-6 tw:wide:grid-cols-[11rem_minmax(0,1fr)_20rem] tw:medium:grid-cols-[10rem_minmax(0,1fr)]">
        <nav
          aria-label="Slides"
          className="tw:hidden tw:rounded-lg tw:bg-(--ostudy-color-canvas-inset) tw:p-3 tw:wide:block tw:medium:block"
        >
          <ol className="tw:m-0 tw:flex tw:list-none tw:flex-col tw:gap-0.5 tw:p-0">
            {slides.map((item, position) => (
              <li key={item.key}>
                <ItemTrigger
                  aria-current={position === index ? "step" : undefined}
                  onClick={() => show(position)}
                  className={cn(
                    "tw:w-full tw:items-center tw:gap-2.5 tw:rounded-md tw:px-2 tw:py-2 tw:text-start tw:text-small tw:outline-none tw:hover:bg-accent/40 tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50",
                    position === index && "tw:bg-card tw:font-semibold tw:shadow-sm",
                  )}
                >
                  <span className="tw:flex tw:min-w-0 tw:flex-1 tw:flex-col tw:gap-1">
                    {designs.images.has(item.key) ? (
                      <span className="tw:relative tw:block">
                        <img
                          src={designs.images.get(item.key)}
                          alt=""
                          className="tw:block tw:aspect-video tw:w-full tw:rounded-sm tw:ring-1 tw:ring-border"
                        />
                        <InkOverlay marks={ink.marks.get(item.key) ?? []} width={SLIDE_WIDTH} height={SLIDE_HEIGHT} />
                      </span>
                    ) : null}
                    <span className="tw:flex tw:gap-2">
                      <span aria-hidden="true" className="tw:w-4 tw:shrink-0 tw:text-caption tw:tabular-nums tw:text-muted-foreground">
                        {position + 1}
                      </span>
                      <span className="tw:min-w-0 tw:flex-1 tw:break-words">{item.title}</span>
                    </span>
                  </span>
                </ItemTrigger>
              </li>
            ))}
          </ol>
        </nav>
        <article aria-labelledby={titleId} className="tw:flex tw:min-w-0 tw:flex-col tw:gap-4">
          <section
            data-testid="lecture-slide"
            className={cn(
              "tw:flex tw:aspect-video tw:flex-col tw:gap-4 tw:overflow-auto tw:rounded-lg",
              image
                ? "tw:relative tw:overflow-hidden tw:shadow-md tw:ring-1 tw:ring-border"
                : "tw:border tw:border-solid tw:border-border tw:bg-card tw:p-8 tw:shadow-sm tw:compact:p-5",
            )}
          >
            {image ? <img src={image} alt="" className="tw:block tw:size-full" /> : null}
            {image ? <InkOverlay marks={ink.marks.get(slide.key) ?? []} width={SLIDE_WIDTH} height={SLIDE_HEIGHT} /> : null}
            {/* The designed slide is a picture of this text, which stays for screen readers. */}
            <div className={image ? "tw:sr-only" : "tw:contents"}>
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
            </div>
          </section>
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
        <aside aria-label="With this slide" className="tw:flex tw:min-w-0 tw:flex-col tw:gap-4 tw:medium:col-start-2">
          {(talk?.talk ?? slide.notes).trim() ? (
            <section aria-label="Speaker notes" className="tw:flex tw:flex-col tw:gap-1">
              <h3 className="tw:m-0 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground">
                {talk ? "The teacher says" : "Speaker notes"}
              </h3>
              <p className="tw:m-0 tw:whitespace-pre-line tw:break-words">{talk?.talk ?? slide.notes}</p>
            </section>
          ) : null}
          {talk?.question && !lecturing ? (
            <div>
              <Button className="tw:rounded-full" onClick={() => setAsking({ slide: talk, resumeAt: null })}>
                <Icon name="help" size={16} /> Answer the teacher's question
              </Button>
            </div>
          ) : null}
          {step?.check ? (
            <StepCheck
              key={`${revisionNumber}-${step.key}`}
              lessonId={lessonId}
              revisionNumber={revisionNumber}
              step={step}
              answered={answeredChecks[step.key] ?? null}
              onAnswered={onAnswered}
            />
          ) : null}
          {step ? <SlideCards key={step.key} subjectId={subjectId} step={step} showToast={showToast} /> : null}
        </aside>
      </div>
      {asking?.slide.question ? (
        <ClassQuestionDialog
          key={asking.slide.slideKey}
          lessonId={lessonId}
          revisionNumber={revisionNumber}
          slide={asking.slide}
          lecturing={asking.resumeAt !== null}
          replying={voice.listening}
          replyError={voice.error}
          onAnswered={answered}
          onClose={closeQuestion}
        />
      ) : null}
      <audio ref={voice.audioRef} className="tw:hidden" />
    </LectureFrame>
  );
}

/**
 * The teacher's question to the class: choose an answer, then hear and read
 * why it is right or wrong. During a lecture the lecture goes on once the
 * reply ends, or at once with Continue.
 */
function ClassQuestionDialog({
  lessonId,
  revisionNumber,
  slide,
  lecturing,
  replying,
  replyError,
  onAnswered,
  onClose,
}: {
  lessonId: string;
  revisionNumber: number;
  slide: LectureScriptSlide;
  lecturing: boolean;
  replying: "idle" | "preparing" | "playing";
  replyError: string | null;
  onAnswered: (optionId: string) => void;
  onClose: (goOn: boolean) => void;
}) {
  const question = slide.question!;
  const [choice, setChoice] = useState("");
  const [answer, setAnswer] = useState<LectureAnswer | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = () => {
    if (!choice || checking) return;
    setChecking(true);
    setError(null);
    answerLectureQuestion(lessonId, revisionNumber, slide.slideKey, choice).then(
      (value) => {
        setAnswer(value);
        setChecking(false);
        onAnswered(choice);
      },
      (failure: unknown) => {
        setError(getErrorMessage(failure));
        setChecking(false);
      },
    );
  };

  return (
    <Dialog
      title="The teacher asks the class"
      width="small"
      busy={checking}
      onClose={() => onClose(false)}
      actions={
        answer ? (
          <Button variant="primary" className="tw:rounded-full" onClick={() => onClose(true)}>
            {lecturing ? "Continue lecture" : "Done"}
          </Button>
        ) : (
          <Button variant="primary" className="tw:rounded-full" disabled={!choice || checking} onClick={submit}>
            Answer
          </Button>
        )
      }
    >
      <p className="tw:m-0 tw:text-body-large tw:font-medium tw:break-words">{question.prompt}</p>
      <RadioGroup
        label={question.prompt}
        value={choice}
        disabled={answer !== null}
        options={question.options.map((option) => ({
          value: option.id,
          label: <span className="tw:break-words">{option.text}</span>,
          className: cn(
            "tw:rounded-md tw:px-3",
            answer && option.id === answer.correctOptionId && "tw:bg-success-soft",
            answer && !answer.correct && option.id === choice && "tw:bg-destructive/10",
          ),
        }))}
        onValueChange={setChoice}
      />
      {error ? <InlineNotice kind="error">The answer was not checked: {error}</InlineNotice> : null}
      {answer ? (
        <div
          role="status"
          className={cn("tw:grid tw:gap-1 tw:rounded-md tw:px-4 tw:py-3", answer.correct ? "tw:bg-success-soft" : "tw:bg-warning-soft")}
        >
          <strong className="tw:font-semibold">{answer.correct ? "Right." : "Not quite."}</strong>
          <span className="tw:break-words">{answer.reply}</span>
          {lecturing && replying === "preparing" ? (
            <span className="tw:text-small tw:text-muted-foreground">The teacher is about to answer…</span>
          ) : null}
        </div>
      ) : null}
      {answer && replyError ? (
        <InlineNotice kind="error">The teacher's reply could not be read aloud: {replyError}</InlineNotice>
      ) : null}
    </Dialog>
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
