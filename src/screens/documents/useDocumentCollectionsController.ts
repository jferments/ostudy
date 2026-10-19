import { useCallback, useEffect, useRef, useState } from "react";

import { useRequestLane } from "../../controllers/requestLane";
import {
  addDocumentCollectionMembership,
  createDocumentCollection,
  deleteDocumentCollection,
  getDocumentCollectionOrganization,
  moveDocumentCollectionMembership,
  removeDocumentCollectionMembership,
  setDocumentCollectionRanges,
  updateDocumentCollection,
  type DocumentCollection,
  type DocumentCollectionMembership,
  type DocumentCollectionOrganization,
  type SourceRangeInput,
} from "../../ipc/collections";
import { createOperationId, getErrorMessage } from "../../ipc/common";

export function useDocumentCollectionsController() {
  const [organization, setOrganization] = useState<DocumentCollectionOrganization | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reads = useRequestLane();
  const receipts = useRef(new Map<string, string>());

  const load = useCallback(async () => {
    const token = reads.begin();
    setLoading(true);
    try {
      const value = await getDocumentCollectionOrganization();
      if (!reads.isCurrent(token)) return;
      setOrganization(value);
      setError(null);
    } catch (failure: unknown) {
      if (reads.isCurrent(token)) setError(getErrorMessage(failure));
    } finally {
      if (reads.isCurrent(token)) setLoading(false);
    }
  }, [reads]);

  useEffect(() => { void load(); }, [load]);

  async function mutate(
    key: string,
    write: (operationId: string, expectedRevision: number) => Promise<DocumentCollectionOrganization>,
  ): Promise<boolean> {
    if (busy || !organization) return false;
    reads.invalidate();
    const operationId = receipts.current.get(key) ?? createOperationId();
    receipts.current.set(key, operationId);
    setBusy(true);
    setError(null);
    try {
      const next = await write(operationId, organization.revision);
      receipts.current.delete(key);
      setOrganization(next);
      return true;
    } catch (failure: unknown) {
      setError(getErrorMessage(failure));
      return false;
    } finally {
      setBusy(false);
    }
  }

  function createFolder(name: string): Promise<boolean> {
    const normalized = name.trim();
    return mutate(
      JSON.stringify(["create", normalized]),
      (operationId, expectedOrganizationRevision) => createDocumentCollection({
        operationId, name: normalized, expectedOrganizationRevision,
      }),
    );
  }

  function updateFolder(
    folder: DocumentCollection,
    patch: { name?: string },
  ): Promise<boolean> {
    const name = patch.name?.trim() ?? folder.name;
    return mutate(
      JSON.stringify(["update", folder.id, folder.revision, name]),
      (operationId, expectedOrganizationRevision) => updateDocumentCollection({
        operationId,
        collectionId: folder.id,
        name,
        revision: folder.revision,
        expectedOrganizationRevision,
      }),
    );
  }

  function deleteFolder(folder: DocumentCollection): Promise<boolean> {
    return mutate(
      JSON.stringify(["delete", folder.id, folder.revision]),
      (operationId, expectedOrganizationRevision) => deleteDocumentCollection({
        operationId,
        collectionId: folder.id,
        revision: folder.revision,
        expectedOrganizationRevision,
      }),
    );
  }

  function addDocument(folderId: string, documentId: string): Promise<boolean> {
    if (organization?.memberships.some(
      (membership) => membership.collectionId === folderId && membership.documentId === documentId,
    )) {
      return Promise.resolve(true);
    }
    return mutate(
      JSON.stringify(["add", folderId, documentId]),
      (operationId, expectedOrganizationRevision) => addDocumentCollectionMembership({
        operationId, collectionId: folderId, documentId, expectedOrganizationRevision,
      }),
    );
  }

  /** Adds several documents in turn, keeping each one that lands if a later one fails. */
  async function addDocuments(folderId: string, documentIds: readonly string[]): Promise<boolean> {
    if (busy || !organization) return false;
    const pending = documentIds.filter((documentId) => !organization.memberships.some(
      (membership) => membership.collectionId === folderId && membership.documentId === documentId,
    ));
    if (!pending.length) return true;
    reads.invalidate();
    setBusy(true);
    setError(null);
    let revision = organization.revision;
    try {
      for (const documentId of pending) {
        const key = JSON.stringify(["add", folderId, documentId]);
        const operationId = receipts.current.get(key) ?? createOperationId();
        receipts.current.set(key, operationId);
        const next = await addDocumentCollectionMembership({
          operationId, collectionId: folderId, documentId, expectedOrganizationRevision: revision,
        });
        receipts.current.delete(key);
        revision = next.revision;
        setOrganization(next);
      }
      return true;
    } catch (failure: unknown) {
      setError(getErrorMessage(failure));
      return false;
    } finally {
      setBusy(false);
    }
  }

  function moveDocument(
    membership: DocumentCollectionMembership,
    targetCollectionId: string,
  ): Promise<boolean> {
    return mutate(
      JSON.stringify(["move", membership.id, membership.revision, targetCollectionId]),
      (operationId, expectedOrganizationRevision) => moveDocumentCollectionMembership({
        operationId,
        membershipId: membership.id,
        targetCollectionId,
        revision: membership.revision,
        expectedOrganizationRevision,
      }),
    );
  }

  function removeDocument(membership: DocumentCollectionMembership): Promise<boolean> {
    return mutate(
      JSON.stringify(["remove", membership.id, membership.revision]),
      (operationId, expectedOrganizationRevision) => removeDocumentCollectionMembership({
        operationId,
        membershipId: membership.id,
        revision: membership.revision,
        expectedOrganizationRevision,
      }),
    );
  }

  function setRanges(
    membership: DocumentCollectionMembership,
    ranges: SourceRangeInput[],
  ): Promise<boolean> {
    return mutate(
      JSON.stringify(["ranges", membership.id, membership.revision, ranges]),
      (operationId, expectedOrganizationRevision) => setDocumentCollectionRanges({
        operationId,
        membershipId: membership.id,
        ranges,
        revision: membership.revision,
        expectedOrganizationRevision,
      }),
    );
  }

  return {
    organization, loading, busy, error, load,
    createFolder, updateFolder, deleteFolder, addDocument, addDocuments, moveDocument, removeDocument, setRanges,
  };
}

export type DocumentCollectionsController = ReturnType<typeof useDocumentCollectionsController>;
