import { useState } from "react";

import { useDocumentJobs } from "../../controllers/DocumentJobsProvider";
import { getErrorMessage } from "../../ipc/common";
import type { StagedExternalFile } from "../../ipc/externalFiles";
import { generationInstructionsError } from "../../ipc/generationInstructions";
import { SOURCE_NOTES_PROMPT_MAX_CHARS, writeSourceNotes } from "../../ipc/sourceNotes";
import {
  createSubject,
  getSubjectLibrary,
  moveDocumentToSubject,
  type Subject,
  type SubjectLibrary,
} from "../../ipc/subjects";
import { selectExternalImport } from "../../platform/files";
import { Icon } from "../../ui/icons/Icon";
import { IconButton } from "../../ui/icons/IconButton";
import { Sheet } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { Checkbox, Textarea } from "../../ui/primitives/FormControls";
import { FormField } from "../../ui/primitives/FormField";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { TextField } from "../../ui/primitives/TextField";
import type { MaterialsChoice } from "../documents/SubjectStudyActions";
import { queueStudySet } from "../documents/pendingStudySets";

const MAX_SUBJECT_NAME_CHARS = 120;
const NEW_SUBJECT = "";

type Kind = keyof Required<MaterialsChoice>;
const KINDS: { key: Kind; label: string }[] = [
  { key: "flashcards", label: "Flashcards" },
  { key: "studyGuide", label: "Study guide" },
  { key: "lessons", label: "Lessons" },
  { key: "podcast", label: "Podcast" },
];

interface NewStudySetSheetProps {
  subjects: readonly Subject[];
  /** The subject to add to, or a new one when not given. */
  subjectId?: string;
  onClose: () => void;
  /** Receives the subject the set was made in, and the library when a subject was created. */
  onCreated: (subjectId: string, library: SubjectLibrary | null) => void;
}

/**
 * The one place a study set starts: documents, a prompt, or both, and what to
 * make from them. A prompt with documents says how to use them; a prompt
 * alone has the AI write the notes the set is made from. The documents are
 * filed in the subject and read; what was asked for starts once they are.
 */
export function NewStudySetSheet({ subjects, subjectId: initialSubject, onClose, onCreated }: NewStudySetSheetProps) {
  const jobs = useDocumentJobs();
  const [subjectId, setSubjectId] = useState(initialSubject ?? NEW_SUBJECT);
  const [name, setName] = useState("");
  const [files, setFiles] = useState<StagedExternalFile[]>([]);
  const [prompt, setPrompt] = useState("");
  const [kinds, setKinds] = useState<Required<MaterialsChoice>>({
    flashcards: true, studyGuide: true, lessons: true, podcast: false,
  });
  const [choosing, setChoosing] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const trimmedPrompt = prompt.trim();
  const trimmedName = name.trim();
  const promptTooLong = trimmedPrompt.length > SOURCE_NOTES_PROMPT_MAX_CHARS;
  const nothingChosen = !kinds.flashcards && !kinds.studyGuide && !kinds.lessons;
  const blocked = (files.length === 0 && trimmedPrompt === "")
    || (subjectId === NEW_SUBJECT && trimmedName === "")
    || nothingChosen
    || promptTooLong
    || busy !== null
    || !jobs;

  async function addFile(): Promise<void> {
    setChoosing(true);
    setError(null);
    try {
      const chosen = await selectExternalImport("document");
      if (chosen) setFiles((current) => current.some((file) => file.id === chosen.id) ? current : [...current, chosen]);
    } catch (caught: unknown) {
      setError(getErrorMessage(caught));
    } finally {
      setChoosing(false);
    }
  }

  async function create(): Promise<void> {
    if (blocked || !jobs) return;
    setError(null);
    let created: SubjectLibrary | null = null;
    let target = subjectId;
    try {
      if (target === NEW_SUBJECT) {
        setBusy("Creating the subject…");
        const current = await getSubjectLibrary();
        created = await createSubject({ name: trimmedName, expectedLibraryRevision: current.libraryRevision });
        const known = new Set(current.subjects.map((subject) => subject.id));
        const made = created.subjects.find((subject) => !known.has(subject.id));
        if (!made) throw new Error("The new subject could not be found.");
        target = made.id;
        // A failure past here keeps the subject, so trying again adds to it.
        setSubjectId(target);
      }
      let sources = files;
      if (sources.length === 0) {
        setBusy("Writing notes on your topic…");
        sources = [await writeSourceNotes(trimmedPrompt)];
      }
      setBusy("Adding the documents…");
      // The prompt guides the cards too, when it fits as card instructions.
      const instructions = trimmedPrompt !== "" && generationInstructionsError(trimmedPrompt) === null
        ? trimmedPrompt : undefined;
      const documentIds: string[] = [];
      for (const source of sources) {
        const documentId = await jobs.importStagedDocument(source, instructions, async (id) => {
          const library = await getSubjectLibrary();
          return moveDocumentToSubject({
            documentId: id, subjectId: target, expectedDocumentFilingRevision: library.documentFilingRevision,
          });
        });
        if (documentId) documentIds.push(documentId);
      }
      if (documentIds.length === 0) throw new Error("No document could be added, so nothing was made.");
      queueStudySet(target, { ...kinds, podcast: kinds.lessons && kinds.podcast, documentIds });
      onCreated(target, created);
    } catch (caught: unknown) {
      setError(getErrorMessage(caught));
      setBusy(null);
    }
  }

  return (
    <Sheet
      title="New study set"
      description="Add documents, describe what to learn, or both."
      width="small"
      onClose={onClose}
      busy={busy !== null}
      dismissible={busy === null}
      onSubmit={() => void create()}
      actions={
        <>
          <Button disabled={busy !== null} onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" loading={busy !== null} disabled={blocked} data-testid="new-study-set-create">
            <Icon name="sparkles" size={16} /> Create study set
          </Button>
        </>
      }
    >
      <div className="tw:grid tw:gap-5">
        <FormField label="Subject">
          <NativeSelect value={subjectId} disabled={busy !== null} onChange={(event) => setSubjectId(event.currentTarget.value)}>
            <option value={NEW_SUBJECT}>A new subject</option>
            {subjects.map((subject) => <option key={subject.id} value={subject.id}>{subject.name}</option>)}
          </NativeSelect>
        </FormField>
        {subjectId === NEW_SUBJECT ? (
          <FormField label="Name">
            <TextField
              data-autofocus="true"
              value={name}
              maxLength={MAX_SUBJECT_NAME_CHARS}
              placeholder="For example, Cardiac physiology"
              autoComplete="off"
              disabled={busy !== null}
              onChange={(event) => setName(event.currentTarget.value)}
            />
          </FormField>
        ) : null}
        <fieldset className="tw:m-0 tw:grid tw:min-w-0 tw:gap-2 tw:border-0 tw:p-0">
          <legend className="tw:mb-2 tw:p-0 tw:text-label tw:font-medium">Documents</legend>
          {files.length > 0 ? (
            <ul className="tw:m-0 tw:grid tw:list-none tw:rounded-lg tw:border tw:border-solid tw:border-border tw:p-0">
              {files.map((file) => (
                <li key={file.id} className="tw:flex tw:min-h-11 tw:items-center tw:gap-3 tw:border-0 tw:border-solid tw:border-border tw:ps-4 tw:pe-1 tw:not-last:border-b">
                  <Icon name="document" size={16} />
                  <span className="tw:min-w-0 tw:flex-1 tw:truncate tw:text-label">{file.displayName}</span>
                  <IconButton
                    icon="close"
                    label={`Remove ${file.displayName}`}
                    disabled={busy !== null}
                    onClick={() => setFiles((current) => current.filter((item) => item.id !== file.id))}
                  />
                </li>
              ))}
            </ul>
          ) : null}
          <div>
            <Button size="small" disabled={choosing || busy !== null} onClick={() => void addFile()} data-testid="new-study-set-add-document">
              <Icon name="plus" size={15} /> Add a document
            </Button>
          </div>
        </fieldset>
        <FormField
          label="What to learn (optional with documents)"
          description={files.length > 0
            ? "Say how to use the documents, such as what to focus on or leave out."
            : "Describe a topic and the AI writes the notes everything is made from."}
          error={promptTooLong ? `Keep it under ${SOURCE_NOTES_PROMPT_MAX_CHARS.toLocaleString()} characters.` : null}
        >
          <Textarea
            rows={4}
            value={prompt}
            disabled={busy !== null}
            placeholder="For example, the cardiac cycle for my first-year physiology exam"
            onChange={(event) => setPrompt(event.currentTarget.value)}
          />
        </FormField>
        <fieldset className="tw:m-0 tw:grid tw:min-w-0 tw:gap-1 tw:border-0 tw:p-0">
          <legend className="tw:mb-2 tw:p-0 tw:text-label tw:font-medium">Make</legend>
          {KINDS.map((kind) => {
            const disabled = busy !== null || (kind.key === "podcast" && !kinds.lessons);
            return (
              <div key={kind.key} className="tw:flex tw:min-h-11 tw:flex-wrap tw:items-center tw:gap-x-3">
                <label className="tw:flex tw:min-h-11 tw:cursor-pointer tw:items-center tw:gap-3">
                  <Checkbox
                    checked={kinds[kind.key] && !(kind.key === "podcast" && !kinds.lessons)}
                    disabled={disabled}
                    onCheckedChange={(checked) => setKinds((current) => ({ ...current, [kind.key]: checked }))}
                  />
                  <span className="tw:text-label">{kind.label}</span>
                </label>
                {kind.key === "podcast" ? (
                  <span className="tw:text-small tw:text-muted-foreground">
                    {kinds.lessons ? "Two hosts talk through the first lessons" : "Made from the lessons"}
                  </span>
                ) : null}
              </div>
            );
          })}
        </fieldset>
        {busy ? <InlineNotice kind="info">{busy}</InlineNotice> : null}
        {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      </div>
    </Sheet>
  );
}
