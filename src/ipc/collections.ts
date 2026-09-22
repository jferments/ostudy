import { invoke } from "@tauri-apps/api/core";

export interface DocumentCollection {
  id: string;
  name: string;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface DocumentCollectionMembership {
  id: string;
  collectionId: string;
  documentId: string;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface DocumentCollectionRange {
  id: string;
  membershipId: string;
  unitKind: string;
  startUnit: number;
  endUnit: number;
  sourceSha256: string;
  createdAt: number;
}

export interface DocumentCollectionOrganization {
  revision: number;
  collections: DocumentCollection[];
  memberships: DocumentCollectionMembership[];
  ranges: DocumentCollectionRange[];
}

export interface SourceRangeInput {
  unitKind: string;
  startUnit: number;
  endUnit: number;
}

export interface SelectedDocumentInput {
  documentId: string;
  ranges?: SourceRangeInput[];
}

export interface SourceSelectionInput {
  collectionIds?: string[];
  documents?: SelectedDocumentInput[];
  exclusions?: SelectedDocumentInput[];
}

export interface SourceSetMember {
  documentId: string;
  sourceSha256: string;
  documentRevision: number;
  sourceFormat: string;
  sourceUnitKind: string;
  sourceUnitCount: number;
  ranges: SourceRangeInput[];
  collectionReferences: Array<{
    collectionId: string;
    membershipId: string;
    membershipRevision: number;
  }>;
}

export interface SourceDisposition {
  documentId: string | null;
  collectionId: string | null;
  membershipId: string | null;
  reason: string;
}

export interface SourceSetSnapshot {
  snapshotId: string;
  organizationRevision: number;
  members: SourceSetMember[];
  dispositions: SourceDisposition[];
  expandedUnitCount: number;
  expandedTextBytes: number;
}

interface OrganizationMutationBase {
  operationId: string;
  expectedOrganizationRevision: number;
}

export interface CreateDocumentCollectionInput extends OrganizationMutationBase {
  name: string;
}

export interface UpdateDocumentCollectionInput extends OrganizationMutationBase {
  collectionId: string;
  name: string;
  revision: number;
}

export interface DeleteDocumentCollectionInput extends OrganizationMutationBase {
  collectionId: string;
  revision: number;
}

export interface AddDocumentCollectionMembershipInput extends OrganizationMutationBase {
  collectionId: string;
  documentId: string;
}

export interface MoveDocumentCollectionMembershipInput extends OrganizationMutationBase {
  membershipId: string;
  targetCollectionId: string;
  revision: number;
}

export interface RemoveDocumentCollectionMembershipInput extends OrganizationMutationBase {
  membershipId: string;
  revision: number;
}

export interface SetDocumentCollectionRangesInput extends OrganizationMutationBase {
  membershipId: string;
  ranges: SourceRangeInput[];
  revision: number;
}

export async function getDocumentCollectionOrganization(): Promise<DocumentCollectionOrganization> {
  return invoke<DocumentCollectionOrganization>("get_document_collection_organization");
}

export async function createDocumentCollection(
  input: CreateDocumentCollectionInput,
): Promise<DocumentCollectionOrganization> {
  return invoke<DocumentCollectionOrganization>("create_document_collection", { input });
}

export async function updateDocumentCollection(
  input: UpdateDocumentCollectionInput,
): Promise<DocumentCollectionOrganization> {
  return invoke<DocumentCollectionOrganization>("update_document_collection", { input });
}

export async function deleteDocumentCollection(
  input: DeleteDocumentCollectionInput,
): Promise<DocumentCollectionOrganization> {
  return invoke<DocumentCollectionOrganization>("delete_document_collection", { input });
}

export async function addDocumentCollectionMembership(
  input: AddDocumentCollectionMembershipInput,
): Promise<DocumentCollectionOrganization> {
  return invoke<DocumentCollectionOrganization>("add_document_collection_membership", { input });
}

export async function moveDocumentCollectionMembership(
  input: MoveDocumentCollectionMembershipInput,
): Promise<DocumentCollectionOrganization> {
  return invoke<DocumentCollectionOrganization>("move_document_collection_membership", { input });
}

export async function removeDocumentCollectionMembership(
  input: RemoveDocumentCollectionMembershipInput,
): Promise<DocumentCollectionOrganization> {
  return invoke<DocumentCollectionOrganization>("remove_document_collection_membership", { input });
}

export async function setDocumentCollectionRanges(
  input: SetDocumentCollectionRangesInput,
): Promise<DocumentCollectionOrganization> {
  return invoke<DocumentCollectionOrganization>("set_document_collection_ranges", { input });
}

export async function resolveDocumentSourceSelection(
  input: SourceSelectionInput,
): Promise<SourceSetSnapshot> {
  return invoke<SourceSetSnapshot>("resolve_document_source_selection", { input });
}
