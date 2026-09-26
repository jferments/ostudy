/** Vite's exact @tauri-apps/api/core alias preserves the SDK surface while all
 * existing typed IPC gateways acquire the same native library generation.
 * The private raw alias resolves to the installed, pinned SDK, not this module.
 */
export * from "@ostudy/tauri-core";
import { invoke as nativeInvoke } from "@ostudy/tauri-core";
import { createLibraryTransport } from "./libraryTransport";

export const invoke = createLibraryTransport(nativeInvoke);
