import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  addDocumentCollectionMembership,
  createDocumentCollection,
  deleteDocumentCollection,
  getDocumentCollectionOrganization,
  moveDocumentCollectionMembership,
  removeDocumentCollectionMembership,
  resolveDocumentSourceSelection,
  setDocumentCollectionRanges,
  updateDocumentCollection,
} from "./collections";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const organization = { revision: 1, collections: [], memberships: [], ranges: [] };

describe("document collection IPC", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue(organization);
  });

  it("keeps collection mutation and scope resolution behind typed native commands", async () => {
    await getDocumentCollectionOrganization();
    expect(invoke).toHaveBeenLastCalledWith("get_document_collection_organization");

    const create = {
      operationId: "1".repeat(32),
      name: "Week 1",
      parentId: null,
      expectedOrganizationRevision: 1,
    };
    await createDocumentCollection(create);
    expect(invoke).toHaveBeenLastCalledWith("create_document_collection", { input: create });

    const update = {
      operationId: "2".repeat(32),
      collectionId: "a".repeat(32),
      name: "Week 1",
      parentId: "b".repeat(32),
      archived: false,
      revision: 1,
      expectedOrganizationRevision: 2,
    };
    await updateDocumentCollection(update);
    expect(invoke).toHaveBeenLastCalledWith("update_document_collection", { input: update });

    const add = {
      operationId: "3".repeat(32),
      collectionId: "a".repeat(32),
      documentId: "d".repeat(32),
      expectedOrganizationRevision: 3,
    };
    await addDocumentCollectionMembership(add);
    expect(invoke).toHaveBeenLastCalledWith("add_document_collection_membership", { input: add });

    const ranges = {
      operationId: "4".repeat(32),
      membershipId: "c".repeat(32),
      ranges: [{ unitKind: "page", startUnit: 4, endUnit: 9 }],
      revision: 1,
      expectedOrganizationRevision: 4,
    };
    await setDocumentCollectionRanges(ranges);
    expect(invoke).toHaveBeenLastCalledWith("set_document_collection_ranges", { input: ranges });

    const move = {
      operationId: "5".repeat(32),
      membershipId: "c".repeat(32),
      targetCollectionId: "b".repeat(32),
      revision: 2,
      expectedOrganizationRevision: 5,
    };
    await moveDocumentCollectionMembership(move);
    expect(invoke).toHaveBeenLastCalledWith("move_document_collection_membership", { input: move });

    const remove = {
      operationId: "6".repeat(32),
      membershipId: "c".repeat(32),
      revision: 3,
      expectedOrganizationRevision: 6,
    };
    await removeDocumentCollectionMembership(remove);
    expect(invoke).toHaveBeenLastCalledWith("remove_document_collection_membership", { input: remove });

    const del = {
      operationId: "7".repeat(32),
      collectionId: "a".repeat(32),
      revision: 2,
      expectedOrganizationRevision: 7,
    };
    await deleteDocumentCollection(del);
    expect(invoke).toHaveBeenLastCalledWith("delete_document_collection", { input: del });

    const selection = {
      collectionIds: ["a".repeat(32)],
      documents: [{ documentId: "d".repeat(32), ranges: [] }],
      exclusions: [],
    };
    await resolveDocumentSourceSelection(selection);
    expect(invoke).toHaveBeenLastCalledWith("resolve_document_source_selection", {
      input: selection,
    });
  });
});
