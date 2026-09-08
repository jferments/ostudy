import { listen, type UnlistenFn } from "@tauri-apps/api/event";

const OPERATION_PROGRESS_EVENT = "ostudy-operation-progress";

export interface OperationProgress {
  schemaVersion: number;
  operationId: string;
  operationKind: string;
  phase: string;
  processed: number | null;
  total: number | null;
  succeeded: number;
  failed: number;
}

export async function listenToOperationProgress(
  operationId: string,
  listener: (progress: OperationProgress) => void,
): Promise<UnlistenFn> {
  return listen<OperationProgress>(OPERATION_PROGRESS_EVENT, (event) => {
    if (event.payload.schemaVersion === 1 && event.payload.operationId === operationId) {
      listener(event.payload);
    }
  });
}
