import { useCallback, useEffect, useRef, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { getErrorMessage } from "../../ipc/common";
import {
  createNotebook,
  deleteNotebook,
  listNotebooks,
  moveNotebookToSubject,
  renameNotebook,
  type NotebookSummary,
  type PageSize,
  type Paper,
} from "../../ipc/notebooks";
import type { LibrarySubjectScope } from "../../navigation/appDestinations";
import { Icon } from "../../ui/icons/Icon";
import { Dialog } from "../../ui/overlays/Dialog";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { EmptyState, LoadingState } from "../../ui/primitives/EmptyState";
import { ItemTrigger } from "../../ui/primitives/Item";
import { FormField } from "../../ui/primitives/FormField";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { SegmentedControl } from "../../ui/primitives/SegmentedControl";
import { TextField } from "../../ui/primitives/TextField";
import { useLibrarySubjects, type LibrarySubjects } from "../library/LibrarySubjects";
import { LabeledChoice } from "./LabeledChoice";
import { NotebookEditor, type NotebookLinkActions } from "./NotebookEditor";
import { NotebookThumbnail, PAPER_LABELS } from "./paper";

interface LibraryNotesProps extends Omit<NotebookLinkActions, "onOpenNotebook"> {
  subject: LibrarySubjectScope;
  showToast: ShowToast;
  /** A notebook to open at one of its pages, such as one a document page was clipped into. */
  openRequest?: NotebookOpenRequest | null;
  onOpenRequestConsumed?: (requestId: number) => void;
}

export interface NotebookOpenRequest {
  requestId: number;
  notebookId: string;
  pageId: string;
}

const PAGE_SIZE_OPTIONS = [
  { value: "letter", label: "Letter" },
  { value: "a4", label: "A4" },
  { value: "wide", label: "Wide" },
] as const;

const PAPER_OPTIONS = [
  { value: "blank", label: PAPER_LABELS.blank },
  { value: "lined", label: PAPER_LABELS.lined },
  { value: "grid", label: PAPER_LABELS.grid },
  { value: "dots", label: PAPER_LABELS.dots },
] as const;

/** The longest notebook name; the native side checks it too. */
const MAX_TITLE_CHARS = 200;

/**
 * The Notes destination: notebooks, each shown by its first page, and New
 * notebook. With every subject in scope they are grouped under their subjects.
 * Opening one makes it the page.
 */
export function LibraryNotes({ subject, showToast, openRequest = null, onOpenRequestConsumed, ...links }: LibraryNotesProps) {
  const subjects = useLibrarySubjects(subject);
  const [notebooks, setNotebooks] = useState<NotebookSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState<{ notebookId: string; title: string; pageId?: string } | null>(null);
  const [creating, setCreating] = useState(false);
  const request = useRef(0);

  const refresh = useCallback(() => {
    const current = ++request.current;
    listNotebooks().then((next) => {
      if (request.current !== current) return;
      setNotebooks(next);
      setError(null);
    }, (failure: unknown) => {
      if (request.current === current) setError(getErrorMessage(failure));
    });
  }, []);
  useEffect(() => { refresh(); }, [refresh]);
  useEffect(() => {
    if (!openRequest) return;
    // The notebook names itself once it opens.
    setOpen({ notebookId: openRequest.notebookId, title: "Notes", pageId: openRequest.pageId });
    onOpenRequestConsumed?.(openRequest.requestId);
  }, [onOpenRequestConsumed, openRequest]);

  if (open) {
    return (
      <NotebookEditor
        key={open.notebookId}
        notebookId={open.notebookId}
        title={open.title}
        pageId={open.pageId}
        showToast={showToast}
        onClose={() => {
          setOpen(null);
          refresh();
        }}
        onOpenNotebook={(notebookId, pageId) => {
          const title = notebooks?.find((item) => item.notebook.id === notebookId)?.notebook.title ?? "Notes";
          setOpen({ notebookId, title, pageId });
        }}
        {...links}
      />
    );
  }

  const shown = notebooks?.filter((item) => subject.kind === "all"
    || (subject.kind === "none" ? item.notebook.subjectId === null : item.notebook.subjectId === subject.subjectId)) ?? [];
  const defaultSubjectId = subject.kind === "subject" ? subject.subjectId : null;

  return (
    <div data-testid="library-notes" className="tw:grid tw:min-w-0 tw:gap-4">
      <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
        <h1 className="tw:m-0 tw:text-title tw:leading-tight tw:font-semibold tw:tracking-tight">Notes</h1>
        <Button variant="primary" onClick={() => setCreating(true)}>
          <Icon name="plus" />
          New notebook
        </Button>
      </div>
      {error ? (
        <InlineNotice kind="error">
          <span className="tw:flex tw:flex-wrap tw:items-center tw:gap-2">
            {error}
            <Button size="small" onClick={refresh}>Try again</Button>
          </span>
        </InlineNotice>
      ) : null}
      {notebooks === null && !error ? (
        <LoadingState label="Loading notes…" />
      ) : shown.length === 0 && notebooks !== null ? (
        <EmptyState
          icon="pen"
          title="No notes yet"
          description="A notebook is pages of paper to write and draw on with the PDF tools, and to link to your documents, study guides and flashcards."
        />
      ) : subject.kind === "all" ? (
        groupBySubject(shown, subjects).map((group) => (
          <section key={group.key} aria-label={group.name} className="tw:grid tw:gap-2">
            <h2 className="tw:m-0 tw:text-body tw:font-semibold">{group.name}</h2>
            <NotebookGrid notebooks={group.notebooks} subjects={subjects} showToast={showToast} onOpen={setOpen} onChanged={refresh} />
          </section>
        ))
      ) : (
        <NotebookGrid notebooks={shown} subjects={subjects} showToast={showToast} onOpen={setOpen} onChanged={refresh} />
      )}
      {creating ? (
        <NewNotebookDialog
          subjects={subjects}
          defaultSubjectId={defaultSubjectId}
          onClose={() => setCreating(false)}
          onCreated={(notebookId, title) => {
            setCreating(false);
            refresh();
            setOpen({ notebookId, title });
          }}
        />
      ) : null}
    </div>
  );
}

function groupBySubject(notebooks: readonly NotebookSummary[], subjects: LibrarySubjects) {
  const groups = subjects.subjects.map((tile) => ({
    key: tile.key,
    name: tile.name,
    notebooks: notebooks.filter((item) => item.notebook.subjectId === tile.subjectId),
  }));
  const known = new Set(subjects.subjects.map((tile) => tile.subjectId));
  // No subject last, with any notebook whose subject the list has not caught up with.
  groups.push({
    key: "no-subject",
    name: "No subject",
    notebooks: notebooks.filter((item) => item.notebook.subjectId === null || !known.has(item.notebook.subjectId)),
  });
  return groups.filter((group) => group.notebooks.length > 0);
}

function NotebookGrid({ notebooks, subjects, showToast, onOpen, onChanged }: {
  notebooks: readonly NotebookSummary[];
  subjects: LibrarySubjects;
  showToast: ShowToast;
  onOpen: (open: { notebookId: string; title: string }) => void;
  onChanged: () => void;
}) {
  const confirm = useConfirm();
  const [renaming, setRenaming] = useState<NotebookSummary | null>(null);
  const [moving, setMoving] = useState<NotebookSummary | null>(null);

  async function remove(item: NotebookSummary) {
    if (!(await confirm({
      title: `Delete ${item.notebook.title}?`,
      description: "The notebook and all of its pages are deleted. This cannot be undone.",
      confirmLabel: "Delete notebook",
      cancelLabel: "Keep notebook",
      destructive: true,
    }))) return;
    try {
      await deleteNotebook(item.notebook.id);
      showToast("success", `Deleted ${item.notebook.title}.`);
    } catch (error) {
      showToast("error", getErrorMessage(error));
    }
    onChanged();
  }

  return (
    <>
      <ul className="tw:m-0 tw:grid tw:list-none tw:grid-cols-[repeat(auto-fill,minmax(11rem,1fr))] tw:gap-4 tw:p-0">
        {notebooks.map((item) => {
          const { notebook } = item;
          const page = item.firstPage;
          return (
            <li key={notebook.id} className="tw:relative tw:grid tw:min-w-0 tw:content-start tw:gap-2">
              <ItemTrigger
                aria-label={`Open ${notebook.title}`}
                onClick={() => onOpen({ notebookId: notebook.id, title: notebook.title })}
                className="tw:block tw:w-full tw:cursor-pointer tw:overflow-hidden tw:rounded-md tw:border-0 tw:p-0 tw:shadow-sm tw:ring-1 tw:ring-border tw:outline-none tw:focus-visible:ring-2 tw:focus-visible:ring-ring tw:hover:ring-primary"
                style={{ aspectRatio: page ? `${page.width} / ${page.height}` : "612 / 792" }}
              >
                <NotebookThumbnail page={page} items={item.preview} />
              </ItemTrigger>
              <div className="tw:flex tw:min-w-0 tw:items-start tw:justify-between tw:gap-1">
                <div className="tw:min-w-0">
                  <p className="tw:m-0 tw:truncate tw:text-small tw:font-semibold">{notebook.title}</p>
                  <p className="tw:m-0 tw:text-caption tw:text-muted-foreground">
                    {[
                      `${item.pageCount} ${item.pageCount === 1 ? "page" : "pages"}`,
                      item.linkCount > 0 ? `${item.linkCount} ${item.linkCount === 1 ? "link" : "links"}` : null,
                    ].filter(Boolean).join(" · ")}
                  </p>
                </div>
                <OverflowMenu
                  label={`${notebook.title} actions`}
                  triggerVariant="ghost"
                  items={[
                    { id: "rename", label: "Rename", onSelect: () => setRenaming(item) },
                    { id: "move", label: "Move to subject", onSelect: () => setMoving(item) },
                    { id: "delete", label: "Delete notebook", destructive: true, onSelect: () => void remove(item) },
                  ]}
                />
              </div>
            </li>
          );
        })}
      </ul>
      {renaming ? (
        <RenameNotebookDialog
          item={renaming}
          onClose={() => setRenaming(null)}
          onSaved={() => {
            setRenaming(null);
            onChanged();
          }}
        />
      ) : null}
      {moving ? (
        <MoveNotebookDialog
          item={moving}
          subjects={subjects}
          onClose={() => setMoving(null)}
          onSaved={() => {
            setMoving(null);
            onChanged();
          }}
        />
      ) : null}
    </>
  );
}

function SubjectSelect({ subjects, value, onChange }: {
  subjects: LibrarySubjects;
  value: string | null;
  onChange: (subjectId: string | null) => void;
}) {
  return (
    <NativeSelect value={value ?? ""} onChange={(event) => onChange(event.target.value || null)}>
      {subjects.subjects.map((tile) => tile.subjectId === null ? null : (
        <option key={tile.subjectId} value={tile.subjectId}>{tile.name}</option>
      ))}
      <option value="">No subject</option>
    </NativeSelect>
  );
}

function NewNotebookDialog({ subjects, defaultSubjectId, onClose, onCreated }: {
  subjects: LibrarySubjects;
  defaultSubjectId: string | null;
  onClose: () => void;
  onCreated: (notebookId: string, title: string) => void;
}) {
  const [title, setTitle] = useState("");
  const [subjectId, setSubjectId] = useState(defaultSubjectId);
  const [paper, setPaper] = useState<Paper>("lined");
  const [pageSize, setPageSize] = useState<PageSize>("letter");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const trimmed = title.trim();

  async function create() {
    if (!trimmed || saving) return;
    setSaving(true);
    setError(null);
    try {
      const created = await createNotebook({ title: trimmed, subjectId, paper, pageSize });
      onCreated(created.notebook.id, created.notebook.title);
    } catch (failure) {
      setError(getErrorMessage(failure));
      setSaving(false);
    }
  }

  return (
    <Dialog
      title="New notebook"
      width="medium"
      busy={saving}
      onClose={onClose}
      onSubmit={(event) => {
        event.preventDefault();
        void create();
      }}
      actions={<>
        <Button variant="secondary" onClick={onClose}>Cancel</Button>
        <Button variant="primary" type="submit" disabled={!trimmed || saving}>Create notebook</Button>
      </>}
    >
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      <FormField label="Name" required>
        <TextField value={title} maxLength={MAX_TITLE_CHARS} autoFocus
          onChange={(event) => setTitle(event.target.value)} />
      </FormField>
      <FormField label="Subject">
        <SubjectSelect subjects={subjects} value={subjectId} onChange={setSubjectId} />
      </FormField>
      <LabeledChoice label="Paper">
        <SegmentedControl label="Paper" value={paper} options={PAPER_OPTIONS} onChange={setPaper} />
      </LabeledChoice>
      <LabeledChoice label="Page size">
        <SegmentedControl label="Page size" value={pageSize} options={PAGE_SIZE_OPTIONS} onChange={setPageSize} />
      </LabeledChoice>
      <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
        Wide pages lie on their side, with room for mind maps and diagrams.
      </p>
    </Dialog>
  );
}

function RenameNotebookDialog({ item, onClose, onSaved }: { item: NotebookSummary; onClose: () => void; onSaved: () => void }) {
  const [title, setTitle] = useState(item.notebook.title);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const trimmed = title.trim();
  async function save() {
    if (!trimmed || saving) return;
    setSaving(true);
    try {
      await renameNotebook(item.notebook.id, trimmed);
      onSaved();
    } catch (failure) {
      setError(getErrorMessage(failure));
      setSaving(false);
    }
  }
  return (
    <Dialog
      title="Rename notebook"
      width="small"
      busy={saving}
      onClose={onClose}
      onSubmit={(event) => {
        event.preventDefault();
        void save();
      }}
      actions={<>
        <Button variant="secondary" onClick={onClose}>Cancel</Button>
        <Button variant="primary" type="submit" disabled={!trimmed || saving}>Rename</Button>
      </>}
    >
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      <FormField label="Name">
        <TextField value={title} maxLength={MAX_TITLE_CHARS} autoFocus onChange={(event) => setTitle(event.target.value)} />
      </FormField>
    </Dialog>
  );
}

function MoveNotebookDialog({ item, subjects, onClose, onSaved }: {
  item: NotebookSummary;
  subjects: LibrarySubjects;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [subjectId, setSubjectId] = useState(item.notebook.subjectId);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function save() {
    if (saving) return;
    setSaving(true);
    try {
      await moveNotebookToSubject(item.notebook.id, subjectId);
      onSaved();
    } catch (failure) {
      setError(getErrorMessage(failure));
      setSaving(false);
    }
  }
  return (
    <Dialog
      title={`Move ${item.notebook.title}`}
      width="small"
      busy={saving}
      onClose={onClose}
      onSubmit={(event) => {
        event.preventDefault();
        void save();
      }}
      actions={<>
        <Button variant="secondary" onClick={onClose}>Cancel</Button>
        <Button variant="primary" type="submit" disabled={saving}>Move</Button>
      </>}
    >
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      <FormField label="Subject">
        <SubjectSelect subjects={subjects} value={subjectId} onChange={setSubjectId} />
      </FormField>
    </Dialog>
  );
}
