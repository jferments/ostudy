import { invoke as nativeInvoke } from "@tauri-apps/api/core";

import { createLibraryTransport } from "./libraryTransport";

/**
 * Application-owned native command transport.
 *
 * Domain gateways import this invoke explicitly. Non-command SDK utilities
 * (events, plugin listeners, convertFileSrc, isTauri) remain direct Tauri imports
 * at their narrow platform owners.
 */
export const invoke = createLibraryTransport(nativeInvoke);
