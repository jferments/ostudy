import { useCallback, useEffect, useRef, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { getErrorMessage } from "../../ipc/common";
import {
  createNotebook,
  createNoteFolder,
  deleteNotebook,
  deleteNoteFolder,
  listNotebooks,
  listNoteFolders,
  moveNotebookToSubject,
  pinNotebook,
  putNotebookInFolder,
  renameNotebook,
  renameNoteFolder,
  type NoteFolder,
  type NotebookSummary,
  type PageSize,
  type Paper,
} from "../../ipc/notebooks";
import type { LibrarySubjectScope } from "../../navigation/appDestinations";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
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
import { NotesWelcome } from "./NotesWelcome";
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
/** The longest folder name, and how deep folders go; the native side checks both too. */
const MAX_FOLDER_NAME_CHARS = 120;
const MAX_FOLDER_DEPTH = 4;

/** What Notes shows: every notebook, the latest ones, the pinned ones, or one folder. */
type NotesView = { kind: "all" } | { kind: "recent" } | { kind: "pinned" } | { kind: "folder"; folderId: string };
type NotesSort = "changed" | "name";

/** How many notebooks Recent shows. */
const RECENT_COUNT = 12;

/**
 * The Notes destination: the learner's folders beside their notebooks, each
 * notebook shown by its first page with the links it holds. All notes,
 * Recent and Pinned sit above the folders, which may hold folders. Opening
 * a notebook makes it the page.
 */
export function LibraryNotes({ subject, showToast, openRequest = null, onOpenRequestConsumed, ...links }: LibraryNotesProps) {
  const subjects = useLibrarySubjects(subject);
  const [notebooks, setNotebooks] = useState<NotebookSummary[] | null>(null);
  const [folders, setFolders] = useState<NoteFolder[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState<{ notebookId: string; title: string; pageId?: string } | null>(null);
  const [creating, setCreating] = useState(false);
  const [view, setView] = useState<NotesView>({ kind: "all" });
  const [sort, setSort] = useState<NotesSort>("changed");
  const [naming, setNaming] = useState<{ folder: NoteFolder | null; parentId: string | null } | null>(null);
  const confirm = useConfirm();
  const request = useRef(0);

  const refresh = useCallback(() => {
    const current = ++request.current;
    Promise.all([listNotebooks(), listNoteFolders()]).then(([next, nextFolders]) => {
      if (request.current !== current) return;
      setNotebooks(next);
      setFolders(nextFolders);
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

  const inScope = notebooks?.filter((item) => subject.kind === "all"
    || (subject.kind === "none" ? item.notebook.subjectId === null : item.notebook.subjectId === subject.subjectId)) ?? [];
  const defaultSubjectId = subject.kind === "subject" ? subject.subjectId : null;
  // A folder deleted elsewhere shows everything instead.
  const folder = view.kind === "folder" ? folders.find((item) => item.id === view.folderId) ?? null : null;
  const shownView: NotesView = view.kind === "folder" && !folder ? { kind: "all" } : view;
  const counts = new Map<string | null, number>();
  for (const item of inScope) counts.set(item.folderId, (counts.get(item.folderId) ?? 0) + 1);
  const pinned = inScope.filter((item) => item.pinnedAt !== null);
  const sorted = [...inScope].sort(sort === "name"
    ? (left, right) => left.notebook.title.localeCompare(right.notebook.title)
    : (left, right) => right.notebook.updatedAt - left.notebook.updatedAt);
  const shown = shownView.kind === "folder"
    ? sorted.filter((item) => item.folderId === shownView.folderId)
    : shownView.kind === "pinned"
      ? sorted.filter((item) => item.pinnedAt !== null)
      : shownView.kind === "recent"
        ? [...inScope].sort((left, right) => right.notebook.updatedAt - left.notebook.updatedAt).slice(0, RECENT_COUNT)
        : sorted;
  const childFolders = folder ? folders.filter((item) => item.parentId === folder.id) : [];
  const viewTitle = folder ? folder.name
    : shownView.kind === "recent" ? "Recent" : shownView.kind === "pinned" ? "Pinned" : "All notes";
  const trail = folder ? folderTrail(folders, folder) : [];

  async function removeFolder(target: NoteFolder) {
    if (!(await confirm({
      title: `Delete ${target.name}?`,
      description: "The folder and the folders inside it are deleted. Their notebooks are kept and go back to All notes.",
      confirmLabel: "Delete folder",
      cancelLabel: "Keep folder",
      destructive: true,
    }))) return;
    try {
      await deleteNoteFolder(target.id);
      if (view.kind === "folder" && (view.folderId === target.id || folderTrail(folders, folders.find((item) => item.id === view.folderId) ?? target).some((item) => item.id === target.id))) {
        setView(target.parentId ? { kind: "folder", folderId: target.parentId } : { kind: "all" });
      }
    } catch (failure) {
      showToast("error", getErrorMessage(failure));
    }
    refresh();
  }

  const destinations: { view: NotesView; label: string; icon: IconName; count?: number }[] = [
    { view: { kind: "all" }, label: "All notes", icon: "sticky-note", count: inScope.length },
    { view: { kind: "recent" }, label: "Recent", icon: "schedule" },
    { view: { kind: "pinned" }, label: "Pinned", icon: "pin", count: pinned.length },
  ];
  const isCurrent = (candidate: NotesView) => candidate.kind === shownView.kind
    && (candidate.kind !== "folder" || (shownView.kind === "folder" && candidate.folderId === shownView.folderId));
  const viewKey = (candidate: NotesView) => candidate.kind === "folder" ? `folder:${candidate.folderId}` : candidate.kind;

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
      <div className="tw:grid tw:min-w-0 tw:grid-cols-[14rem_minmax(0,1fr)] tw:items-start tw:gap-6 tw:compact:grid-cols-1 tw:compact:gap-3">
        <nav aria-label="Note folders" className="tw:grid tw:gap-0.5 tw:compact:hidden">
          {destinations.map((item) => (
            <NotesNavItem key={item.label} label={item.label} icon={item.icon} count={item.count}
              current={isCurrent(item.view)} onSelect={() => setView(item.view)} />
          ))}
          <p className="tw:m-0 tw:px-3 tw:pt-4 tw:pb-1 tw:text-caption tw:font-semibold tw:text-muted-foreground">Folders</p>
          {folderRows(folders).map(({ folder: item, depth }) => (
            <NotesNavItem key={item.id} label={item.name} icon="folder" count={counts.get(item.id)} depth={depth}
              current={isCurrent({ kind: "folder", folderId: item.id })}
              onSelect={() => setView({ kind: "folder", folderId: item.id })} />
          ))}
          <NotesNavItem label="New folder" icon="plus" onSelect={() => setNaming({ folder: null, parentId: null })} />
        </nav>
        <div className="tw:hidden tw:compact:flex tw:compact:flex-wrap tw:compact:items-center tw:compact:gap-2">
          <NativeSelect aria-label="Show" className="tw:min-w-0 tw:flex-1" value={viewKey(shownView)}
            onChange={(event) => {
              const value = event.currentTarget.value;
              setView(value.startsWith("folder:") ? { kind: "folder", folderId: value.slice(7) }
                : { kind: value as "all" | "recent" | "pinned" });
            }}>
            {destinations.map((item) => <option key={item.label} value={viewKey(item.view)}>{item.label}</option>)}
            {folderRows(folders).map(({ folder: item, depth }) => (
              <option key={item.id} value={`folder:${item.id}`}>{`${"  ".repeat(depth)}${item.name}`}</option>
            ))}
          </NativeSelect>
          <Button size="small" onClick={() => setNaming({ folder: null, parentId: null })}>
            <Icon name="plus" size={15} /> New folder
          </Button>
        </div>
        <section aria-labelledby="notes-view-title" className="tw:grid tw:min-w-0 tw:content-start tw:gap-4">
          <div className="tw:flex tw:flex-wrap tw:items-end tw:gap-3">
            <div className="tw:min-w-0 tw:flex-1">
              {trail.length > 1 ? (
                <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
                  {trail.slice(0, -1).map((item) => item.name).join(" / ")}
                </p>
              ) : null}
              <h2 id="notes-view-title" className="tw:m-0 tw:truncate tw:text-section tw:font-semibold">{viewTitle}</h2>
            </div>
            {shownView.kind !== "recent" && shown.length > 0 ? (
              <NativeSelect aria-label="Sort notes" value={sort} onChange={(event) => setSort(event.currentTarget.value as NotesSort)}>
                <option value="changed">Recently changed</option>
                <option value="name">Name</option>
              </NativeSelect>
            ) : null}
            {folder ? (
              <OverflowMenu
                label={`${folder.name} actions`}
                triggerVariant="ghost"
                items={[
                  ...(trail.length < MAX_FOLDER_DEPTH
                    ? [{ id: "new-folder", label: "New folder inside", onSelect: () => setNaming({ folder: null, parentId: folder.id }) }]
                    : []),
                  { id: "rename", label: "Rename folder", onSelect: () => setNaming({ folder, parentId: folder.parentId }) },
                  { id: "delete", label: "Delete folder", destructive: true, onSelect: () => void removeFolder(folder) },
                ]}
              />
            ) : null}
          </div>
          {childFolders.length > 0 ? (
            <ul aria-label="Folders" className="tw:m-0 tw:grid tw:list-none tw:grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] tw:gap-3 tw:p-0">
              {childFolders.map((item) => (
                <li key={item.id}>
                  <ItemTrigger
                    onClick={() => setView({ kind: "folder", folderId: item.id })}
                    className="tw:flex tw:w-full tw:cursor-pointer tw:items-center tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-3.5 tw:text-start tw:font-sans tw:text-foreground tw:outline-none tw:hover:bg-foreground/4 tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50"
                  >
                    <Icon name="folder" size={28} className="tw:shrink-0 tw:text-primary" />
                    <span className="tw:grid tw:min-w-0">
                      <span className="tw:truncate tw:text-label tw:font-semibold">{item.name}</span>
                      <span className="tw:text-small tw:text-muted-foreground">{countLabel(counts.get(item.id) ?? 0, "notebook")}</span>
                    </span>
                  </ItemTrigger>
                </li>
              ))}
            </ul>
          ) : null}
          {notebooks === null && !error ? (
            <LoadingState label="Loading notes…" />
          ) : shown.length === 0 && notebooks !== null ? (
            shownView.kind === "all" ? (
              <NotesWelcome onCreate={() => setCreating(true)} onCreateFolder={() => setNaming({ folder: null, parentId: null })} />
            ) : childFolders.length === 0 ? (
              <EmptyState
                compact
                icon={shownView.kind === "pinned" ? "pin" : "folder"}
                title={shownView.kind === "pinned" ? "Nothing pinned" : "Nothing here yet"}
                description={shownView.kind === "pinned"
                  ? "Pin a notebook from its menu to keep it here."
                  : "Make a notebook here with New notebook, or move one in from its menu."}
              />
            ) : null
          ) : (
            <NotebookGrid notebooks={shown} subjects={subjects} folders={folders} showToast={showToast} onOpen={setOpen} onChanged={refresh} />
          )}
        </section>
      </div>
      {creating ? (
        <NewNotebookDialog
          subjects={subjects}
          defaultSubjectId={defaultSubjectId}
          onClose={() => setCreating(false)}
          onCreated={(notebookId, title) => {
            setCreating(false);
            const done = () => {
              refresh();
              setOpen({ notebookId, title });
            };
            // A notebook made inside a folder starts there.
            if (folder) {
              putNotebookInFolder(notebookId, folder.id).then(done, (failure: unknown) => {
                showToast("error", getErrorMessage(failure));
                done();
              });
            } else {
              done();
            }
          }}
        />
      ) : null}
      {naming ? (
        <FolderNameDialog
          folder={naming.folder}
          onClose={() => setNaming(null)}
          onSave={async (name) => {
            if (naming.folder) {
              await renameNoteFolder(naming.folder.id, name);
            } else {
              const created = await createNoteFolder(name, naming.parentId);
              setView({ kind: "folder", folderId: created.id });
            }
            setNaming(null);
            refresh();
          }}
        />
      ) : null}
    </div>
  );
}

function NotesNavItem({ label, icon, count, depth = 0, current = false, onSelect }: {
  label: string;
  icon: IconName;
  count?: number;
  depth?: number;
  current?: boolean;
  onSelect: () => void;
}) {
  return (
    <ItemTrigger
      aria-current={current ? "page" : undefined}
      onClick={onSelect}
      style={{ paddingInlineStart: `${0.75 + depth * 1.25}rem` }}
      className={cn(
        "tw:flex tw:min-h-10 tw:w-full tw:cursor-pointer tw:items-center tw:gap-2.5 tw:rounded-md tw:border-0 tw:bg-transparent tw:pe-3 tw:text-start tw:font-sans tw:text-label tw:text-muted-foreground tw:outline-none tw:hover:bg-foreground/4 tw:hover:text-foreground tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50",
        current && "tw:bg-accent tw:text-foreground",
      )}
    >
      <Icon name={icon} size={16} className="tw:shrink-0" />
      <span className="tw:min-w-0 tw:flex-1 tw:truncate">{label}</span>
      {count !== undefined && count > 0 ? <span className="tw:text-small tw:text-muted-foreground">{count}</span> : null}
    </ItemTrigger>
  );
}

/** Folders in reading order, each with how deep it is. */
function folderRows(folders: readonly NoteFolder[]): { folder: NoteFolder; depth: number }[] {
  const rows: { folder: NoteFolder; depth: number }[] = [];
  const visit = (parentId: string | null, depth: number) => {
    for (const folder of folders.filter((item) => item.parentId === parentId)) {
      rows.push({ folder, depth });
      if (depth < MAX_FOLDER_DEPTH) visit(folder.id, depth + 1);
    }
  };
  visit(null, 0);
  return rows;
}

/** The folders from the top down to `folder`, itself last. */
function folderTrail(folders: readonly NoteFolder[], folder: NoteFolder): NoteFolder[] {
  const trail = [folder];
  let parentId = folder.parentId;
  while (parentId && trail.length <= MAX_FOLDER_DEPTH) {
    const parent = folders.find((item) => item.id === parentId);
    if (!parent) break;
    trail.unshift(parent);
    parentId = parent.parentId;
  }
  return trail;
}

function countLabel(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}

/** Today, Yesterday, or the day it was changed. */
function changedLabel(at: number, now = Date.now()): string {
  const day = (time: number) => new Date(time).toDateString();
  if (day(at) === day(now)) return "Today";
  if (day(at) === day(now - 86_400_000)) return "Yesterday";
  return new Date(at).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function NotebookGrid({ notebooks, subjects, folders, showToast, onOpen, onChanged }: {
  notebooks: readonly NotebookSummary[];
  subjects: LibrarySubjects;
  folders: readonly NoteFolder[];
  showToast: ShowToast;
  onOpen: (open: { notebookId: string; title: string }) => void;
  onChanged: () => void;
}) {
  const confirm = useConfirm();
  const [renaming, setRenaming] = useState<NotebookSummary | null>(null);
  const [moving, setMoving] = useState<NotebookSummary | null>(null);
  const [filing, setFiling] = useState<NotebookSummary | null>(null);

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

  async function togglePin(item: NotebookSummary) {
    try {
      await pinNotebook(item.notebook.id, item.pinnedAt === null);
    } catch (error) {
      showToast("error", getErrorMessage(error));
    }
    onChanged();
  }

  return (
    <>
      <ul aria-label="Notebooks" className="tw:m-0 tw:grid tw:list-none tw:grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] tw:gap-4 tw:p-0">
        {notebooks.map((item) => {
          const { notebook } = item;
          const page = item.firstPage;
          return (
            <li key={notebook.id} className="tw:grid tw:min-w-0 tw:content-start tw:overflow-hidden tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card">
              <ItemTrigger
                aria-label={`Open ${notebook.title}`}
                onClick={() => onOpen({ notebookId: notebook.id, title: notebook.title })}
                className="tw:block tw:aspect-[4/3] tw:w-full tw:cursor-pointer tw:overflow-hidden tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-[var(--ostudy-color-neutral-0)] tw:p-0 tw:outline-none tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:focus-visible:ring-inset"
              >
                <NotebookThumbnail page={page} items={item.preview} />
              </ItemTrigger>
              <div className="tw:grid tw:gap-1.5 tw:px-3 tw:pt-2 tw:pb-3">
                <div className="tw:flex tw:min-w-0 tw:items-start tw:justify-between tw:gap-1">
                  <div className="tw:min-w-0">
                    <p className="tw:m-0 tw:flex tw:items-center tw:gap-1.5 tw:text-small tw:font-semibold">
                      {item.pinnedAt !== null ? <Icon name="pin" size={14} label="Pinned" /> : null}
                      <span className="tw:truncate">{notebook.title}</span>
                    </p>
                    <p className="tw:m-0 tw:text-caption tw:text-muted-foreground">
                      {[
                        changedLabel(notebook.updatedAt),
                        `${item.pageCount} ${item.pageCount === 1 ? "page" : "pages"}`,
                        item.linkCount > 0 ? `${item.linkCount} ${item.linkCount === 1 ? "link" : "links"}` : null,
                      ].filter(Boolean).join(" · ")}
                    </p>
                  </div>
                  <OverflowMenu
                    label={`${notebook.title} actions`}
                    triggerVariant="ghost"
                    items={[
                      { id: "pin", label: item.pinnedAt === null ? "Pin" : "Unpin", onSelect: () => void togglePin(item) },
                      { id: "rename", label: "Rename", onSelect: () => setRenaming(item) },
                      { id: "folder", label: "Move to folder", onSelect: () => setFiling(item) },
                      { id: "move", label: "Move to subject", onSelect: () => setMoving(item) },
                      { id: "delete", label: "Delete notebook", destructive: true, onSelect: () => void remove(item) },
                    ]}
                  />
                </div>
                {item.linkLabels.length > 0 ? (
                  <ul aria-label="Links" className="tw:m-0 tw:flex tw:list-none tw:flex-wrap tw:gap-1 tw:p-0">
                    {item.linkLabels.map((label) => (
                      <li key={label} className="tw:inline-flex tw:max-w-full tw:items-center tw:gap-1 tw:rounded-sm tw:bg-accent tw:px-1.5 tw:py-0.5 tw:text-caption tw:text-accent-foreground">
                        <Icon name="link" size={12} className="tw:shrink-0" />
                        <span className="tw:truncate">{label}</span>
                      </li>
                    ))}
                  </ul>
                ) : null}
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
      {filing ? (
        <MoveToFolderDialog
          item={filing}
          folders={folders}
          onClose={() => setFiling(null)}
          onSaved={() => {
            setFiling(null);
            onChanged();
          }}
        />
      ) : null}
    </>
  );
}

function FolderNameDialog({ folder, onClose, onSave }: {
  folder: NoteFolder | null;
  onClose: () => void;
  onSave: (name: string) => Promise<void>;
}) {
  const [name, setName] = useState(folder?.name ?? "");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const trimmed = name.trim();
  async function save() {
    if (!trimmed || saving) return;
    setSaving(true);
    try {
      await onSave(trimmed);
    } catch (failure) {
      setError(getErrorMessage(failure));
      setSaving(false);
    }
  }
  return (
    <Dialog
      title={folder ? "Rename folder" : "New folder"}
      width="small"
      busy={saving}
      onClose={onClose}
      onSubmit={(event) => {
        event.preventDefault();
        void save();
      }}
      actions={<>
        <Button variant="secondary" onClick={onClose}>Cancel</Button>
        <Button variant="primary" type="submit" disabled={!trimmed || saving}>{folder ? "Rename" : "Create folder"}</Button>
      </>}
    >
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      <FormField label="Name">
        <TextField value={name} maxLength={MAX_FOLDER_NAME_CHARS} autoFocus onChange={(event) => setName(event.target.value)} />
      </FormField>
    </Dialog>
  );
}

function MoveToFolderDialog({ item, folders, onClose, onSaved }: {
  item: NotebookSummary;
  folders: readonly NoteFolder[];
  onClose: () => void;
  onSaved: () => void;
}) {
  const [folderId, setFolderId] = useState(item.folderId);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function save() {
    if (saving) return;
    setSaving(true);
    try {
      await putNotebookInFolder(item.notebook.id, folderId);
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
      <FormField label="Folder">
        <NativeSelect value={folderId ?? ""} onChange={(event) => setFolderId(event.target.value || null)}>
          <option value="">No folder</option>
          {folderRows(folders).map(({ folder, depth }) => (
            <option key={folder.id} value={folder.id}>{`${"  ".repeat(depth)}${folder.name}`}</option>
          ))}
        </NativeSelect>
      </FormField>
    </Dialog>
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
