import { type ReactNode, useEffect, useMemo, useState } from "react";

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
import { Progress } from "../../ui/primitives/Progress";
import { Surface } from "../../ui/primitives/Surface";
import { TextField } from "../../ui/primitives/TextField";
import { hasDocumentFolderDrag, readDocumentFolderDrag } from "./documentFolderDrag";
import { useDocumentCollectionsController } from "./useDocumentCollectionsController";
import { useStudyTargetBundleController } from "./useStudyTargetBundleController";

type CollectionDialog =
  | { kind: "create"; name: string }
  | { kind: "rename"; folder: DocumentCollection; name: string }
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
      <section aria-label="Document collections" className="tw:mb-6 tw:grid tw:gap-3">
        <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-2">
          <h2 className="tw:m-0 tw:text-lg tw:font-semibold">Document collections</h2>
          <Button size="small" disabled>New collection</Button>
        </div>
        {controller.error ? (
          <InlineNotice kind="error">
            {controller.error} <Button size="small" onClick={() => void controller.load()}>Retry</Button>
          </InlineNotice>
        ) : <p role="status" className="tw:m-0 tw:text-sm tw:text-muted-foreground">Loading collections…</p>}
      </section>
    );
  }

  const current = selectedId
    ? organization.collections.find((folder) => folder.id === selectedId) ?? null
    : null;
  const folders = visibleFolders(organization.collections, showArchived);

  async function deleteCurrent(folder: DocumentCollection): Promise<void> {
    const approved = await confirm({
      title: "Delete this document collection?",
      description: "The collection and its memberships will be removed. Retained source documents and independently created study material are not deleted.",
      confirmLabel: "Delete collection",
      destructive: true,
    });
    if (!approved) return;
    if (await controller.deleteFolder(folder)) setSelectedId(null);
  }

  async function removeMembership(membership: DocumentCollectionMembership): Promise<void> {
    const document = documentById.get(membership.documentId);
    const approved = await confirm({
      title: "Remove this source from the collection?",
      description: `${document?.title ?? "This document"} remains in Documents and in any other collections that reference it.`,
      confirmLabel: "Remove source",
      destructive: true,
    });
    if (approved) await controller.removeDocument(membership);
  }

  function openRanges(membership: DocumentCollectionMembership): void {
    const ranges = organization?.ranges.filter((range) => range.membershipId === membership.id) ?? [];
    setDialog({
      kind: "ranges",
      membership,
      value: ranges.map((range) => range.startUnit === range.endUnit
        ? String(range.startUnit)
        : `${range.startUnit}-${range.endUnit}`).join(", "),
    });
    setDialogError(null);
  }

  function addDroppedDocument(folderId: string, documentId: string): void {
    if (!documentById.has(documentId)) return;
    void controller.addDocument(folderId, documentId);
  }

  return (
    <section aria-label="Document collections" className="tw:mb-6 tw:grid tw:gap-4">
      {controller.error ? <InlineNotice kind="error">{controller.error}</InlineNotice> : null}
      {!current ? (
        <>
          <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3">
            <h2 className="tw:m-0 tw:text-lg tw:font-semibold">Document collections</h2>
            <Button
              size="small"
              disabled={controller.busy}
              onClick={() => { setDialog({ kind: "create", name: "" }); setDialogError(null); }}
            >
              + New collection
            </Button>
          </div>
          <label className="tw:flex tw:min-h-11 tw:w-fit tw:items-center tw:gap-2 tw:text-sm">
            <Checkbox checked={showArchived} onCheckedChange={setShowArchived} />
            Show archived collections
          </label>
          {folders.length ? (
            <div className="tw:grid tw:gap-2 tw:sm:grid-cols-2 tw:xl:grid-cols-3">
              {folders.map((folder) => (
                <FolderTile
                  key={folder.id}
                  folder={folder}
                  organization={organization}
                  disabled={controller.busy}
                  onDropDocument={(documentId) => addDroppedDocument(folder.id, documentId)}
                  onOpen={() => setSelectedId(folder.id)}
                />
              ))}
            </div>
          ) : (
            <p className="tw:m-0 tw:text-sm tw:text-muted-foreground">
              {showArchived ? "No collections yet." : "No active collections yet."}
            </p>
          )}
        </>
      ) : (
        <>
          <div>
            <Button size="small" variant="text" onClick={() => setSelectedId(null)}>← Collections</Button>
          </div>
          <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-start tw:justify-between tw:gap-3">
            <h2 className="tw:m-0 tw:min-w-0 tw:wrap-anywhere tw:text-lg tw:font-semibold">{current.name}</h2>
            <div className="tw:flex tw:flex-wrap tw:gap-2">
              <Button size="small" disabled={controller.busy}
                onClick={() => { setDialog({ kind: "rename", folder: current, name: current.name }); setDialogError(null); }}>
                Rename
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
          <FolderDropZone
            folderId={current.id}
            disabled={controller.busy}
            onDropDocument={(documentId) => addDroppedDocument(current.id, documentId)}
          >
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
          </FolderDropZone>
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
          onCreate={(name) => controller.createFolder(name)}
          onUpdate={(folder, patch) => controller.updateFolder(folder, patch)}
          onAdd={(folderId, documentId) => controller.addDocument(folderId, documentId)}
          onMove={(membership, target) => controller.moveDocument(membership, target)}
          onRanges={(membership, ranges) => controller.setRanges(membership, ranges)}
        />
      ) : null}
    </section>
  );
}

function FolderDropZone({ folderId, disabled, onDropDocument, children }: {
  folderId: string;
  disabled: boolean;
  onDropDocument: (documentId: string) => void;
  children: ReactNode;
}) {
  const [active, setActive] = useState(false);
  return (
    <div
      data-document-folder-drop={folderId}
      data-drop-active={active || undefined}
      className={active ? "tw:rounded-xl tw:ring-2 tw:ring-primary tw:ring-offset-2 tw:ring-offset-background" : ""}
      onDragEnter={(event) => {
        if (disabled || !hasDocumentFolderDrag(event.dataTransfer)) return;
        event.preventDefault();
        setActive(true);
      }}
      onDragOver={(event) => {
        if (disabled || !hasDocumentFolderDrag(event.dataTransfer)) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = "copy";
        if (!active) setActive(true);
      }}
      onDragLeave={(event) => {
        if (event.currentTarget.contains(event.relatedTarget as Node | null)) return;
        setActive(false);
      }}
      onDrop={(event) => {
        if (disabled) return;
        const documentId = readDocumentFolderDrag(event.dataTransfer);
        if (!documentId) return;
        event.preventDefault();
        setActive(false);
        onDropDocument(documentId);
      }}
    >
      {children}
    </div>
  );
}

function CollectionStudyMaterials({ status, loading, busy, error, onRun, onOpenDeck, onOpenGuide }: {
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
            {loading ? "Checking saved artifacts…"
              : status?.sourceChanged ? "Sources changed · update available"
                : "One ordinary flashcard deck and one collection study guide."}
          </span>
        </div>
        <Button size="small" variant={status ? "secondary" : "primary"} loading={busy} disabled={loading} onClick={onRun}>
          {action}
        </Button>
      </div>
      {busy ? <CollectionBundleProgress status={status} /> : null}
      <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-x-5 tw:gap-y-2 tw:text-sm" aria-live="polite">
        <ArtifactStatus label="Flashcards" artifact={status?.flashcards ?? null} openLabel="Open Flashcards deck"
          onOpen={status?.deckId && onOpenDeck ? () => onOpenDeck(status.deckId as string) : undefined} />
        <ArtifactStatus label="Study Guide" artifact={status?.studyGuide ?? null} openLabel="Open collection study guide"
          onOpen={status?.guideId && status.target.kind === "collection" && onOpenGuide
            ? () => onOpenGuide(status.target.id, status.title) : undefined} />
      </div>
      {error ? <InlineNotice kind="warning">{error}</InlineNotice> : null}
    </Surface>
  );
}

function CollectionBundleProgress({ status }: { status: StudyTargetBundleStatus | null }) {
  const active = bundleProgress(status);
  return (
    <div className="tw:grid tw:gap-2" role="status" aria-live="polite">
      <div className="tw:flex tw:items-center tw:justify-between tw:gap-3 tw:text-sm">
        <span>{active.title}</span>
        <span className="tw:text-muted-foreground">{active.label}</span>
      </div>
      <Progress
        value={active.value}
        max={active.max}
        label="Collection study-material generation progress"
        valueText={active.label}
      />
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
      title: "Writing collection study guide",
      label: determinate ? `${guide.completedUnits} / ${guide.totalUnits}` : "Working…",
      value: determinate ? guide.completedUnits : null,
      max: determinate ? guide.totalUnits : 100,
    };
  }
  const cards = status.flashcards;
  if (cards.state === "running" || cards.state === "committing") {
    const determinate = cards.totalUnits > 0;
    return {
      title: "Building collection flashcards",
      label: determinate ? `${cards.completedUnits} / ${cards.totalUnits}` : "Working…",
      value: determinate ? cards.completedUnits : null,
      max: determinate ? cards.totalUnits : 100,
    };
  }
  if (cards.state === "succeeded" && guide.state === "pending") {
    return { title: "Preparing collection study guide", label: "Working…", value: null, max: 100 };
  }
  return { title: "Preparing study materials", label: "Working…", value: null, max: 100 };
}

function ArtifactStatus({ label, artifact, openLabel, onOpen }: {
  label: string;
  artifact: StudyTargetArtifactStatus | null;
  openLabel?: string;
  onOpen?: () => void;
}) {
  const value = artifactStatusLabel(artifact);
  return (
    <span className="tw:flex tw:min-h-9 tw:items-center tw:gap-2">
      <strong>{label}</strong>
      {onOpen ? <Button size="small" variant="text" aria-label={openLabel} onClick={onOpen}>{value}</Button>
        : <span className="tw:text-muted-foreground">{value}</span>}
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
    return artifact.totalUnits > 0 ? `${artifact.completedUnits}/${artifact.totalUnits}` : "Working…";
  }
  return "Not created";
}

function bundleActionLabel(status: StudyTargetBundleStatus | null, busy: boolean): string {
  if (busy) return "Updating study materials";
  if (!status) return "Create study materials";
  if (status.flashcards.state === "suppressed" || status.studyGuide.state === "suppressed") return "Recreate study materials";
  if (["failed", "interrupted", "canceled"].includes(status.latestBatchState ?? "")
    || status.flashcards.state === "failed" || status.studyGuide.state === "failed") return "Retry study materials";
  return "Update study materials";
}

function FolderTile({ folder, organization, disabled, onDropDocument, onOpen }: {
  folder: DocumentCollection;
  organization: DocumentCollectionOrganization;
  disabled: boolean;
  onDropDocument: (documentId: string) => void;
  onOpen: () => void;
}) {
  const members = organization.memberships.filter((entry) => entry.collectionId === folder.id).length;
  return (
    <FolderDropZone folderId={folder.id} disabled={disabled} onDropDocument={onDropDocument}>
      <Surface className="tw:grid tw:gap-2 tw:p-3">
        <Button variant="ghost" className="tw:h-auto tw:min-h-11 tw:justify-start tw:whitespace-normal tw:text-left" onClick={onOpen}>
          <span className="tw:grid tw:min-w-0 tw:gap-1">
            <strong className="tw:wrap-anywhere">{folder.name}</strong>
            <small className="tw:text-muted-foreground">
              {members} source{members === 1 ? "" : "s"}{folder.archivedAt ? " · Archived" : ""}
            </small>
          </span>
        </Button>
      </Surface>
    </FolderDropZone>
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
    <section className="tw:grid tw:gap-3" aria-label="Collection sources">
      <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-2">
        <h3 className="tw:m-0 tw:text-base tw:font-semibold">Sources</h3>
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
      }) : <p className="tw:m-0 tw:text-sm tw:text-muted-foreground">No sources in this collection yet.</p>}
    </section>
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
  onCreate: (name: string) => Promise<boolean>;
  onUpdate: (folder: DocumentCollection, patch: { name?: string; archived?: boolean }) => Promise<boolean>;
  onAdd: (folderId: string, documentId: string) => Promise<boolean>;
  onMove: (membership: DocumentCollectionMembership, target: string) => Promise<boolean>;
  onRanges: (membership: DocumentCollectionMembership, ranges: SourceRangeInput[]) => Promise<boolean>;
}) {
  const close = () => { if (!busy) { setDialog(null); setError(null); } };

  if (dialog.kind === "create" || dialog.kind === "rename") {
    const title = dialog.kind === "create" ? "Create document collection" : "Rename document collection";
    return (
      <Dialog title={title} onClose={close} busy={busy} width="small">
        <div data-collection-dialog-body className="tw:grid tw:min-w-0 tw:gap-4 tw:p-4">
          <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-sm tw:font-medium">
            Name
            <TextField autoFocus value={dialog.name} maxLength={120}
              onChange={(event) => setDialog({ ...dialog, name: event.target.value })} />
          </label>
          {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
          <DialogActions>
            <Button onClick={close}>Cancel</Button>
            <Button variant="primary" disabled={!dialog.name.trim()} loading={busy} onClick={() => void (async () => {
              const ok = dialog.kind === "create" ? await onCreate(dialog.name) : await onUpdate(dialog.folder, { name: dialog.name });
              if (ok) close();
            })()}>Save</Button>
          </DialogActions>
        </div>
      </Dialog>
    );
  }

  if (dialog.kind === "add-source") {
    if (!currentFolderId) return null;
    const already = new Set(organization.memberships.filter((entry) => entry.collectionId === currentFolderId).map((entry) => entry.documentId));
    const choices = documents.filter((document) => !already.has(document.id));
    return (
      <Dialog title="Add source to collection" onClose={close} busy={busy} width="small">
        <div data-collection-dialog-body className="tw:grid tw:min-w-0 tw:gap-4 tw:p-4">
          <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-sm tw:font-medium">
            Document
            <NativeSelect value={dialog.documentId} onChange={(event) => setDialog({ ...dialog, documentId: event.target.value })}>
              <option value="">Select a document…</option>
              {choices.map((document) => <option key={document.id} value={document.id}>{document.title}</option>)}
            </NativeSelect>
          </label>
          {!choices.length ? <p className="tw:m-0 tw:text-sm tw:text-muted-foreground">Every retained document is already in this collection.</p> : null}
          {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
          <DialogActions>
            <Button onClick={close}>Cancel</Button>
            <Button variant="primary" disabled={!dialog.documentId} loading={busy} onClick={() => void (async () => {
              const ok = await onAdd(currentFolderId, dialog.documentId);
              if (ok) close();
            })()}>Add source</Button>
          </DialogActions>
        </div>
      </Dialog>
    );
  }

  if (dialog.kind === "move-source") {
    const choices = organization.collections.filter((folder) => folder.id !== dialog.membership.collectionId);
    return (
      <Dialog title="Move source to another collection" onClose={close} busy={busy} width="small">
        <div data-collection-dialog-body className="tw:grid tw:min-w-0 tw:gap-4 tw:p-4">
          <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-sm tw:font-medium">
            Destination
            <NativeSelect value={dialog.targetCollectionId} onChange={(event) => setDialog({ ...dialog, targetCollectionId: event.target.value })}>
              <option value="">Select a collection…</option>
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
        </div>
      </Dialog>
    );
  }

  const document = documentById.get(dialog.membership.documentId);
  if (!document) return null;
  const unitLabel = unitPlural(document.sourceUnitKind);
  return (
    <Dialog title="Reading range"
      description={`Choose ${unitLabel} from 1 to ${document.sourceUnitCount}. Leave blank to use the whole document.`}
      onClose={close} busy={busy} width="small">
      <div data-collection-dialog-body className="tw:grid tw:min-w-0 tw:gap-4 tw:p-4">
        <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-sm tw:font-medium">
          {capitalize(unitLabel)}
          <TextField autoFocus value={dialog.value} placeholder={document.sourceUnitCount > 1 ? "1-3, 7" : "1"}
            onChange={(event) => { setDialog({ ...dialog, value: event.target.value }); setError(null); }} />
        </label>
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
      </div>
    </Dialog>
  );
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

function rangeLabel(membership: DocumentCollectionMembership, organization: DocumentCollectionOrganization, document: DocumentSummary | undefined): string {
  if (!document) return "Source unavailable";
  const ranges = organization.ranges.filter((range) => range.membershipId === membership.id);
  if (!ranges.length) return `Whole document · ${document.sourceUnitCount} ${unitPlural(document.sourceUnitKind)}`;
  if (ranges.some((range) => range.sourceSha256 !== document.sha256)) return "Reading range needs review because the source changed";
  const pieces = ranges.map((range) => range.startUnit === range.endUnit ? String(range.startUnit) : `${range.startUnit}–${range.endUnit}`);
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

function visibleFolders(folders: DocumentCollection[], showArchived: boolean): DocumentCollection[] {
  return folders.filter((folder) => showArchived || folder.archivedAt === null)
    .sort((left, right) => left.sortOrder - right.sortOrder || left.name.localeCompare(right.name));
}
