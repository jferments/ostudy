import {
  open as openNativeDialog,
  save as saveNativeDialog,
} from "@tauri-apps/plugin-dialog";

export interface FileFilter {
  name: string;
  extensions: string[];
}

export interface OpenFileOptions {
  title?: string;
  multiple?: boolean;
  directory?: boolean;
  filters?: FileFilter[];
  defaultPath?: string;
}

export interface SaveFileOptions {
  title?: string;
  filters?: FileFilter[];
  defaultPath?: string;
}

/**
 * Platform-owned file selection boundary. Android may later stage content URIs
 * into application cache without changing feature controllers or screens.
 */
export async function openFile(
  options: OpenFileOptions,
): Promise<string | string[] | null> {
  return openNativeDialog(options);
}

export async function saveFile(options: SaveFileOptions): Promise<string | null> {
  return saveNativeDialog(options);
}

// Transitional names keep call sites compact while ownership moves out of
// feature code. They are platform adapters, not direct Tauri plugin exports.
export const open = openFile;
export const save = saveFile;
