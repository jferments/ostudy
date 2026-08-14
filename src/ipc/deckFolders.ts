import { invoke } from "@tauri-apps/api/core";

export interface DeckFolder {
  id: string;
  name: string;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface DeckPlacement {
  deckId: string;
  folderId: string | null;
  sortOrder: number;
}

export interface DeckLibraryOrganization {
  revision: number;
  folders: DeckFolder[];
  placements: DeckPlacement[];
}

export interface CreateDeckFolderInput {
  name: string;
  expectedOrganizationRevision: number;
}

export interface UpdateDeckFolderInput {
  folderId: string;
  name: string;
  revision: number;
  expectedOrganizationRevision: number;
}

export interface DeleteDeckFolderInput {
  folderId: string;
  revision: number;
  expectedOrganizationRevision: number;
}

export interface MoveDeckInLibraryInput {
  deckId: string;
  targetFolderId: string | null;
  targetIndex: number;
  expectedOrganizationRevision: number;
}

export interface MoveDeckFolderInput {
  folderId: string;
  targetIndex: number;
  expectedOrganizationRevision: number;
}

export async function getDeckLibraryOrganization(): Promise<DeckLibraryOrganization> {
  return invoke<DeckLibraryOrganization>("get_deck_library_organization");
}

export async function createDeckFolder(
  input: CreateDeckFolderInput,
): Promise<DeckLibraryOrganization> {
  return invoke<DeckLibraryOrganization>("create_deck_folder", { input });
}

export async function updateDeckFolder(
  input: UpdateDeckFolderInput,
): Promise<DeckLibraryOrganization> {
  return invoke<DeckLibraryOrganization>("update_deck_folder", { input });
}

export async function deleteDeckFolder(
  input: DeleteDeckFolderInput,
): Promise<DeckLibraryOrganization> {
  return invoke<DeckLibraryOrganization>("delete_deck_folder", { input });
}

export async function moveDeckInLibrary(
  input: MoveDeckInLibraryInput,
): Promise<DeckLibraryOrganization> {
  return invoke<DeckLibraryOrganization>("move_deck_in_library", { input });
}

export async function moveDeckFolder(
  input: MoveDeckFolderInput,
): Promise<DeckLibraryOrganization> {
  return invoke<DeckLibraryOrganization>("move_deck_folder", { input });
}
