import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface CompletedLibraryRestore {
  operationId: string;
  deckCount: number;
  cardCount: number;
  documentCount: number;
  templateCount: number;
  reviewCount: number;
  assetCount: number;
  completed: boolean;
  restartRequired: boolean;
}

export interface LibrarySession {
  generation: number;
  ready: boolean;
  restoring: boolean;
  recoveryRequired: boolean;
  lastRestore: CompletedLibraryRestore | null;
}

export function getLibrarySession(): Promise<LibrarySession> {
  return invoke<LibrarySession>("get_library_session");
}

export async function observeLibraryRestore(
  completed: (result: CompletedLibraryRestore) => void,
  rolledBack: (message: string) => void,
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const cleanups: Array<() => void> = [];
  try {
    cleanups.push(await listen<CompletedLibraryRestore>("ostudy-library-restored", (event) => completed(event.payload)));
    cleanups.push(await listen<{ message: string }>("ostudy-library-restore-rolled-back", (event) => rolledBack(event.payload.message)));
    return () => cleanups.forEach((cleanup) => cleanup());
  } catch (error) {
    cleanups.forEach((cleanup) => cleanup());
    throw error;
  }
}

export async function getNativeLibrarySession(): Promise<LibrarySession | null> {
  return isTauri() ? getLibrarySession() : null;
}
