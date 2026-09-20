import { useEffect, useMemo, useState } from "react";

import type {
  DocumentCollection,
  DocumentCollectionMembership,
  DocumentCollectionOrganization,
  SourceRangeInput,
} from "../../ipc/collections";
import type { DocumentSummary } from "../../ipc/documents";
import type { StudyTargetArtifactStatus, StudyTargetBundleStatus } from "../../ipc/studyMaterials";
import { Dialog, DialogActions } from "../../ui/overlays/Dialog";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { Checkbox } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { Surface } from "../../ui/primitives/Surface";
import { Tabs } from "../../ui/primitives/Tabs";
import { TextField } from "../../ui/primitives/TextField";
import { useDocumentCollectionsController } from "./useDocumentCollectionsController";
import { useStudyTargetBundleController } from "./useStudyTargetBundleController";

type CollectionDialog =
  | { kind: "create"; parentId: string | null; name: string }
  | { kind: "rename"; folder: DocumentCollection; name: string }
  | { kind: "move-folder"; folder: DocumentCollection; targetParentId: string }
  | { kind: "add-source"; documentId: string }
  | { kind: "move-source"; membership: DocumentCollectionMembership; targetCollectionId: string }
  | { kind: "ranges"; membership: DocumentCollectionMembership; value: string };

export function DocumentCollectionsWorkspace({
  documents,
  refreshDecks,
  onOpenDeck,
  onOpenGuide,
}: {
  documents: DocumentSummary[];
  refreshDecks?: () => Promise<unknown>;
  onOpenDeck?: (deckId: string) => void;
  onOpenGuide?: (collectionId: string, title: string) => void;
}) {
  const controller = useDocumentCollectionsController();
  const confirm = useConfirm();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [tab, setTab] = useState<"overview" | "sources">("overview");
  const [showArchived, setShowArchived] = useState(false);
  const [dialog, setDialog] = useState<CollectionDialog | null>(null);
  const [dialogError, setDialogError] = useState<string | null>(null);
  const organization = controller.organization;
  const bundle = useStudyTargetBundleController({
    target: selectedId ? { kind: "collection", id: selectedId } : null,
    changed: refreshDecks,
    refreshKey: organization?.revision ?? null,
  });

  useEffect(() => {
    if (selectedId && organization && !organization.collections.some((folder) => folder.id === selectedId)) {
      setSelectedId(null);
    }
  }, [organization, selectedId]);

  const documentById = useMemo(
    () => new Map(documents.map((document) => [document.id, document])),
    [documents],
  );

  if (!organization) {
    return (
      <section aria-label="Document folders" className="tw:mb-6 tw:grid tw:gap-3">
        <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-2">
          <h2 className="tw:m-0 tw:text-lg tw:font-semibold">Document folders</h2>
          <Button size="small" disabled>New folder</Button>
        </div>
        {controller.error ? (
          <InlineNotice kind="error">
            {controller.error} <Button size="small" onClick={() => void controller.load()}>Retry</Button>
          </InlineNotice>
        ) : <p role="status" className="tw:m-0 tw:text-sm tw:text-muted-foreground">Loading folders…</p>}
      </section>
    );
  }

  const current = selectedId
    ? organization.collections.find((folder) => folder.id === selectedId) ?? null
    : null;
  const roots = visibleFolders(
    organization.collections.filter((folder) => folder.parentId === null),
    showArchived,
  );

  async function deleteCurrent(folder: DocumentCollection): Promise<void> {
    const approved = await confirm({
      title: "Delete this document folder?",
      description: "The folder and its memberships will be removed. Retained source documents and independently created study material are not deleted.",
      confirmLabel: "Delete folder",
      destructive: true,
    });
    if (!approved) return;
    if (await controller.deleteFolder(folder)) setSelectedId(null);
  }

  async function removeMembership(membership: DocumentCollectionMembership): Promise<void> {
    const document = documentById.get(membership.documentId);
    const approved = await confirm({
      title: "Remove this source from the folder?",
      description: `${document?.title ?? "This document"} remains in Documents and in any other folders that reference it.`,
      confirmLabel: "Remove source",
      destructive: true,
    });
    if (approved) await controller.removeDocument(membership);
  }

  function openRanges(membership: DocumentCollectionMembership): void {
    if (!organization) return;
    const ranges = organization.ranges.filter((range) => range.membershipId === membership.id);
    setDialog({
      kind: "ranges",
      membership,
      value: ranges.map((range) => range.startUnit === range.endUnit
        ? String(range.startUnit)
        : `${range.startUnit}-${range.endUnit}`).join(", "),
    });
    setDialogError(null);
  }

  return (
    <section aria-label="Document folders" className="tw:mb-6 tw:grid tw:gap-4">
      {controller.error ? <InlineNotice kind="error">{controller.error}</InlineNotice> : null}
      {!current ? (
        <>
          <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
            <div>
              <h2 className="tw:m-0 tw:text-lg tw:font-semibold">Document folders</h2>
              <p className="tw:mt-1 tw:mb-0 tw:text-sm tw:text-muted-foreground">
                Group retained sources without copying or reprocessing them.
              </p>
            </div>
            <Button
              size="small"
              disabled={controller.busy}
              onClick={() => { setDialog({ kind: "create", parentId: null, name: "" }); setDialogError(null); }}
            >
              + New folder
            </Button>
          </div>
          <label className="tw:flex tw:min-h-11 tw:w-fit tw:items-center tw:gap-2 tw:text-sm">
            <Checkbox checked={showArchived} onCheckedChange={setShowArchived} />
            Show archived folders
          </label>
          {roots.length ? (
            <div className="tw:grid tw:gap-2 tw:sm:grid-cols-2 tw:xl:grid-cols-3">
              {roots.map((folder) => (
                <FolderTile
                  key={folder.id}
                  folder={folder}
                  organization={organization}
                  onOpen={() => { setSelectedId(folder.id); setTab("overview"); }}
                />
              ))}
            </div>
          ) : (
            <p className="tw:m-0 tw:text-sm tw:text-muted-foreground">
              {showArchived ? "No folders yet." : "No active folders yet. Create one for a week, unit, or reading target."}
            </p>
          )}
        </>
      ) : (
        <>
          <Breadcrumbs
            folder={current}
            organization={organization}
            onOpen={(id) => { setSelectedId(id); setTab("overview"); }}
            onRoot={() => setSelectedId(null)}
          />
          <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-start tw:justify-between tw:gap-3">
            <div className="tw:min-w-0">
              <h2 className="tw:m-0 tw:wrap-anywhere tw:text-lg tw:font-semibold">{current.name}</h2>
              <p className="tw:mt-1 tw:mb-0 tw:text-sm tw:text-muted-foreground">
                {current.archivedAt ? "Archived folder. Explicit source selection remains available." : "Folder organization is local and makes no AI request."}
              </p>
            </div>
            <div className="tw:flex tw:flex-wrap tw:gap-2">
              <Button size="small" disabled={controller.busy}
                onClick={() => { setDialog({ kind: "create", parentId: current.id, name: "" }); setDialogError(null); }}>
                + Subfolder
              </Button>
              <Button size="small" disabled={controller.busy}
                onClick={() => { setDialog({ kind: "rename", folder: current, name: current.name }); setDialogError(null); }}>
                Rename
              </Button>
              <Button size="small" disabled={controller.busy}
                onClick={() => { setDialog({ kind: "move-folder", folder: current, targetParentId: current.parentId ?? "" }); setDialogError(null); }}>
                Move folder
              </Button>
              <Button size="small" disabled={controller.busy}
                onClick={() => void controller.updateFolder(current, { archived: current.archivedAt === null })}>
                {current.archivedAt ? "Restore" : "Archive"}
              </Button>
              <Button size="small" variant="dangerGhost" disabled={controller.busy}
                onClick={() => void deleteCurrent(current)}>
                Delete
              </Button>
            </div>
          </div>
          <CollectionStudyMaterials
            status={bundle.status}
            loading={bundle.loading}
            busy={bundle.busy}
            error={bundle.error}
            onRun={() => void bundle.run()}
            onOpenDeck={onOpenDeck}
            onOpenGuide={onOpenGuide}
          />
          <Tabs
            ariaLabel="Document folder views"
            value={tab}
            onValueChange={setTab}
            tabs={[
              {
                value: "overview",
                label: "Overview",
                panel: (
                  <FolderOverview
                    folder={current}
                    organization={organization}
                    documents={documentById}
                    showArchived={showArchived}
                    onOpenFolder={(id) => { setSelectedId(id); setTab("overview"); }}
                    onAddSource={() => { setDialog({ kind: "add-source", documentId: "" }); setDialogError(null); }}
                    disabled={controller.busy}
                  />
                ),
              },
              {
                value: "sources",
                label: "Sources",
                panel: (
                  <FolderSources
                    folder={current}
                    organization={organization}
                    documents={documentById}
                    onAdd={() => { setDialog({ kind: "add-source", documentId: "" }); setDialogError(null); }}
                    onRanges={openRanges}
                    onMove={(membership) => {
                      setDialog({ kind: "move-source", membership, targetCollectionId: "" });
                      setDialogError(null);
                    }}
                    onRemove={(membership) => void removeMembership(membership)}
                    disabled={controller.busy}
                  />
                ),
              },
            ]}
          />
        </>
      )}
      {dialog ? (
        <CollectionEditorDialog
          dialog={dialog}
          setDialog={setDialog}
          organization={organization}
          documents={documents}
          documentById={documentById}
          currentFolderId={current?.id ?? null}
          error={dialogError}
          setError={setDialogError}
          busy={controller.busy}
          onCreate={(name, parentId) => controller.createFolder(name, parentId)}
          onUpdate={(folder, patch) => controller.updateFolder(folder, patch)}
          onAdd={(folderId, documentId) => controller.addDocument(folderId, documentId)}
          onMove={(membership, target) => controller.moveDocument(membership, target)}
          onRanges={(membership, ranges) => controller.setRanges(membership, ranges)}
        />
      ) : null}
    </section>
  );
}

function CollectionStudyMaterials({
  status,
  loading,
  busy,
  error,
  onRun,
  onOpenDeck,
  onOpenGuide,
}: {
  status: StudyTargetBundleStatus | null;
  loading: boolean;
  busy: boolean;
  error: string | null;
  onRun: () => void;
  onOpenDeck?: (deckId: string) => void;
  onOpenGuide?: (collectionId: string, title: string) => void;
}) {
  const action = bundleActionLabel(status, busy);
  return (
    <Surface as="section" className="tw:grid tw:gap-3 tw:p-3" aria-label="Collection study materials">
      <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
        <div className="tw:grid tw:min-w-0 tw:gap-1">
          <strong className="tw:text-sm">Study materials</strong>
          <span className="tw:text-sm tw:text-muted-foreground">
            {loading
              ? "Checking saved artifacts…"
              : status?.sourceChanged
                ? "Sources changed · update available"
                : "One ordinary flashcard deck and one collection study guide."}
          </span>
        </div>
        <Button size="small" variant={status ? "secondary" : "primary"} loading={busy} disabled={loading} onClick={onRun}>
          {action}
        </Button>
      </div>
      <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-x-5 tw:gap-y-2 tw:text-sm" aria-live="polite">
        <ArtifactStatus
          label="Flashcards"
          artifact={status?.flashcards ?? null}
          openLabel="Open Flashcards deck"
          onOpen={status?.deckId && onOpenDeck ? () => onOpenDeck(status.deckId as string) : undefined}
        />
        <ArtifactStatus
          label="Study Guide"
          artifact={status?.studyGuide ?? null}
          openLabel="Open collection study guide"
          onOpen={status?.guideId && status.target.kind === "collection" && onOpenGuide
            ? () => onOpenGuide(status.target.id, status.title)
            : undefined}
        />
      </div>
      {error ? <InlineNotice kind="warning">{error}</InlineNotice> : null}
    </Surface>
  );
}

function ArtifactStatus({
  label,
  artifact,
  openLabel,
  onOpen,
}: {
  label: string;
  artifact: StudyTargetArtifactStatus | null;
  openLabel?: string;
  onOpen?: () => void;
}) {
  const value = artifactStatusLabel(artifact);
  return (
    <span className="tw:flex tw:min-h-9 tw:items-center tw:gap-2">
      <strong>{label}</strong>
      {onOpen ? (
        <Button size="small" variant="text" aria-label={openLabel} onClick={onOpen}>{value}</Button>
      ) : <span className="tw:text-muted-foreground">{value}</span>}
    </span>
  );
}

function artifactStatusLabel(artifact: StudyTargetArtifactStatus | null): string {
  if (!artifact) return "Not created";
  if (artifact.state === "succeeded") return "Ready";
  if (artifact.state === "suppressed") return "Deleted";
  if (artifact.state === "failed") return "Needs attention";
  if (artifact.state === "interrupted") return "Interrupted";
  if (artifact.state === "canceled") return "Stopped";
  if (["pending", "running", "committing"].includes(artifact.state)) {
    return artifact.totalUnits > 0
      ? `${artifact.completedUnits}/${artifact.totalUnits}`
      : "Working…";
  }
  return "Not created";
}

function bundleActionLabel(status: StudyTargetBundleStatus | null, busy: boolean): string {
  if (busy) return "Updating study materials";
  if (!status) return "Create study materials";
  if (status.flashcards.state === "suppressed" || status.studyGuide.state === "suppressed") {
    return "Recreate study materials";
  }
  if (
    ["failed", "interrupted", "canceled"].includes(status.latestBatchState ?? "")
    || status.flashcards.state === "failed"
    || status.studyGuide.state === "failed"
  ) {
    return "Retry study materials";
  }
  return status.sourceChanged ? "Update study materials" : "Update study materials";
}

function FolderTile({ folder, organization, onOpen }: {
  folder: DocumentCollection;
  organization: DocumentCollectionOrganization;
  onOpen: () => void;
}) {
  const members = organization.memberships.filter((entry) => entry.collectionId === folder.id).length;
  const children = organization.collections.filter((entry) => entry.parentId === folder.id).length;
  return (
    <Surface className="tw:grid tw:gap-2 tw:p-3">
      <Button variant="ghost" className="tw:h-auto tw:min-h-11 tw:justify-start tw:whitespace-normal tw:text-left" onClick={onOpen}>
        <span className="tw:grid tw:min-w-0 tw:gap-1">
          <strong className="tw:wrap-anywhere">{folder.name}</strong>
          <small className="tw:text-muted-foreground">{members} source{members === 1 ? "" : "s"} · {children} subfolder{children === 1 ? "" : "s"}{folder.archivedAt ? " · Archived" : ""}</small>
        </span>
      </Button>
    </Surface>
  );
}

function Breadcrumbs({ folder, organization, onOpen, onRoot }: {
  folder: DocumentCollection;
  organization: DocumentCollectionOrganization;
  onOpen: (id: string) => void;
  onRoot: () => void;
}) {
  const trail = folderTrail(folder, organization.collections);
  return (
    <nav aria-label="Document folder breadcrumbs" className="tw:flex tw:min-h-11 tw:flex-wrap tw:items-center tw:gap-1 tw:text-sm">
      <Button size="small" variant="text" onClick={onRoot}>Folders</Button>
      {trail.map((entry) => (
        <span key={entry.id} className="tw:flex tw:items-center tw:gap-1">
          <span aria-hidden="true">/</span>
          <Button size="small" variant="text" disabled={entry.id === folder.id} onClick={() => onOpen(entry.id)}>
            {entry.name}
          </Button>
        </span>
      ))}
    </nav>
  );
}

function FolderOverview({ folder, organization, documents, showArchived, onOpenFolder, onAddSource, disabled }: {
  folder: DocumentCollection;
  organization: DocumentCollectionOrganization;
  documents: Map<string, DocumentSummary>;
  showArchived: boolean;
  onOpenFolder: (id: string) => void;
  onAddSource: () => void;
  disabled: boolean;
}) {
  const children = visibleFolders(organization.collections.filter((entry) => entry.parentId === folder.id), showArchived);
  const memberships = organization.memberships.filter((entry) => entry.collectionId === folder.id);
  return (
    <div className="tw:grid tw:gap-5">
      <section className="tw:grid tw:gap-2">
        <h3 className="tw:m-0 tw:text-base tw:font-semibold">Subfolders</h3>
        {children.length ? <div className="tw:grid tw:gap-2 tw:sm:grid-cols-2">{children.map((child) => (
          <FolderTile key={child.id} folder={child} organization={organization} onOpen={() => onOpenFolder(child.id)} />
        ))}</div> : <p className="tw:m-0 tw:text-sm tw:text-muted-foreground">No subfolders.</p>}
      </section>
      <section className="tw:grid tw:gap-2">
        <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-2">
          <h3 className="tw:m-0 tw:text-base tw:font-semibold">Sources</h3>
          <Button size="small" disabled={disabled} onClick={onAddSource}>Add source</Button>
        </div>
        {memberships.length ? <div className="tw:grid tw:gap-2">{memberships.map((membership) => (
          <SourceTile
            key={membership.id}
            document={documents.get(membership.documentId)}
            rangeLabel={rangeLabel(membership, organization, documents.get(membership.documentId))}
          />
        ))}</div> : <p className="tw:m-0 tw:text-sm tw:text-muted-foreground">No sources in this folder yet.</p>}
      </section>
    </div>
  );
}

function FolderSources({ folder, organization, documents, onAdd, onRanges, onMove, onRemove, disabled }: {
  folder: DocumentCollection;
  organization: DocumentCollectionOrganization;
  documents: Map<string, DocumentSummary>;
  onAdd: () => void;
  onRanges: (membership: DocumentCollectionMembership) => void;
  onMove: (membership: DocumentCollectionMembership) => void;
  onRemove: (membership: DocumentCollectionMembership) => void;
  disabled: boolean;
}) {
  const memberships = organization.memberships.filter((entry) => entry.collectionId === folder.id);
  return (
    <div className="tw:grid tw:gap-3">
      <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-2">
        <p className="tw:m-0 tw:text-sm tw:text-muted-foreground">
          Reading ranges use each source’s canonical {memberships.length ? "unit precision" : "page, slide, image, or section precision"}.
        </p>
        <Button size="small" disabled={disabled} onClick={onAdd}>Add source</Button>
      </div>
      {memberships.length ? memberships.map((membership) => {
        const document = documents.get(membership.documentId);
        return (
          <Surface key={membership.id} className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-3 tw:p-3">
            <SourceTile document={document} rangeLabel={rangeLabel(membership, organization, document)} />
            <div className="tw:ml-auto tw:flex tw:flex-wrap tw:gap-2">
              <Button size="small" disabled={disabled || !document} onClick={() => onRanges(membership)}>Edit range</Button>
              <Button size="small" disabled={disabled} onClick={() => onMove(membership)}>Move</Button>
              <Button size="small" variant="dangerGhost" disabled={disabled} onClick={() => onRemove(membership)}>Remove</Button>
            </div>
          </Surface>
        );
      }) : <p className="tw:m-0 tw:text-sm tw:text-muted-foreground">No sources in this folder yet.</p>}
    </div>
  );
}

function SourceTile({ document, rangeLabel: label }: { document: DocumentSummary | undefined; rangeLabel: string }) {
  return (
    <div className="tw:flex tw:min-w-0 tw:flex-1 tw:items-center tw:gap-3">
      <div aria-hidden="true" className="tw:grid tw:size-11 tw:shrink-0 tw:place-items-center tw:rounded-lg tw:bg-muted tw:px-1 tw:text-[0.65rem] tw:font-semibold tw:uppercase tw:text-muted-foreground">
        {document?.sourceFormat.slice(0, 4) ?? "DOC"}
      </div>
      <div className="tw:grid tw:min-w-0 tw:gap-0.5">
        <strong className="tw:wrap-anywhere tw:text-sm">{document?.title ?? "Source no longer available"}</strong>
        <small className="tw:wrap-anywhere tw:text-muted-foreground">{label}</small>
      </div>
    </div>
  );
}

function CollectionEditorDialog({ dialog, setDialog, organization, documents, documentById, currentFolderId, error, setError, busy, onCreate, onUpdate, onAdd, onMove, onRanges }: {
  dialog: CollectionDialog;
  setDialog: (value: CollectionDialog | null) => void;
  organization: DocumentCollectionOrganization;
  documents: DocumentSummary[];
  documentById: Map<string, DocumentSummary>;
  currentFolderId: string | null;
  error: string | null;
  setError: (value: string | null) => void;
  busy: boolean;
  onCreate: (name: string, parentId: string | null) => Promise<boolean>;
  onUpdate: (folder: DocumentCollection, patch: { name?: string; parentId?: string | null }) => Promise<boolean>;
  onAdd: (folderId: string, documentId: string) => Promise<boolean>;
  onMove: (membership: DocumentCollectionMembership, target: string) => Promise<boolean>;
  onRanges: (membership: DocumentCollectionMembership, ranges: SourceRangeInput[]) => Promise<boolean>;
}) {
  const close = () => { if (!busy) { setDialog(null); setError(null); } };

  if (dialog.kind === "create" || dialog.kind === "rename") {
    const title = dialog.kind === "create" ? "Create document folder" : "Rename document folder";
    return <Dialog title={title} onClose={close} busy={busy}>
      <label className="tw:grid tw:gap-2 tw:text-sm tw:font-medium">Name
        <TextField autoFocus value={dialog.name} maxLength={120} onChange={(event) => setDialog({ ...dialog, name: event.target.value })} />
      </label>
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      <DialogActions>
        <Button onClick={close}>Cancel</Button>
        <Button variant="primary" disabled={!dialog.name.trim()} loading={busy} onClick={() => void (async () => {
          const ok = dialog.kind === "create"
            ? await onCreate(dialog.name, dialog.parentId)
            : await onUpdate(dialog.folder, { name: dialog.name });
          if (ok) close();
        })()}>Save</Button>
      </DialogActions>
    </Dialog>;
  }

  if (dialog.kind === "move-folder") {
    const forbidden = descendantIds(dialog.folder.id, organization.collections);
    forbidden.add(dialog.folder.id);
    return <Dialog title="Move document folder" onClose={close} busy={busy}>
      <label className="tw:grid tw:gap-2 tw:text-sm tw:font-medium">Parent folder
        <NativeSelect value={dialog.targetParentId} onChange={(event) => setDialog({ ...dialog, targetParentId: event.target.value })}>
          <option value="">Top level</option>
          {organization.collections.filter((folder) => !forbidden.has(folder.id)).map((folder) => (
            <option key={folder.id} value={folder.id}>{folder.name}</option>
          ))}
        </NativeSelect>
      </label>
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      <DialogActions>
        <Button onClick={close}>Cancel</Button>
        <Button variant="primary" loading={busy} onClick={() => void (async () => {
          const ok = await onUpdate(dialog.folder, { parentId: dialog.targetParentId || null });
          if (ok) close();
        })()}>Move folder</Button>
      </DialogActions>
    </Dialog>;
  }

  if (dialog.kind === "add-source") {
    if (!currentFolderId) return null;
    const already = new Set(organization.memberships.filter((entry) => entry.collectionId === currentFolderId).map((entry) => entry.documentId));
    const choices = documents.filter((document) => !already.has(document.id));
    return <Dialog title="Add source to folder" onClose={close} busy={busy}>
      <label className="tw:grid tw:gap-2 tw:text-sm tw:font-medium">Document
        <NativeSelect value={dialog.documentId} onChange={(event) => setDialog({ ...dialog, documentId: event.target.value })}>
          <option value="">Select a document…</option>
          {choices.map((document) => <option key={document.id} value={document.id}>{document.title}</option>)}
        </NativeSelect>
      </label>
      {!choices.length ? <p className="tw:text-sm tw:text-muted-foreground">Every retained document is already in this folder.</p> : null}
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      <DialogActions>
        <Button onClick={close}>Cancel</Button>
        <Button variant="primary" disabled={!dialog.documentId} loading={busy} onClick={() => void (async () => {
          const ok = await onAdd(currentFolderId, dialog.documentId);
          if (ok) close();
        })()}>Add source</Button>
      </DialogActions>
    </Dialog>;
  }

  if (dialog.kind === "move-source") {
    const choices = organization.collections.filter((folder) => folder.id !== dialog.membership.collectionId);
    return <Dialog title="Move source to another folder" onClose={close} busy={busy}>
      <label className="tw:grid tw:gap-2 tw:text-sm tw:font-medium">Destination
        <NativeSelect value={dialog.targetCollectionId} onChange={(event) => setDialog({ ...dialog, targetCollectionId: event.target.value })}>
          <option value="">Select a folder…</option>
          {choices.map((folder) => <option key={folder.id} value={folder.id}>{folder.name}</option>)}
        </NativeSelect>
      </label>
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      <DialogActions>
        <Button onClick={close}>Cancel</Button>
        <Button variant="primary" disabled={!dialog.targetCollectionId} loading={busy} onClick={() => void (async () => {
          const ok = await onMove(dialog.membership, dialog.targetCollectionId);
          if (ok) close();
        })()}>Move source</Button>
      </DialogActions>
    </Dialog>;
  }

  const document = documentById.get(dialog.membership.documentId);
  if (!document) return null;
  const unitLabel = unitPlural(document.sourceUnitKind);
  return <Dialog
    title="Reading range"
    description={`Choose ${unitLabel} from 1 to ${document.sourceUnitCount}. Leave blank to use the whole document.`}
    onClose={close}
    busy={busy}
  >
    <label className="tw:grid tw:gap-2 tw:text-sm tw:font-medium">
      {capitalize(unitLabel)}
      <TextField
        autoFocus
        value={dialog.value}
        placeholder={document.sourceUnitCount > 1 ? "1-3, 7" : "1"}
        onChange={(event) => { setDialog({ ...dialog, value: event.target.value }); setError(null); }}
      />
    </label>
    <p className="tw:text-sm tw:text-muted-foreground">
      This folder can only authorize the canonical {document.sourceUnitKind.replaceAll("_", " ")} units you select. Labels do not broaden access.
    </p>
    {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
    <DialogActions>
      <Button onClick={close}>Cancel</Button>
      <Button loading={busy} onClick={() => setDialog({ ...dialog, value: "" })}>Whole document</Button>
      <Button variant="primary" loading={busy} onClick={() => void (async () => {
        const parsed = parseRanges(dialog.value, document);
        if (typeof parsed === "string") { setError(parsed); return; }
        const ok = await onRanges(dialog.membership, parsed);
        if (ok) close();
      })()}>Save range</Button>
    </DialogActions>
  </Dialog>;
}

function parseRanges(value: string, document: DocumentSummary): SourceRangeInput[] | string {
  const text = value.trim();
  if (!text) return [];
  if (document.sourceUnitCount < 1) return "This source does not currently expose bounded source units.";
  const ranges: SourceRangeInput[] = [];
  for (const part of text.split(",")) {
    const match = /^\s*(\d+)(?:\s*-\s*(\d+))?\s*$/.exec(part);
    if (!match) return "Enter ranges like 1-3, 7.";
    const start = Number(match[1]);
    const end = Number(match[2] ?? match[1]);
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 1 || end < start || end > document.sourceUnitCount) {
      return `Use ${unitPlural(document.sourceUnitKind)} between 1 and ${document.sourceUnitCount}.`;
    }
    ranges.push({ unitKind: document.sourceUnitKind, startUnit: start, endUnit: end });
  }
  return ranges;
}

function rangeLabel(
  membership: DocumentCollectionMembership,
  organization: DocumentCollectionOrganization,
  document: DocumentSummary | undefined,
): string {
  if (!document) return "Source unavailable";
  const ranges = organization.ranges.filter((range) => range.membershipId === membership.id);
  if (!ranges.length) return `Whole document · ${document.sourceUnitCount} ${unitPlural(document.sourceUnitKind)}`;
  if (ranges.some((range) => range.sourceSha256 !== document.sha256)) return "Reading range needs review because the source changed";
  const pieces = ranges.map((range) => range.startUnit === range.endUnit
    ? String(range.startUnit)
    : `${range.startUnit}–${range.endUnit}`);
  return `${capitalize(unitPlural(document.sourceUnitKind))} ${pieces.join(", ")}`;
}

function unitPlural(kind: string): string {
  switch (kind) {
    case "page": return "pages";
    case "slide": return "slides";
    case "chapter": return "chapters";
    case "image": return "images";
    case "segment": return "segments";
    case "time_segment": return "time segments";
    default: return "sections";
  }
}

function capitalize(value: string): string {
  return value ? value[0].toUpperCase() + value.slice(1) : value;
}

function folderTrail(folder: DocumentCollection, all: DocumentCollection[]): DocumentCollection[] {
  const byId = new Map(all.map((entry) => [entry.id, entry]));
  const result: DocumentCollection[] = [];
  const seen = new Set<string>();
  let current: DocumentCollection | undefined = folder;
  while (current && !seen.has(current.id)) {
    seen.add(current.id);
    result.unshift(current);
    current = current.parentId ? byId.get(current.parentId) : undefined;
  }
  return result;
}

function descendantIds(folderId: string, all: DocumentCollection[]): Set<string> {
  const result = new Set<string>();
  const stack = [folderId];
  while (stack.length) {
    const parent = stack.pop() as string;
    for (const child of all.filter((entry) => entry.parentId === parent)) {
      if (result.has(child.id)) continue;
      result.add(child.id);
      stack.push(child.id);
    }
  }
  return result;
}

function visibleFolders(folders: DocumentCollection[], showArchived: boolean): DocumentCollection[] {
  return folders.filter((folder) => showArchived || folder.archivedAt === null)
    .sort((left, right) => left.sortOrder - right.sortOrder || left.name.localeCompare(right.name));
}
