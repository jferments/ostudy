import { useCallback, useEffect, useRef, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { createOperationId, getErrorMessage } from "../../ipc/common";
import { releaseStagedExternalImport, type StagedExternalFile } from "../../ipc/externalFiles";
import {
  amendImageSet,
  getImageSet,
  getImageSetPreview,
  importImageSet,
  inspectImageSet,
  prepareImageSet,
  setImageSetFilters,
  type ImageMemberDetails,
  type ImageRegionOverride,
  type ImageSelectionInspection,
  type ImageSetDetails,
  type ImageSetInclusionCrop,
  type ImageSetPreview,
} from "../../ipc/imageSets";
import { selectExternalImport } from "../../platform/files";
import { latestResult, useRequestLane } from "../../controllers/requestLane";
import {
  appendImageSetMembers,
  moveImageSetMember,
  removeImageSetMember,
} from "./imageSetDraft";

export interface ExistingImageDraft {
  id: string;
  originalFilename: string;
  quarterTurns: 0 | 1 | 2 | 3;
  inclusion: ImageSetInclusionCrop | null;
}

export interface PendingImageDraft {
  id: string;
  staged: StagedExternalFile;
  inspection: ImageSelectionInspection;
  quarterTurns: 0 | 1 | 2 | 3;
  inclusion: ImageSetInclusionCrop | null;
}

interface ControllerInput {
  documentId: string | null;
  showToast: ShowToast;
  onImported: (documentId: string) => void;
  onChanged: () => Promise<unknown>;
  onClose: () => void;
}

function turns(value: number): 0 | 1 | 2 | 3 {
  return (((value % 4) + 4) % 4) as 0 | 1 | 2 | 3;
}

function existingMember(member: ImageMemberDetails): ExistingImageDraft {
  return {
    id: member.id,
    originalFilename: member.originalFilename,
    quarterTurns: member.quarterTurns,
    inclusion: member.inclusion,
  };
}

export function useImageSetWorkspaceController({
  documentId,
  showToast,
  onImported,
  onChanged,
  onClose,
}: ControllerInput) {
  const [details, setDetails] = useState<ImageSetDetails | null>(null);
  const [members, setMembers] = useState<ExistingImageDraft[]>([]);
  const [pending, setPending] = useState<PendingImageDraft[]>([]);
  const [title, setTitle] = useState("Image study material");
  const [maximumCount, setMaximumCount] = useState<number | null>(null);
  const [selectedMemberId, setSelectedMemberId] = useState<string | null>(null);
  const [preview, setPreview] = useState<ImageSetPreview | null>(null);
  const [includeInterface, setIncludeInterface] = useState(false);
  const [regionActions, setRegionActions] = useState<Record<string, ImageRegionOverride | null>>({});
  const [isLoading, setIsLoading] = useState(documentId !== null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const detailsLane = useRequestLane();
  const previewLane = useRequestLane();
  const importOperation = useRef(createOperationId());
  const amendOperation = useRef(createOperationId());
  const filterOperation = useRef(createOperationId());

  const applyDetails = useCallback((value: ImageSetDetails) => {
    setDetails(value);
    setMembers(value.members.map(existingMember));
    setIncludeInterface(value.includeInterface);
    const nextActions: Record<string, ImageRegionOverride | null> = {};
    for (const member of value.members) {
      for (const region of member.processing?.regions ?? []) {
        nextActions[region.id] = region.overrideAction;
      }
    }
    setRegionActions(nextActions);
    setSelectedMemberId((current) =>
      current && value.members.some((member) => member.id === current)
        ? current
        : value.members[0]?.id ?? null,
    );
  }, []);

  const reloadDetails = useCallback(async (id: string) => {
    const value = await latestResult(detailsLane, () => getImageSet(id));
    if (value) applyDetails(value);
    return value;
  }, [applyDetails, detailsLane]);

  useEffect(() => {
    if (!documentId) {
      setDetails(null);
      setMembers([]);
      setSelectedMemberId(null);
      setPreview(null);
      setIsLoading(false);
      return;
    }
    setIsLoading(true);
    setErrorMessage(null);
    void reloadDetails(documentId)
      .catch((error: unknown) => setErrorMessage(getErrorMessage(error)))
      .finally(() => setIsLoading(false));
  }, [documentId, reloadDetails]);

  useEffect(() => {
    if (!documentId || !details || !selectedMemberId) {
      setPreview(null);
      return;
    }
    setPreview(null);
    void latestResult(previewLane, () =>
      getImageSetPreview({
        documentId,
        memberId: selectedMemberId,
        expectedRevision: details.revision,
        viewport: null,
      }),
    )
      .then((value) => {
        if (value) setPreview(value);
      })
      .catch((error: unknown) => setErrorMessage(getErrorMessage(error)));
  }, [details, documentId, previewLane, selectedMemberId]);

  async function addImage(): Promise<void> {
    if (busyAction) return;
    setErrorMessage(null);
    const selected = await selectExternalImport("image");
    if (!selected) return;
    try {
      const inspection = await inspectImageSet([selected.id]);
      setMaximumCount(inspection.maximumCount);
      const inspected = inspection.members[0];
      if (!inspected) throw new Error("Image inspection returned no selection result.");
      const draft: PendingImageDraft = {
        id: selected.id,
        staged: selected,
        inspection: inspected,
        quarterTurns: 0,
        inclusion: null,
      };
      setPending((current) => [
        ...appendImageSetMembers(current, [draft], inspection.maximumCount),
      ]);
      if (pending.length === 0 && documentId === null) {
        setTitle(selected.displayName.replace(/\.[^.]+$/u, "") || "Image study material");
      }
    } catch (error: unknown) {
      await releaseStagedExternalImport(selected.id).catch(() => undefined);
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    }
  }

  async function retryPending(id: string): Promise<void> {
    if (busyAction) return;
    try {
      const inspection = await inspectImageSet([id]);
      setMaximumCount(inspection.maximumCount);
      const inspected = inspection.members[0];
      if (!inspected) throw new Error("Image inspection returned no selection result.");
      setPending((current) =>
        current.map((member) =>
          member.id === id ? { ...member, inspection: inspected } : member,
        ),
      );
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    }
  }

  async function removePending(id: string): Promise<void> {
    const next = removeImageSetMember(pending, id);
    setPending([...next]);
    await releaseStagedExternalImport(id).catch(() => undefined);
  }

  function movePending(id: string, destination: number): void {
    setPending((current) => [...moveImageSetMember(current, id, destination)]);
  }

  function moveExisting(id: string, destination: number): void {
    setMembers((current) => [...moveImageSetMember(current, id, destination)]);
  }

  function rotatePending(id: string, delta: number): void {
    setPending((current) =>
      current.map((member) =>
        member.id === id
          ? { ...member, quarterTurns: turns(member.quarterTurns + delta) }
          : member,
      ),
    );
  }

  function rotateExisting(id: string, delta: number): void {
    setMembers((current) =>
      current.map((member) =>
        member.id === id
          ? { ...member, quarterTurns: turns(member.quarterTurns + delta) }
          : member,
      ),
    );
  }

  function setPendingInclusion(id: string, inclusion: ImageSetInclusionCrop | null): void {
    setPending((current) =>
      current.map((member) => (member.id === id ? { ...member, inclusion } : member)),
    );
  }

  function setExistingInclusion(id: string, inclusion: ImageSetInclusionCrop | null): void {
    setMembers((current) =>
      current.map((member) => (member.id === id ? { ...member, inclusion } : member)),
    );
  }

  async function reconcile(): Promise<void> {
    try {
      await onChanged();
    } catch (error: unknown) {
      setErrorMessage(`Saved successfully; refreshing Documents failed: ${getErrorMessage(error)}`);
    }
  }

  async function prepareCommitted(id: string, retryUnreadable = false): Promise<boolean> {
    try {
      await prepareImageSet(id, retryUnreadable);
      return true;
    } catch (error: unknown) {
      const message = `Source saved; local image preparation failed: ${getErrorMessage(error)}`;
      setErrorMessage(message);
      showToast("error", message);
      return false;
    }
  }

  async function importCurrent(): Promise<void> {
    if (busyAction || pending.length === 0) return;
    if (pending.some((member) => member.inspection.image === null)) {
      setErrorMessage("Remove or retry failed image selections before importing.");
      return;
    }
    const name = title.trim();
    if (!name) {
      setErrorMessage("Name this image set before importing it.");
      return;
    }
    setBusyAction("import");
    setErrorMessage(null);
    try {
      const result = await importImageSet({
        operationId: importOperation.current,
        title: name,
        members: pending.map((member) => ({
          stagedFileId: member.staged.id,
          quarterTurns: member.quarterTurns,
          inclusion: member.inclusion,
        })),
      });
      importOperation.current = createOperationId();
      setPending([]);
      onImported(result.documentId);
      const prepared = await prepareCommitted(result.documentId);
      await reconcile();
      if (prepared) {
        showToast("success", "Image set imported and prepared locally.");
      }
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    } finally {
      setBusyAction(null);
    }
  }

  async function saveSource(): Promise<void> {
    if (!documentId || !details || busyAction) return;
    if (pending.some((member) => member.inspection.image === null)) {
      setErrorMessage("Remove or retry failed appended images before saving.");
      return;
    }
    setBusyAction("source");
    setErrorMessage(null);
    try {
      await amendImageSet({
        operationId: amendOperation.current,
        documentId,
        expectedRevision: details.revision,
        expectedFilterRevision: details.filterRevision,
        members: members.map((member) => ({
          memberId: member.id,
          quarterTurns: member.quarterTurns,
          inclusion: member.inclusion,
        })),
        append: pending.map((member) => ({
          stagedFileId: member.staged.id,
          quarterTurns: member.quarterTurns,
          inclusion: member.inclusion,
        })),
      });
      amendOperation.current = createOperationId();
      setPending([]);
      const prepared = await prepareCommitted(documentId);
      await reloadDetails(documentId).catch((error: unknown) => {
        setErrorMessage(`Source saved; reopening the image set failed: ${getErrorMessage(error)}`);
      });
      await reconcile();
      if (prepared) showToast("success", "Image source changes saved.");
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    } finally {
      setBusyAction(null);
    }
  }

  async function saveFilters(): Promise<void> {
    if (!documentId || !details || busyAction) return;
    if (!details.prepared) {
      setErrorMessage("Prepare the current source before changing region filters.");
      return;
    }
    setBusyAction("filters");
    setErrorMessage(null);
    try {
      await setImageSetFilters({
        operationId: filterOperation.current,
        documentId,
        expectedRevision: details.revision,
        expectedFilterRevision: details.filterRevision,
        includeInterface,
        overrides: details.members.flatMap((member) =>
          (member.processing?.regions ?? []).map((region) => ({
            regionId: region.id,
            action: regionActions[region.id] ?? null,
          })),
        ),
      });
      filterOperation.current = createOperationId();
      const prepared = await prepareCommitted(documentId);
      await reloadDetails(documentId).catch((error: unknown) => {
        setErrorMessage(`Filters saved; reopening the image set failed: ${getErrorMessage(error)}`);
      });
      await reconcile();
      if (prepared) showToast("success", "Content filters saved and reapplied locally.");
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    } finally {
      setBusyAction(null);
    }
  }

  async function retryPreparation(): Promise<void> {
    if (!documentId || busyAction) return;
    setBusyAction("prepare");
    setErrorMessage(null);
    try {
      await prepareImageSet(documentId, true);
      await reloadDetails(documentId);
      await reconcile();
      showToast("success", "Local image preparation completed.");
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    } finally {
      setBusyAction(null);
    }
  }

  async function close(): Promise<void> {
    const leases = pending.map((member) => member.staged.id);
    setPending([]);
    await Promise.all(leases.map((id) => releaseStagedExternalImport(id).catch(() => undefined)));
    onClose();
  }

  const selectedMember =
    details?.members.find((member) => member.id === selectedMemberId) ?? null;

  return {
    details,
    members,
    pending,
    title,
    maximumCount,
    selectedMemberId,
    selectedMember,
    preview,
    includeInterface,
    regionActions,
    isLoading,
    busyAction,
    errorMessage,
    setTitle,
    setSelectedMemberId,
    setIncludeInterface,
    setRegionAction(regionId: string, action: ImageRegionOverride | null) {
      setRegionActions((current) => ({ ...current, [regionId]: action }));
    },
    addImage,
    retryPending,
    removePending,
    movePending,
    moveExisting,
    rotatePending,
    rotateExisting,
    setPendingInclusion,
    setExistingInclusion,
    importCurrent,
    saveSource,
    saveFilters,
    retryPreparation,
    close,
  };
}
