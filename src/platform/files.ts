import {
  open as openNativeDialog,
  save as saveNativeDialog,
} from "@tauri-apps/plugin-dialog";
import { openPath as openNativePath } from "@tauri-apps/plugin-opener";

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

/**
 * Open one app-controlled local path with the platform's default application.
 * Feature screens never import Tauri plugins directly.
 */
export async function openLocalPath(path: string): Promise<void> {
  await openNativePath(path);
}
