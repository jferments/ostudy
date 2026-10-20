import { type ReactNode, useEffect, useMemo, useState } from "react";

import type {
  DocumentCollection,
  DocumentCollectionMembership,
  DocumentCollectionOrganization,
  SourceRangeInput,
} from "../../ipc/collections";
import type { DocumentSummary } from "../../ipc/documents";
import type { StudyTargetBundleStatus } from "../../ipc/studyMaterials";
import { Dialog } from "../../ui/overlays/Dialog";
import { OverflowMenu, type OverflowMenuItem } from "../../ui/overlays/OverflowMenu";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Checkbox } from "../../ui/primitives/FormControls";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { Progress } from "../../ui/primitives/Progress";
import { TextField } from "../../ui/primitives/TextField";
import { canAnnotateDocument } from "./annotator/canAnnotate";
import { hasDocumentFolderDrag, readDocumentFolderDrag } from "./documentFolderDrag";
import { formatDocumentType } from "./DocumentProgress";
import type { DocumentCollectionsController } from "./useDocumentCollectionsController";
import { useStudyTargetBundleController } from "./useStudyTargetBundleController";

export type CollectionDialog =
  | { kind: "create"; name: string }
  | { kind: "rename"; folder: DocumentCollection; name: string }
  | { kind: "add-documents"; folderId: string; documentIds: string[] }
  | { kind: "move-document"; documentId: string; targetCollectionId: string }
  | { kind: "ranges"; membership: DocumentCollectionMembership; value: string };

/**
 * Library's document collections, each drawn as a stack so it reads apart
 * from a single document. The page owns which dialog is open, so its own
 * actions (New collection, a document's Move to a collection) open them too.
 */
export function DocumentCollectionsWorkspace({
  controller,
  dialog,
  onDialogChange,
  documents,
  refreshDecks,
  onStudyCollection,
  onOpenGuide,
  onOpenDocument,
  onAnnotateDocument,
  onShowFigures,
  onRenameDocument,
  onUploadInto,
  onContainedDocumentIdsChange,
}: {
  controller: DocumentCollectionsController;
  dialog: CollectionDialog | null;
  onDialogChange: (dialog: CollectionDialog | null) => void;
  documents: DocumentSummary[];
  refreshDecks?: () => Promise<unknown>;
  onStudyCollection?: (collectionId: string) => void;
  onOpenGuide?: (collectionId: string, title: string) => void;
  onOpenDocument?: (document: DocumentSummary) => void;
  onAnnotateDocument?: (document: DocumentSummary) => void;
  onShowFigures?: (document: DocumentSummary) => void;
  onRenameDocument?: (document: DocumentSummary) => void;
  /** Uploads a new document straight into the collection. */
  onUploadInto?: (collectionId: string) => void;
  onContainedDocumentIdsChange?: (documentIds: string[]) => void;
}) {
  const confirm = useConfirm();
  const [dialogError, setDialogError] = useState<string | null>(null);
  const organization = controller.organization;
  const documentById = useMemo(
    () => new Map(documents.map((document) => [document.id, document])),
    [documents],
  );

  useEffect(() => {
    if (!organization) {
      onContainedDocumentIdsChange?.([]);
      return;
    }
    onContainedDocumentIdsChange?.(
      [...new Set(organization.memberships.map((membership) => membership.documentId))],
    );
  }, [onContainedDocumentIdsChange, organization]);

  function open(next: CollectionDialog): void {
    onDialogChange(next);
    setDialogError(null);
  }

  if (!organization) {
    return controller.error ? (
      <InlineNotice kind="error" className="tw:mb-6">
        {controller.error} <Button size="small" onClick={() => void controller.load()}>Retry</Button>
      </InlineNotice>
    ) : null;
  }

  const loadedOrganization = organization;
  const folders = [...loadedOrganization.collections]
    .sort((left, right) => left.sortOrder - right.sortOrder || left.name.localeCompare(right.name));

  async function deleteCurrent(folder: DocumentCollection): Promise<void> {
    const approved = await confirm({
      title: "Delete this document collection?",
      description: "The collection is removed and its documents and decks move to No subject. Retained source documents and independently created study material are not deleted.",
      confirmLabel: "Delete collection",
      destructive: true,
    });
    if (approved) await controller.deleteFolder(folder);
  }

  async function removeMembership(membership: DocumentCollectionMembership): Promise<void> {
    const document = documentById.get(membership.documentId);
    const approved = await confirm({
      title: "Remove this document from the collection?",
      description: `${document?.title ?? "This document"} stays in Documents without a collection, and its reading range is cleared.`,
      confirmLabel: "Remove from collection",
      destructive: true,
    });
    if (approved) await controller.removeDocument(membership);
  }

  function openRanges(membership: DocumentCollectionMembership): void {
    const ranges = loadedOrganization.ranges.filter((range) => range.membershipId === membership.id);
    open({
      kind: "ranges",
      membership,
      value: ranges.map((range) => range.startUnit === range.endUnit
        ? String(range.startUnit)
        : `${range.startUnit}-${range.endUnit}`).join(", "),
    });
  }

  function addDroppedDocument(folderId: string, documentId: string): void {
    if (!documentById.has(documentId)) return;
    void controller.addDocument(folderId, documentId);
  }

  return (
    <>
      {folders.length || controller.error ? (
        <section aria-labelledby="document-collections-heading" className="tw:mb-8 tw:grid tw:gap-4">
          {folders.length ? (
            <h2 id="document-collections-heading" className="tw:m-0 tw:text-body tw:font-semibold">Collections</h2>
          ) : null}
          {controller.error ? <InlineNotice kind="error">{controller.error}</InlineNotice> : null}
          <div className="tw:grid tw:grid-cols-[repeat(auto-fill,minmax(min(100%,22rem),1fr))] tw:items-start tw:gap-5">
            {folders.map((folder) => (
              <CollectionCard
                key={folder.id}
                folder={folder}
                organization={loadedOrganization}
                documents={documentById}
                mutationBusy={controller.busy}
                refreshDecks={refreshDecks}
                onStudyCollection={onStudyCollection}
                onOpenGuide={onOpenGuide}
                onOpenDocument={onOpenDocument}
                onAnnotateDocument={onAnnotateDocument}
                onShowFigures={onShowFigures}
                onRenameDocument={onRenameDocument}
                onRename={() => open({ kind: "rename", folder, name: folder.name })}
                onDelete={() => void deleteCurrent(folder)}
                onAdd={() => open({ kind: "add-documents", folderId: folder.id, documentIds: [] })}
                onRanges={openRanges}
                onMove={(membership) => open({
                  kind: "move-document", documentId: membership.documentId, targetCollectionId: "",
                })}
                onRemove={(membership) => void removeMembership(membership)}
                onDropDocument={(documentId) => addDroppedDocument(folder.id, documentId)}
              />
            ))}
          </div>
        </section>
      ) : null}
      {dialog ? (
        <CollectionEditorDialog
          dialog={dialog}
          setDialog={onDialogChange}
          organization={organization}
          documents={documents}
          documentById={documentById}
          error={dialogError}
          setError={setDialogError}
          busy={controller.busy}
          onCreate={(name) => controller.createFolder(name)}
          onUpdate={(folder, patch) => controller.updateFolder(folder, patch)}
          onAdd={(folderId, documentIds) => controller.addDocuments(folderId, documentIds)}
          onUploadInto={onUploadInto}
          onMove={(documentId, target) => {
            const membership = organization.memberships.find((entry) => entry.documentId === documentId);
            return membership ? controller.moveDocument(membership, target) : controller.addDocument(target, documentId);
          }}
          onRanges={(membership, ranges) => controller.setRanges(membership, ranges)}
        />
      ) : null}
    </>
  );
}

function CollectionCard({
  folder,
  organization,
  documents,
  mutationBusy,
  refreshDecks,
  onStudyCollection,
  onOpenGuide,
  onOpenDocument,
  onAnnotateDocument,
  onShowFigures,
  onRenameDocument,
  onRename,
  onDelete,
  onAdd,
  onRanges,
  onMove,
  onRemove,
  onDropDocument,
}: {
  folder: DocumentCollection;
  organization: DocumentCollectionOrganization;
  documents: Map<string, DocumentSummary>;
  mutationBusy: boolean;
  refreshDecks?: () => Promise<unknown>;
  onStudyCollection?: (collectionId: string) => void;
  onOpenGuide?: (collectionId: string, title: string) => void;
  onOpenDocument?: (document: DocumentSummary) => void;
  onAnnotateDocument?: (document: DocumentSummary) => void;
  onShowFigures?: (document: DocumentSummary) => void;
  onRenameDocument?: (document: DocumentSummary) => void;
  onRename: () => void;
  onDelete: () => void;
  onAdd: () => void;
  onRanges: (membership: DocumentCollectionMembership) => void;
  onMove: (membership: DocumentCollectionMembership) => void;
  onRemove: (membership: DocumentCollectionMembership) => void;
  onDropDocument: (documentId: string) => void;
}) {
  const bundle = useStudyTargetBundleController({
    target: { kind: "collection", id: folder.id },
    changed: refreshDecks,
    refreshKey: organization.revision,
  });
  const memberships = organization.memberships.filter((entry) => entry.collectionId === folder.id);
  const status = bundle.status;
  const headingId = `collection-${folder.id}`;
  const update = updateLabel(status);
  const menu: OverflowMenuItem[] = [
    { id: "add", label: "Add documents…", onSelect: onAdd, disabled: mutationBusy },
    { id: "rename", label: "Rename…", onSelect: onRename, disabled: mutationBusy },
  ];
  if (update && memberships.length && !bundle.busy) {
    menu.push({ id: "update", label: update, onSelect: () => void bundle.run(), disabled: bundle.loading });
  }
  menu.push({ id: "delete", label: "Delete collection", onSelect: onDelete, disabled: mutationBusy, destructive: true });

  return (
    <FolderDropZone folderId={folder.id} disabled={mutationBusy} onDropDocument={onDropDocument}>
      <article aria-labelledby={headingId} data-testid="document-collection" className="tw:relative tw:mr-1.5 tw:mb-1.5 tw:min-w-0">
        {/* The offset sheet behind the card makes a collection read as a stack of documents. */}
        <span aria-hidden="true" className="tw:absolute tw:inset-0 tw:translate-x-1.5 tw:translate-y-1.5 tw:rounded-xl tw:border tw:border-solid tw:border-primary/30 tw:bg-primary/12" />
        <div className="tw:relative tw:min-w-0 tw:rounded-xl tw:bg-card">
          <div className="tw:grid tw:min-w-0 tw:gap-3 tw:rounded-xl tw:border tw:border-solid tw:border-primary/30 tw:bg-primary/7 tw:p-4 tw:compact:p-3">
            <div className="tw:flex tw:min-w-0 tw:items-center tw:gap-3">
              <span aria-hidden="true" className="tw:grid tw:size-10 tw:shrink-0 tw:place-items-center tw:rounded-lg tw:bg-primary tw:text-primary-foreground">
                <Icon name="folder" size={20} />
              </span>
              <div className="tw:grid tw:min-w-0 tw:flex-1">
                <h3 id={headingId} className="tw:m-0 tw:wrap-anywhere tw:text-body tw:font-semibold">{folder.name}</h3>
                <span className="tw:text-small tw:text-muted-foreground">{collectionSummary(memberships.length, status)}</span>
              </div>
              <OverflowMenu label={`More actions for ${folder.name}`} items={menu} />
            </div>
            {memberships.length ? (
              <ul aria-label={`Documents in ${folder.name}`} className="tw:m-0 tw:grid tw:list-none tw:rounded-lg tw:border tw:border-solid tw:border-primary/20 tw:bg-card tw:p-0">
                {memberships.map((membership) => (
                  <CollectionMember
                    key={membership.id}
                    membership={membership}
                    organization={organization}
                    document={documents.get(membership.documentId)}
                    canMove={organization.collections.length > 1}
                    disabled={mutationBusy}
                    onOpen={onOpenDocument}
                    onAnnotate={onAnnotateDocument}
                    onShowFigures={onShowFigures}
                    onRenameDocument={onRenameDocument}
                    onRanges={onRanges}
                    onMove={onMove}
                    onRemove={onRemove}
                  />
                ))}
              </ul>
            ) : (
              <Button
                variant="ghost"
                disabled={mutationBusy}
                onClick={onAdd}
                className="tw:min-h-24 tw:w-full tw:justify-center tw:rounded-lg tw:border-2 tw:border-dashed tw:border-primary/30 tw:text-primary tw:hover:bg-primary/6 tw:hover:text-primary"
              >
                <Icon name="plus" size={18} />
                Add documents
              </Button>
            )}
            {memberships.length ? (
              <CollectionStudyActions
                status={status}
                loading={bundle.loading}
                busy={bundle.busy}
                canceling={bundle.canceling}
                onRun={() => void bundle.run()}
                onStop={() => void bundle.stop()}
                onStudy={onStudyCollection}
                onOpenGuide={onOpenGuide}
              />
            ) : null}
            {bundle.error ? <InlineNotice kind="warning">{bundle.error}</InlineNotice> : null}
          </div>
        </div>
      </article>
    </FolderDropZone>
  );
}

function CollectionMember({ membership, organization, document, canMove, disabled, onOpen, onAnnotate, onShowFigures, onRenameDocument, onRanges, onMove, onRemove }: {
  membership: DocumentCollectionMembership;
  organization: DocumentCollectionOrganization;
  document: DocumentSummary | undefined;
  canMove: boolean;
  disabled: boolean;
  onOpen?: (document: DocumentSummary) => void;
  onAnnotate?: (document: DocumentSummary) => void;
  onShowFigures?: (document: DocumentSummary) => void;
  onRenameDocument?: (document: DocumentSummary) => void;
  onRanges: (membership: DocumentCollectionMembership) => void;
  onMove: (membership: DocumentCollectionMembership) => void;
  onRemove: (membership: DocumentCollectionMembership) => void;
}) {
  const title = document?.title ?? "Document no longer available";
  const items: OverflowMenuItem[] = [];
  if (document && onOpen) items.push({ id: "open", label: "Open", onSelect: () => onOpen(document) });
  if (document && onAnnotate && canAnnotateDocument(document)) {
    items.push({ id: "annotate", label: "Annotate", onSelect: () => onAnnotate(document) });
  }
  if (document && onShowFigures && canAnnotateDocument(document)) {
    items.push({ id: "figures", label: "Figures", onSelect: () => onShowFigures(document) });
  }
  if (document && onRenameDocument) {
    items.push({ id: "rename", label: "Rename…", onSelect: () => onRenameDocument(document) });
  }
  items.push({ id: "range", label: "Reading range…", onSelect: () => onRanges(membership), disabled: disabled || !document });
  if (canMove) items.push({ id: "move", label: "Move to another collection…", onSelect: () => onMove(membership), disabled });
  items.push({ id: "remove", label: "Remove from collection", onSelect: () => onRemove(membership), disabled, destructive: true });
  return (
    <li className="tw:flex tw:min-h-11 tw:min-w-0 tw:items-center tw:gap-3 tw:border-0 tw:border-t tw:border-solid tw:border-primary/15 tw:py-1 tw:pr-1 tw:pl-3 tw:first:border-t-0">
      <Icon name={document?.sourceFormat === "image_bundle" ? "image" : "document"} size={16} className="tw:shrink-0 tw:text-muted-foreground" />
      <span className="tw:grid tw:min-w-0 tw:flex-1">
        <span className="tw:truncate tw:text-small tw:font-medium">{title}</span>
        <span className="tw:truncate tw:text-caption tw:text-muted-foreground">{rangeLabel(membership, organization, document)}</span>
      </span>
      {document ? (
        <span className="tw:shrink-0 tw:text-caption tw:text-muted-foreground tw:compact:hidden">
          {document.generatedCardCount.toLocaleString()} card{document.generatedCardCount === 1 ? "" : "s"}
        </span>
      ) : null}
      <OverflowMenu label={`More actions for ${title}`} items={items} />
    </li>
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
      className={active ? "tw:rounded-xl tw:ring-2 tw:ring-primary tw:ring-offset-4 tw:ring-offset-background" : "tw:min-w-0"}
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

/** Study and Read guide once they exist; otherwise the one action that makes them. */
function CollectionStudyActions({
  status,
  loading,
  busy,
  canceling,
  onRun,
  onStop,
  onStudy,
  onOpenGuide,
}: {
  status: StudyTargetBundleStatus | null;
  loading: boolean;
  busy: boolean;
  canceling: boolean;
  onRun: () => void;
  onStop: () => void;
  onStudy?: (collectionId: string) => void;
  onOpenGuide?: (collectionId: string, title: string) => void;
}) {
  if (busy) {
    return (
      <div className="tw:flex tw:min-w-0 tw:items-center tw:gap-3">
        <CollectionBundleProgress status={status} />
        <Button
          size="small"
          variant="dangerSecondary"
          disabled={canceling}
          aria-label={canceling ? "Stopping collection generation" : "Stop collection generation"}
          onClick={onStop}
        >
          <Icon name="stop" size={16} />
          {canceling ? "Stopping…" : "Stop"}
        </Button>
      </div>
    );
  }
  const study = status?.flashcards.state === "succeeded" && status.target.kind === "collection" && onStudy
    ? () => onStudy(status.target.id) : undefined;
  const guide = status?.guideId && status.target.kind === "collection" && onOpenGuide
    ? () => onOpenGuide(status.target.id, status.title) : undefined;
  const update = updateLabel(status);
  const needsUpdate = status !== null && (status.sourceChanged || update !== "Update study materials");
  return (
    <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2" aria-live="polite">
      {study ? <Button size="small" variant="primary" onClick={study}>Study</Button> : null}
      {guide ? <Button size="small" onClick={guide}>Read guide</Button> : null}
      {!status ? (
        <Button size="small" variant="primary" disabled={loading} onClick={onRun}>Make study materials</Button>
      ) : needsUpdate && update ? (
        <Button size="small" variant={study || guide ? "secondary" : "primary"} disabled={loading} onClick={onRun}>{update}</Button>
      ) : null}
    </div>
  );
}

function CollectionBundleProgress({ status }: { status: StudyTargetBundleStatus | null }) {
  const active = bundleProgress(status);
  return (
    <div className="tw:grid tw:min-w-0 tw:flex-1 tw:gap-1.5" role="status" aria-live="polite">
      <div className="tw:flex tw:items-center tw:justify-between tw:gap-3 tw:text-small">
        <span className="tw:truncate">{active.title}</span>
        <span className="tw:shrink-0 tw:text-muted-foreground">{active.label}</span>
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

function collectionSummary(count: number, status: StudyTargetBundleStatus | null): string {
  if (count === 0) return "Empty";
  const parts = [`${count} document${count === 1 ? "" : "s"}`];
  if (status?.guideId) parts.push("study guide");
  if (status?.sourceChanged) parts.push("update available");
  else if (status && (status.flashcards.state === "failed" || status.studyGuide.state === "failed")) parts.push("needs attention");
  return parts.join(" · ");
}

function updateLabel(status: StudyTargetBundleStatus | null): string | null {
  if (!status) return null;
  if (status.flashcards.state === "suppressed" || status.studyGuide.state === "suppressed") return "Recreate study materials";
  if (["failed", "interrupted", "canceled"].includes(status.latestBatchState ?? "")
    || status.flashcards.state === "failed" || status.studyGuide.state === "failed") return "Retry study materials";
  return "Update study materials";
}

function CollectionEditorDialog({ dialog, setDialog, organization, documents, documentById, error, setError, busy, onCreate, onUpdate, onAdd, onUploadInto, onMove, onRanges }: {
  dialog: CollectionDialog;
  setDialog: (value: CollectionDialog | null) => void;
  organization: DocumentCollectionOrganization;
  documents: DocumentSummary[];
  documentById: Map<string, DocumentSummary>;
  error: string | null;
  setError: (value: string | null) => void;
  busy: boolean;
  onCreate: (name: string) => Promise<boolean>;
  onUpdate: (folder: DocumentCollection, patch: { name?: string }) => Promise<boolean>;
  onAdd: (folderId: string, documentIds: string[]) => Promise<boolean>;
  onUploadInto?: (folderId: string) => void;
  onMove: (documentId: string, target: string) => Promise<boolean>;
  onRanges: (membership: DocumentCollectionMembership, ranges: SourceRangeInput[]) => Promise<boolean>;
}) {
  const close = () => { if (!busy) { setDialog(null); setError(null); } };
  const collectionName = (id: string) => organization.collections.find((folder) => folder.id === id)?.name;

  if (dialog.kind === "create" || dialog.kind === "rename") {
    const title = dialog.kind === "create" ? "New collection" : "Rename collection";
    return (
      <Dialog title={title} onClose={close} busy={busy} width="small"
        onSubmit={() => void (async () => {
          if (!dialog.name.trim()) return;
          const ok = dialog.kind === "create" ? await onCreate(dialog.name) : await onUpdate(dialog.folder, { name: dialog.name });
          if (ok) close();
        })()}
        actions={<>
          <Button onClick={close}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={!dialog.name.trim()} loading={busy}>
            {dialog.kind === "create" ? "Create" : "Save"}
          </Button>
        </>}>
        <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-small tw:font-medium">
          Name
          <TextField autoFocus value={dialog.name} maxLength={120}
            onChange={(event) => setDialog({ ...dialog, name: event.target.value })} />
        </label>
        {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      </Dialog>
    );
  }

  if (dialog.kind === "add-documents") {
    const membershipOf = new Map(organization.memberships.map((entry) => [entry.documentId, entry]));
    const choices = documents.filter((document) => membershipOf.get(document.id)?.collectionId !== dialog.folderId);
    const selected = new Set(dialog.documentIds);
    const count = dialog.documentIds.length;
    return (
      <Dialog title="Add documents" onClose={close} busy={busy} width="small"
        actions={<>
          <Button onClick={close}>Cancel</Button>
          <Button variant="primary" disabled={!count} loading={busy} onClick={() => void (async () => {
            const ok = await onAdd(dialog.folderId, dialog.documentIds);
            if (ok) close();
          })()}>{count > 1 ? `Add ${count} documents` : "Add document"}</Button>
        </>}>
        {onUploadInto ? (
          <Button className="tw:w-full tw:justify-center" disabled={busy}
            onClick={() => { const folderId = dialog.folderId; close(); onUploadInto(folderId); }}>
            <Icon name="upload" size={16} />
            Upload a new document
          </Button>
        ) : null}
        {choices.length ? (
          <fieldset className="tw:m-0 tw:grid tw:min-w-0 tw:gap-1 tw:border-0 tw:p-0">
            <legend className="tw:mb-2 tw:p-0 tw:text-small tw:font-medium tw:text-muted-foreground">
              {onUploadInto ? "Or choose from your documents" : "Choose from your documents"}
            </legend>
            {choices.map((document) => {
              const from = membershipOf.get(document.id);
              return (
                <label key={document.id} className="tw:flex tw:min-h-11 tw:min-w-0 tw:cursor-pointer tw:items-center tw:gap-3 tw:rounded-md tw:px-2 tw:hover:bg-muted/50">
                  <Checkbox checked={selected.has(document.id)} disabled={busy}
                    onCheckedChange={(checked) => setDialog({
                      ...dialog,
                      documentIds: checked
                        ? [...dialog.documentIds, document.id]
                        : dialog.documentIds.filter((id) => id !== document.id),
                    })} />
                  <span className="tw:grid tw:min-w-0">
                    <span className="tw:truncate tw:text-small tw:font-medium">{document.title}</span>
                    <span className="tw:truncate tw:text-caption tw:text-muted-foreground">
                      {formatDocumentType(document.sourceFormat)}
                      {from ? ` · moves from ${collectionName(from.collectionId) ?? "another collection"}` : ""}
                    </span>
                  </span>
                </label>
              );
            })}
          </fieldset>
        ) : null}
        {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      </Dialog>
    );
  }

  if (dialog.kind === "move-document") {
    const current = organization.memberships.find((entry) => entry.documentId === dialog.documentId)?.collectionId;
    const choices = organization.collections.filter((folder) => folder.id !== current);
    const title = documentById.get(dialog.documentId)?.title;
    return (
      <Dialog title={current ? "Move to another collection" : "Move to a collection"} description={title}
        onClose={close} busy={busy} width="small"
        actions={<>
          <Button onClick={close}>Cancel</Button>
          <Button variant="primary" disabled={!dialog.targetCollectionId} loading={busy} onClick={() => void (async () => {
            const ok = await onMove(dialog.documentId, dialog.targetCollectionId);
            if (ok) close();
          })()}>Move</Button>
        </>}>
        <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-small tw:font-medium">
          Collection
          <NativeSelect value={dialog.targetCollectionId} onChange={(event) => setDialog({ ...dialog, targetCollectionId: event.target.value })}>
            <option value="">Choose a collection…</option>
            {choices.map((folder) => <option key={folder.id} value={folder.id}>{folder.name}</option>)}
          </NativeSelect>
        </label>
        {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      </Dialog>
    );
  }

  const document = documentById.get(dialog.membership.documentId);
  if (!document) return null;
  const unitLabel = unitPlural(document.sourceUnitKind);
  return (
    <Dialog title="Reading range"
      description={`${capitalize(unitLabel)} 1 to ${document.sourceUnitCount}`}
      onClose={close} busy={busy} width="small"
      actions={<>
        <Button onClick={close}>Cancel</Button>
        <Button loading={busy} onClick={() => setDialog({ ...dialog, value: "" })}>Whole document</Button>
        <Button variant="primary" loading={busy} onClick={() => void (async () => {
          const parsed = parseRanges(dialog.value, document);
          if (typeof parsed === "string") { setError(parsed); return; }
          const ok = await onRanges(dialog.membership, parsed);
          if (ok) close();
        })()}>Save range</Button>
      </>}>
      <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-small tw:font-medium">
        {capitalize(unitLabel)}
        <TextField autoFocus value={dialog.value} placeholder={document.sourceUnitCount > 1 ? "1-3, 7" : "1"}
          onChange={(event) => { setDialog({ ...dialog, value: event.target.value }); setError(null); }} />
      </label>
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
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
  return value ? value.charAt(0).toUpperCase() + value.slice(1) : value;
}

