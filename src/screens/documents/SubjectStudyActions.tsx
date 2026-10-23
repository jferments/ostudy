import { useEffect, useMemo, useRef, useState } from "react";

import { listDecks, type DeckSummary } from "../../ipc/decks";
import type { DocumentSummary } from "../../ipc/documents";
import { generationInstructionsError } from "../../ipc/generationInstructions";
import type { CourseWritingProgress } from "../../ipc/lessons";
import type { StudyTargetBundleStatus, StudyTargetOutputPolicy } from "../../ipc/studyMaterials";
import type { SubjectLibrary } from "../../ipc/subjects";
import { Sheet } from "../../ui/overlays/Dialog";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { ChipButton } from "../../ui/primitives/Chip";
import { Checkbox, Textarea } from "../../ui/primitives/FormControls";
import { FormField } from "../../ui/primitives/FormField";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { Progress } from "../../ui/primitives/Progress";
import { useSubjectCourse } from "../lessons/useSubjectCourse";
import { formatDocumentType } from "./DocumentProgress";
import { queueStudySet, takeStudySet } from "./pendingStudySets";
import { useStudyTargetBundleController } from "./useStudyTargetBundleController";

interface SubjectStudyActionsProps {
  subjectId: string;
  /** Changes whenever the subject's documents or reading ranges do, so the status rereads. */
  refreshKey: string | number;
  hasDocuments: boolean;
  /** The documents filed in the subject, for From. */
  documents?: readonly DocumentSummary[];
  /** The subject library: which decks the subject holds and which documents its study set leaves out. */
  library?: SubjectLibrary | null;
  /** Leaves exactly these documents out of the subject's study materials. */
  onSetLeftOut?: (documentIds: string[]) => Promise<boolean>;
  /** Starts a document's first cards, into a chosen deck or a new one of its own. */
  onMakeFirstCards?: (document: DocumentSummary, instructions: string, deckId: string | undefined) => Promise<void>;
  /** True while a document is being processed or making cards. */
  isDocumentWorking?: (documentId: string) => boolean;
  refreshDecks?: () => Promise<unknown>;
  /** Studies the decks the subject's materials made. */
  onStudy?: (subjectId: string) => void;
  onOpenGuide?: (subjectId: string, title: string) => void;
  /** Set while no model is chosen: Make and Update open AI setup instead. */
  onSetUpAi?: () => void;
}

/** What one Make asks for: the bundle's outputs, and the subject's lessons. */
export interface MaterialsChoice extends StudyTargetOutputPolicy {
  lessons: boolean;
  /** A podcast about the first lessons, once they are written. */
  podcast?: boolean;
}

/** One Make from the sheet: what to make, from which documents, and how to write new cards. */
export interface MakeRequest extends MaterialsChoice {
  documentIds: string[];
  /** The deck new cards go into; null gives each document a deck of its own. */
  deckId: string | null;
  instructions: string;
}

/**
 * The subject's study materials, made from its documents together: Study and
 * Read guide once they exist; otherwise the one action that makes them, and
 * progress with Stop while they are being made. Make study materials opens a
 * sheet that picks the documents, what to make, the deck new cards go into
 * and instructions for them. Documents without cards get their first cards
 * first; the guide and the subject's flashcards are then one native bundle,
 * and the lessons the subject's course, written side by side.
 */
export function SubjectStudyActions({
  subjectId,
  refreshKey,
  hasDocuments,
  documents = [],
  library = null,
  onSetLeftOut,
  onMakeFirstCards,
  isDocumentWorking = (documentId) => documents.some((document) => document.id === documentId && document.lifecycleState === "processing"),
  refreshDecks,
  onStudy,
  onOpenGuide,
  onSetUpAi,
}: SubjectStudyActionsProps) {
  const bundle = useStudyTargetBundleController({
    target: { kind: "collection", id: subjectId },
    changed: refreshDecks,
    refreshKey,
  });
  const course = useSubjectCourse(subjectId);
  const status = bundle.status;
  const [choosing, setChoosing] = useState(false);
  // The bundle and lessons wait for the cards a Make started, since both read them.
  const [waiting, setWaiting] = useState<{ choice: MaterialsChoice; documentIds: string[] } | null>(null);
  const [makeError, setMakeError] = useState<string | null>(null);
  // A document not listed yet has only just been added, so it is still to be read.
  const stillWorking = waiting?.documentIds.some(
    (documentId) => isDocumentWorking(documentId) || !documents.some((document) => document.id === documentId),
  ) ?? false;
  // A New study set made for this subject waits here for its documents.
  useEffect(() => {
    const set = takeStudySet(subjectId);
    if (set) setWaiting({ choice: set, documentIds: set.documentIds });
  }, [subjectId]);
  // A page that closes while waiting hands the wait back for the next one.
  const latestWaiting = useRef(waiting);
  latestWaiting.current = waiting;
  useEffect(() => () => {
    const left = latestWaiting.current;
    if (left) queueStudySet(subjectId, { ...left.choice, documentIds: left.documentIds });
  }, [subjectId]);
  useEffect(() => {
    if (!waiting || stillWorking) return;
    setWaiting(null);
    startOutputs(waiting.choice);
    // startOutputs reads the latest controllers; only the wait ending matters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [waiting, stillWorking]);

  // A course is written once; rewriting one is a separate decision.
  const lessonsMade = (course.course?.lessons.length ?? 0) > 0;
  const lessonsKnown = course.course !== null;
  if (!hasDocuments && !status && !lessonsMade && !waiting) return null;

  function startOutputs(choice: MaterialsChoice): void {
    if (choice.flashcards || choice.studyGuide) {
      void bundle.run({ flashcards: choice.flashcards, studyGuide: choice.studyGuide });
    }
    if (choice.lessons) course.write({ podcast: choice.podcast ?? false });
  }

  async function make(request: MakeRequest): Promise<void> {
    setChoosing(false);
    setMakeError(null);
    const chosen = new Set(request.documentIds);
    const leftOut = documents.filter((document) => !chosen.has(document.id)).map((document) => document.id).sort();
    const current = documents.filter((document) => library?.leftOutDocumentIds.includes(document.id)).map((document) => document.id).sort();
    if (onSetLeftOut && leftOut.join() !== current.join() && !await onSetLeftOut(leftOut)) {
      setMakeError("The documents to use could not be saved, so nothing was made.");
      return;
    }
    const firstCards = request.flashcards
      ? documents.filter((document) => chosen.has(document.id) && needsFirstCards(document) && !isDocumentWorking(document.id))
      : [];
    if (firstCards.length === 0 || !onMakeFirstCards) return startOutputs(request);
    for (const document of firstCards) {
      await onMakeFirstCards(document, request.instructions, request.deckId ?? undefined);
    }
    setWaiting({ choice: request, documentIds: firstCards.map((document) => document.id) });
  }

  if (waiting || bundle.busy || course.writing) {
    const stopping = (!bundle.busy || bundle.canceling) && (!course.writing || course.stopping);
    return (
      <div className="tw:flex tw:min-w-60 tw:flex-1 tw:items-center tw:gap-3" data-testid="subject-study-progress">
        <div className="tw:grid tw:min-w-0 tw:flex-1 tw:gap-2">
          {waiting ? <CardsProgress count={waiting.documentIds.length} /> : null}
          {bundle.busy ? <BundleProgress status={status} /> : null}
          {course.writing ? <LessonsProgress progress={course.progress} podcast={course.writingPodcast} /> : null}
        </div>
        {bundle.busy || course.writing ? (
          <Button
            size="small"
            variant="dangerSecondary"
            disabled={stopping}
            aria-label={stopping ? "Stopping study materials" : "Stop making study materials"}
            onClick={() => {
              if (bundle.busy) void bundle.stop();
              if (course.writing) course.stop();
            }}
          >
            <Icon name="stop" size={16} />
            {stopping ? "Stopping…" : "Stop"}
          </Button>
        ) : null}
      </div>
    );
  }
  const study = status?.flashcards.state === "succeeded" && onStudy ? () => onStudy(subjectId) : undefined;
  const guide = status?.guideId && onOpenGuide ? () => onOpenGuide(subjectId, status.title) : undefined;
  const update = updateLabel(status);
  const needsUpdate = status !== null && (status.sourceChanged || update !== "Update study materials");
  const canMake = hasDocuments && (!status || (lessonsKnown && !lessonsMade));
  return (
    <>
      {study ? <Button onClick={study}>Study</Button> : null}
      {guide ? <Button onClick={guide}>Read guide</Button> : null}
      {canMake ? (
        <Button data-testid="subject-make-materials" disabled={bundle.loading} onClick={onSetUpAi ?? (() => setChoosing(true))}>
          <Icon name="sparkles" size={16} /> Make study materials
        </Button>
      ) : null}
      {needsUpdate && update ? (
        <Button disabled={bundle.loading} onClick={onSetUpAi ?? (() => void bundle.run())}>{update}</Button>
      ) : null}
      {makeError ? <InlineNotice kind="warning" className="tw:basis-full">{makeError}</InlineNotice> : null}
      {bundle.error ? (
        <InlineNotice kind="warning" className="tw:basis-full">{bundle.error}</InlineNotice>
      ) : null}
      {course.writeError ? (
        <InlineNotice kind="warning" className="tw:basis-full">{`Lessons were not written: ${course.writeError}`}</InlineNotice>
      ) : null}
      {course.podcastError ? (
        <InlineNotice kind="warning" className="tw:basis-full">{`The podcast was not written: ${course.podcastError}`}</InlineNotice>
      ) : null}
      {choosing ? (
        <MakeMaterialsSheet
          subjectId={subjectId}
          documents={documents}
          library={library}
          made={{ studyGuide: status !== null, flashcards: status !== null, lessons: lessonsMade }}
          onClose={() => setChoosing(false)}
          onMake={(request) => void make(request)}
        />
      ) : null}
    </>
  );
}

/** A document that has not made cards yet, and can. */
function needsFirstCards(document: DocumentSummary): boolean {
  return document.generatedDeckId === null && document.lifecycleState !== "processing";
}

/**
 * What to make for the subject, and from what. Everything not yet made is
 * ticked to start, and so is every document the subject's study set uses.
 * Flashcards are made when the subject's flashcards are not set up yet or a
 * chosen document has no cards; the deck and instructions are for those
 * documents' new cards.
 */
function MakeMaterialsSheet({ subjectId, documents, library, made, onClose, onMake }: {
  subjectId: string;
  documents: readonly DocumentSummary[];
  library: SubjectLibrary | null;
  made: MaterialsChoice;
  onClose: () => void;
  onMake: (request: MakeRequest) => void;
}) {
  const leftOut = library?.leftOutDocumentIds ?? [];
  const [chosen, setChosen] = useState<ReadonlySet<string>>(
    () => new Set(documents.filter((document) => !leftOut.includes(document.id)).map((document) => document.id)),
  );
  const chosenDocuments = documents.filter((document) => chosen.has(document.id));
  const writesCards = chosenDocuments.some(needsFirstCards);
  const [choice, setChoice] = useState<MaterialsChoice>({
    studyGuide: !made.studyGuide, flashcards: !made.flashcards || writesCards, lessons: !made.lessons,
  });
  const [deckId, setDeckId] = useState("");
  const [instructions, setInstructions] = useState("");
  const decks = useSubjectDecks(subjectId, library);
  const cardOptions = choice.flashcards && writesCards;
  const instructionsError = cardOptions ? generationInstructionsError(instructions) : null;
  const flashcardsMade = made.flashcards && !writesCards;
  const nothing = !choice.flashcards && !choice.studyGuide && !choice.lessons;
  const blocked = nothing || (documents.length > 0 && chosen.size === 0) || instructionsError !== null;
  const options: { key: keyof MaterialsChoice; label: string; made: boolean }[] = [
    { key: "studyGuide", label: "Study guide", made: made.studyGuide },
    { key: "flashcards", label: "Flashcards", made: flashcardsMade },
    { key: "lessons", label: "Lessons", made: made.lessons },
  ];
  return (
    <Sheet
      title="Make study materials"
      width="small"
      onClose={onClose}
      onSubmit={(event) => {
        event.preventDefault();
        if (blocked) return;
        onMake({
          ...choice,
          flashcards: choice.flashcards && !flashcardsMade,
          documentIds: chosenDocuments.map((document) => document.id),
          deckId: cardOptions && deckId ? deckId : null,
          instructions: cardOptions ? instructions : "",
        });
      }}
      actions={<>
        <Button variant="secondary" onClick={onClose}>Cancel</Button>
        <Button type="submit" disabled={blocked}><Icon name="sparkles" size={16} /> Make</Button>
      </>}
    >
      <div className="tw:grid tw:gap-5">
        {documents.length > 0 ? (
          <fieldset className="tw:m-0 tw:grid tw:min-w-0 tw:border-0 tw:p-0">
            <legend className="tw:mb-2 tw:p-0 tw:text-label tw:font-medium">From</legend>
            <ul className="tw:m-0 tw:grid tw:list-none tw:rounded-lg tw:border tw:border-solid tw:border-border tw:p-0">
              {documents.map((document) => (
                <li key={document.id} className="tw:border-0 tw:border-solid tw:border-border tw:not-last:border-b">
                  <label className="tw:flex tw:min-h-11 tw:cursor-pointer tw:items-center tw:gap-3 tw:px-4">
                    <Checkbox
                      checked={chosen.has(document.id)}
                      onCheckedChange={(checked) => setChosen((current) => {
                        const next = new Set(current);
                        if (checked) next.add(document.id); else next.delete(document.id);
                        return next;
                      })}
                    />
                    <span className="tw:min-w-0 tw:flex-1 tw:truncate tw:text-label tw:font-medium">{document.title}</span>
                    <span className="tw:shrink-0 tw:text-small tw:text-muted-foreground">{documentLength(document)}</span>
                  </label>
                </li>
              ))}
            </ul>
          </fieldset>
        ) : null}
        <fieldset className="tw:m-0 tw:border-0 tw:p-0">
          <legend className="tw:mb-2 tw:p-0 tw:text-label tw:font-medium">Make</legend>
          <div className="tw:flex tw:flex-wrap tw:gap-2">
            {options.map((option) => {
              const on = !option.made && choice[option.key];
              return (
                <ChipButton
                  key={option.key}
                  selected={on}
                  disabled={option.made}
                  aria-label={option.made ? `${option.label}, made` : option.label}
                  onClick={() => setChoice((current) => ({ ...current, [option.key]: !current[option.key] }))}
                >
                  {on || option.made ? <Icon name="check" size={14} /> : null}
                  {option.label}
                </ChipButton>
              );
            })}
          </div>
        </fieldset>
        {cardOptions ? <>
          <FormField label="Cards go into">
            <NativeSelect value={deckId} onChange={(event) => setDeckId(event.currentTarget.value)}>
              <option value="">A new deck for each document</option>
              {decks.map((deck) => <option key={deck.id} value={deck.id}>{deck.name}</option>)}
            </NativeSelect>
          </FormField>
          <FormField label="Instructions (optional)" error={instructionsError}>
            <Textarea rows={3} value={instructions} onChange={(event) => setInstructions(event.currentTarget.value)} />
          </FormField>
        </> : null}
      </div>
    </Sheet>
  );
}

/** The subject's own decks, in its order, that new cards can go into. */
function useSubjectDecks(subjectId: string, library: SubjectLibrary | null): DeckSummary[] {
  const [decks, setDecks] = useState<DeckSummary[]>([]);
  useEffect(() => {
    let active = true;
    void listDecks().then((rows) => { if (active) setDecks(rows); }, () => undefined);
    return () => { active = false; };
  }, []);
  return useMemo(() => {
    const placements = (library?.decks ?? []).filter((placement) => placement.subjectId === subjectId);
    const byId = new Map(decks.filter((deck) => deck.archivedAt === null).map((deck) => [deck.id, deck]));
    return [...placements]
      .sort((left, right) => left.sortOrder - right.sortOrder)
      .map((placement) => byId.get(placement.deckId))
      .filter((deck): deck is DeckSummary => deck !== undefined);
  }, [decks, library, subjectId]);
}

function documentLength(document: DocumentSummary): string {
  const count = document.sourceUnitCount;
  if (count > 0 && document.sourceUnitKind) {
    return `${count.toLocaleString()} ${document.sourceUnitKind.replaceAll("_", " ")}${count === 1 ? "" : "s"}`;
  }
  return formatDocumentType(document.sourceFormat);
}

function CardsProgress({ count }: { count: number }) {
  const label = `${count} document${count === 1 ? "" : "s"}`;
  return (
    <div className="tw:grid tw:min-w-0 tw:gap-1.5" role="status" aria-live="polite">
      <div className="tw:flex tw:items-center tw:justify-between tw:gap-3 tw:text-small">
        <span className="tw:truncate">Making flashcards</span>
        <span className="tw:shrink-0 tw:text-muted-foreground">{label}</span>
      </div>
      <Progress value={null} max={100} label="Flashcards progress" valueText={label} />
    </div>
  );
}

function LessonsProgress({ progress, podcast }: { progress: CourseWritingProgress | null; podcast: boolean }) {
  const planned = progress?.planned ?? null;
  if (podcast) {
    return (
      <div className="tw:grid tw:min-w-0 tw:gap-1.5" role="status" aria-live="polite">
        <div className="tw:flex tw:items-center tw:justify-between tw:gap-3 tw:text-small">
          <span className="tw:truncate">Writing the podcast</span>
          <span className="tw:shrink-0 tw:text-muted-foreground">Working…</span>
        </div>
        <Progress value={null} max={100} label="Podcast progress" valueText="Working…" />
      </div>
    );
  }
  const label = planned === null ? "Planning…" : `${progress?.written ?? 0} / ${planned}`;
  return (
    <div className="tw:grid tw:min-w-0 tw:gap-1.5" role="status" aria-live="polite">
      <div className="tw:flex tw:items-center tw:justify-between tw:gap-3 tw:text-small">
        <span className="tw:truncate">Writing lessons</span>
        <span className="tw:shrink-0 tw:text-muted-foreground">{label}</span>
      </div>
      <Progress value={planned === null ? null : progress?.written ?? 0} max={planned ?? 100} label="Lessons progress" valueText={label} />
    </div>
  );
}

function BundleProgress({ status }: { status: StudyTargetBundleStatus | null }) {
  const active = bundleProgress(status);
  return (
    <div className="tw:grid tw:min-w-0 tw:flex-1 tw:gap-1.5" role="status" aria-live="polite">
      <div className="tw:flex tw:items-center tw:justify-between tw:gap-3 tw:text-small">
        <span className="tw:truncate">{active.title}</span>
        <span className="tw:shrink-0 tw:text-muted-foreground">{active.label}</span>
      </div>
      <Progress value={active.value} max={active.max} label="Study materials progress" valueText={active.label} />
    </div>
  );
}

function bundleProgress(status: StudyTargetBundleStatus | null): {
  title: string;
  label: string;
  value: number | null;
  max: number;
} {
  if (!status) return { title: "Preparing study materials", label: "Starting…", value: null, max: 100 };
  const guide = status.studyGuide;
  if (guide.state === "running" || guide.state === "committing") {
    const determinate = guide.totalUnits > 0;
    return {
      title: "Writing the study guide",
      label: determinate ? `${guide.completedUnits} / ${guide.totalUnits}` : "Working…",
      value: determinate ? guide.completedUnits : null,
      max: determinate ? guide.totalUnits : 100,
    };
  }
  const cards = status.flashcards;
  if (cards.state === "running" || cards.state === "committing") {
    const determinate = cards.totalUnits > 0;
    return {
      title: "Making flashcards",
      label: determinate ? `${cards.completedUnits} / ${cards.totalUnits}` : "Working…",
      value: determinate ? cards.completedUnits : null,
      max: determinate ? cards.totalUnits : 100,
    };
  }
  if (cards.state === "succeeded" && guide.state === "pending") {
    return { title: "Preparing the study guide", label: "Working…", value: null, max: 100 };
  }
  return { title: "Preparing study materials", label: "Working…", value: null, max: 100 };
}

function updateLabel(status: StudyTargetBundleStatus | null): string | null {
  if (!status) return null;
  if (status.flashcards.state === "suppressed" || status.studyGuide.state === "suppressed") return "Recreate study materials";
  if (["failed", "interrupted", "canceled"].includes(status.latestBatchState ?? "")
    || status.flashcards.state === "failed" || status.studyGuide.state === "failed") return "Retry study materials";
  return "Update study materials";
}
