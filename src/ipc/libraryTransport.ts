import type { InvokeArgs, InvokeOptions } from "@ostudy/tauri-core";

export type NativeInvoke = <T>(
  command: string,
  args?: InvokeArgs,
  options?: InvokeOptions,
) => Promise<T>;

/** One generation per loaded page. Never retry a stale durable mutation. */
export function createLibraryTransport(nativeInvoke: NativeInvoke): NativeInvoke {
  let generation: Promise<number> | null = null;
  return async function invoke<T>(
    command: string,
    args?: InvokeArgs,
    options?: InvokeOptions,
  ): Promise<T> {
    if (command.startsWith("plugin:") || command === "get_library_session") {
      return nativeInvoke<T>(command, args, options);
    }
    if (args !== undefined && (
      args === null || typeof args !== "object" || Array.isArray(args)
      || args instanceof ArrayBuffer || ArrayBuffer.isView(args)
    )) {
      throw { code: "invalid_library_request", message: "Library commands require typed object arguments." };
    }
    if (generation === null) {
      generation = nativeInvoke<{ generation: number; recoveryRequired: boolean }>("get_library_session")
        .then((session) => {
          if (session.recoveryRequired) {
            throw { code: "library_recovery_required", message: "Library recovery needs attention. Preserve the backup and recovery files and reopen oStudy." };
          }
          if (!Number.isSafeInteger(session.generation) || session.generation < 0) {
            throw new Error("The native library generation is invalid.");
          }
          return session.generation;
        });
      // A startup transport failure can be retried; a successful epoch is never
      // silently replaced in an old page after activation changes the library.
      void generation.catch(() => { generation = null; });
    }
    const originalGeneration = await generation;
    return nativeInvoke<T>(command, { ...args, libraryGeneration: originalGeneration }, options);
  };
}
