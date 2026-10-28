import { useEffect, useState } from "react";

import { useAiFeatures } from "../../controllers/AiFeaturesProvider";
import { useDocumentJobs } from "../../controllers/DocumentJobsProvider";
import { getErrorMessage } from "../../ipc/common";
import type { DocumentSummary } from "../../ipc/documents";
import type { StagedExternalFile } from "../../ipc/externalFiles";
import { generationInstructionsError } from "../../ipc/generationInstructions";
import { getSettings } from "../../ipc/settings";
import { SOURCE_NOTES_PROMPT_MAX_CHARS, writeSourceNotes } from "../../ipc/sourceNotes";
import { loadVoice } from "../lessons/voiceReady";
import {
  createSubject,
  getSubjectLibrary,
  moveDocumentToSubject,
  type Subject,
  type SubjectLibrary,
} from "../../ipc/subjects";
import { selectExternalImports } from "../../platform/files";
import { Icon } from "../../ui/icons/Icon";
import { IconButton } from "../../ui/icons/IconButton";
import { Sheet } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { Checkbox, Textarea } from "../../ui/primitives/FormControls";
import { FormField } from "../../ui/primitives/FormField";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { TextField } from "../../ui/primitives/TextField";
import { formatDocumentType } from "../documents/DocumentProgress";
import { queueStudySet } from "../documents/pendingStudySets";

const MAX_SUBJECT_NAME_CHARS = 120;

type Kind = "flashcards" | "studyGuide" | "lessons" | "podcast";
const KINDS: { key: Kind; label: string }[] = [
  { key: "flashcards", label: "Flashcards" },
  { key: "studyGuide", label: "Study guide" },
  { key: "lessons", label: "Lessons" },
  { key: "podcast", label: "Podcast" },
];

/** A document the set is made from: a file chosen now, or one already in the library. */
type Source = { kind: "file"; file: StagedExternalFile } | { kind: "library"; document: DocumentSummary };

const sourceKey = (source: Source) => source.kind === "file" ? `file:${source.file.id}` : `library:${source.document.id}`;

interface NewStudySetSheetProps {
  subjects: readonly Subject[];
  /** The subject to add materials to, from its menu; a new subject when not given. */
  subjectId?: string;
  onClose: () => void;
  /** Receives the subject the set was made in, and the library when a subject was created. */
  onCreated: (subjectId: string, library: SubjectLibrary | null) => void;
}

/**
 * The one place a study set starts: a subject named for its topic, then
 * documents, a prompt, or both, and what to make from them. A prompt with
 * documents says how to use them; a prompt alone has the AI write the notes
 * the set is made from; a name alone makes an empty subject. The documents
 * are filed in the subject and read; what was asked for starts once they
 * are. Opened from a subject's menu, it adds materials to that subject.
 */
export function NewStudySetSheet({ subjects, subjectId: initialSubject, onClose, onCreated }: NewStudySetSheetProps) {
  const jobs = useDocumentJobs();
  // Without AI a set is a subject and its documents; nothing is made from them.
  const ai = useAiFeatures().enabled;
  const [name, setName] = useState("");
  const [sources, setSources] = useState<Source[]>([]);
  const [browsing, setBrowsing] = useState(false);
  const [prompt, setPrompt] = useState("");
  const [kinds, setKinds] = useState<Record<Kind, boolean>>({
    flashcards: true, studyGuide: true, lessons: true, podcast: false,
  });
  const [audio, setAudio] = useState<{ available: boolean; on: boolean }>({ available: false, on: false });
  const [choosing, setChoosing] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Once a subject is made, trying again adds to it rather than making another.
  const [madeSubject, setMadeSubject] = useState<string | null>(null);

  // Lecture audio starts as the setting says, when a voice is set up.
  useEffect(() => {
    let live = true;
    void Promise.all([getSettings(), loadVoice()]).then(
      ([settings, voice]) => {
        if (!live) return;
        const available = voice.ready;
        setAudio({ available, on: available && settings.lectureAudioWithNewLessons !== false });
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);

  const target = madeSubject ?? initialSubject ?? null;
  const adding = initialSubject !== undefined;
  const addingTo = adding ? subjects.find((subject) => subject.id === initialSubject)?.name ?? "this subject" : null;
  const trimmedPrompt = prompt.trim();
  const trimmedName = name.trim();
  const promptTooLong = trimmedPrompt.length > SOURCE_NOTES_PROMPT_MAX_CHARS;
  const nothingChosen = !kinds.flashcards && !kinds.studyGuide && !kinds.lessons;
  const hasMaterial = sources.length > 0 || (ai && trimmedPrompt !== "");
  const blocked = (adding ? !hasMaterial : trimmedName === "")
    || (ai && hasMaterial && nothingChosen)
    || promptTooLong
    || busy !== null
    || !jobs;
  const chosenIds = new Set(sources.flatMap((source) => source.kind === "library" ? [source.document.id] : []));
  const libraryDocuments = (jobs?.documents ?? []).filter((document) => !chosenIds.has(document.id));

  async function chooseFile(): Promise<void> {
    setChoosing(true);
    setError(null);
    try {
      const chosen = await selectExternalImports("document");
      setSources((current) => {
        const known = new Set(current.map(sourceKey));
        const added = chosen.files.filter((file) => !known.has(`file:${file.id}`));
        return [...current, ...added.map((file) => ({ kind: "file" as const, file }))];
      });
      if (chosen.skipped.length > 0) {
        setError(chosen.skipped.map((file) => `${file.displayName} was not added: ${file.message}`).join(" "));
      }
    } catch (caught: unknown) {
      setError(getErrorMessage(caught));
    } finally {
      setChoosing(false);
    }
  }

  async function fileInSubject(documentId: string, subjectId: string): Promise<SubjectLibrary> {
    const library = await getSubjectLibrary();
    return moveDocumentToSubject({
      documentId, subjectId, expectedDocumentFilingRevision: library.documentFilingRevision,
    });
  }

  async function create(): Promise<void> {
    if (blocked || !jobs) return;
    setError(null);
    let created: SubjectLibrary | null = null;
    let subjectId = target;
    try {
      if (subjectId === null) {
        setBusy("Creating the subject…");
        const current = await getSubjectLibrary();
        created = await createSubject({ name: trimmedName, expectedLibraryRevision: current.libraryRevision });
        const known = new Set(current.subjects.map((subject) => subject.id));
        const made = created.subjects.find((subject) => !known.has(subject.id));
        if (!made) throw new Error("The new subject could not be found.");
        subjectId = made.id;
        setMadeSubject(made.id);
      }
      const inSubject = subjectId;
      if (!hasMaterial) {
        // A name alone makes the subject; its materials can come later.
        onCreated(inSubject, created);
        return;
      }
      let chosen = sources;
      if (chosen.length === 0) {
        setBusy("Writing notes on your topic…");
        chosen = [{ kind: "file", file: await writeSourceNotes(trimmedPrompt) }];
      }
      setBusy("Adding the documents…");
      // The prompt guides the cards too, when it fits as card instructions.
      const instructions = trimmedPrompt !== "" && generationInstructionsError(trimmedPrompt) === null
        ? trimmedPrompt : undefined;
      const documentIds: string[] = [];
      for (const source of chosen) {
        if (source.kind === "library") {
          await fileInSubject(source.document.id, inSubject);
          // A library document that has no cards yet makes its first ones now.
          if (ai && source.document.generatedDeckId === null && source.document.lifecycleState !== "processing") {
            await jobs.retry(source.document, instructions);
          }
          documentIds.push(source.document.id);
          continue;
        }
        const documentId = await jobs.importStagedDocument(source.file, instructions, (id) => fileInSubject(id, inSubject));
        if (documentId) documentIds.push(documentId);
      }
      if (documentIds.length === 0) throw new Error("No document could be added, so nothing was made.");
      if (ai) queueStudySet(inSubject, {
        flashcards: kinds.flashcards,
        studyGuide: kinds.studyGuide,
        lessons: kinds.lessons,
        podcast: kinds.lessons && kinds.podcast,
        lectureAudio: kinds.lessons && audio.on,
        documentIds,
      });
      onCreated(inSubject, created);
    } catch (caught: unknown) {
      setError(getErrorMessage(caught));
      setBusy(null);
    }
  }

  const disabled = busy !== null;
  return (
    <Sheet
      title={addingTo ? `Add to ${addingTo}` : ai ? "New study set" : "New subject"}
      width="large"
      onClose={onClose}
      busy={disabled}
      dismissible={!disabled}
      onSubmit={() => void create()}
      actions={
        <>
          <Button disabled={disabled} onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" loading={disabled} disabled={blocked} data-testid="new-study-set-create">
            {adding ? (ai ? "Add materials" : "Add documents") : ai ? "Make study set" : "Create subject"}
          </Button>
        </>
      }
    >
      <div className="tw:grid tw:gap-6">
        {adding ? null : (
          <FormField label="Subject">
            {madeSubject !== null ? (
              <span className="tw:text-label">{trimmedName}</span>
            ) : (
              <TextField
                data-autofocus="true"
                value={name}
                maxLength={MAX_SUBJECT_NAME_CHARS}
                placeholder="For example, Cardiac physiology"
                autoComplete="off"
                disabled={disabled}
                onChange={(event) => setName(event.currentTarget.value)}
              />
            )}
          </FormField>
        )}

        <fieldset className="tw:m-0 tw:grid tw:min-w-0 tw:border-0 tw:p-0">
          <legend className="tw:mb-2 tw:p-0 tw:text-label tw:font-medium">Documents</legend>
          <div className="tw:grid tw:gap-1.5 tw:rounded-lg tw:border-[1.5px] tw:border-dashed tw:border-border tw:p-2">
            {sources.length > 0 ? (
              <ul className="tw:m-0 tw:grid tw:list-none tw:gap-1.5 tw:p-0">
                {sources.map((source) => {
                  const title = source.kind === "file" ? source.file.displayName : source.document.title;
                  return (
                    <li key={sourceKey(source)} className="tw:flex tw:min-h-12 tw:items-center tw:gap-3 tw:rounded-md tw:bg-muted tw:ps-3 tw:pe-1">
                      <Icon name="document" size={18} />
                      <span className="tw:grid tw:min-w-0 tw:flex-1">
                        <span className="tw:truncate tw:text-label tw:font-medium">{title}</span>
                        {source.kind === "library" ? (
                          <span className="tw:text-small tw:text-muted-foreground">
                            {formatDocumentType(source.document.sourceFormat)} · in your library
                          </span>
                        ) : null}
                      </span>
                      <IconButton
                        icon="close"
                        label={`Remove ${title}`}
                        disabled={disabled}
                        onClick={() => setSources((current) => current.filter((item) => sourceKey(item) !== sourceKey(source)))}
                      />
                    </li>
                  );
                })}
              </ul>
            ) : null}
            <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2 tw:p-1">
              <Button size="small" disabled={choosing || disabled} onClick={() => void chooseFile()} data-testid="new-study-set-add-document">
                <Icon name="upload" size={15} /> Choose files
              </Button>
              {libraryDocuments.length > 0 ? (
                <Button size="small" disabled={disabled} aria-expanded={browsing} onClick={() => setBrowsing((open) => !open)}>
                  <Icon name="library" size={15} /> From the library
                </Button>
              ) : null}
            </div>
            {browsing && libraryDocuments.length > 0 ? (
              <ul aria-label="Documents in your library" className="tw:m-0 tw:grid tw:max-h-56 tw:list-none tw:overflow-auto tw:rounded-md tw:border tw:border-solid tw:border-border tw:p-0">
                {libraryDocuments.map((document) => (
                  <li key={document.id} className="tw:border-0 tw:border-solid tw:border-border tw:not-last:border-b">
                    <label className="tw:flex tw:min-h-11 tw:cursor-pointer tw:items-center tw:gap-3 tw:px-3">
                      <Checkbox
                        checked={false}
                        disabled={disabled}
                        onCheckedChange={(checked) => {
                          if (checked) setSources((current) => [...current, { kind: "library", document }]);
                        }}
                      />
                      <span className="tw:min-w-0 tw:flex-1 tw:truncate tw:text-label">{document.title}</span>
                    </label>
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        </fieldset>

        {ai ? <>
        <FormField
          label="Prompt"
          description={sources.length > 0
            ? "Optional: say how to use the documents, such as what to cover, what to use for detail, or what to skip."
            : "Describe what to learn and the AI writes the notes everything is made from, or add documents."}
          error={promptTooLong ? `Keep it under ${SOURCE_NOTES_PROMPT_MAX_CHARS.toLocaleString()} characters.` : null}
        >
          <Textarea
            rows={4}
            value={prompt}
            disabled={disabled}
            placeholder="For example, make sure the cards cover every topic on the slides, and skip the embryology section."
            onChange={(event) => setPrompt(event.currentTarget.value)}
          />
        </FormField>

        <fieldset className="tw:m-0 tw:grid tw:min-w-0 tw:border-0 tw:p-0">
          <legend className="tw:mb-1 tw:p-0 tw:text-label tw:font-medium">What kinds of study materials do you want to create?</legend>
          <div className="tw:grid tw:gap-0.5">
            {KINDS.map((kind) => {
              const needsLessons = kind.key === "podcast" && !kinds.lessons;
              return (
                <div key={kind.key} className="tw:grid">
                  <label className="tw:flex tw:min-h-11 tw:w-fit tw:cursor-pointer tw:items-center tw:gap-2.5 tw:text-label">
                    <Checkbox
                      checked={kinds[kind.key] && !needsLessons}
                      disabled={disabled || needsLessons}
                      onCheckedChange={(checked) => setKinds((current) => ({ ...current, [kind.key]: checked }))}
                    />
                    {kind.label}
                  </label>
                  {kind.key === "lessons" ? (
                    <label className="tw:ms-8 tw:flex tw:min-h-11 tw:w-fit tw:cursor-pointer tw:items-center tw:gap-2.5 tw:text-label">
                      <Checkbox
                        checked={kinds.lessons && audio.on}
                        disabled={disabled || !kinds.lessons || !audio.available}
                        onCheckedChange={(checked) => setAudio((current) => ({ ...current, on: checked }))}
                      />
                      Lecture audio
                    </label>
                  ) : null}
                </div>
              );
            })}
          </div>
        </fieldset>
        </> : null}
        {busy ? <InlineNotice kind="info">{busy}</InlineNotice> : null}
        {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      </div>
    </Sheet>
  );
}
