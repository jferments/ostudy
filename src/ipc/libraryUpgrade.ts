import { isTauri } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";

export interface LibraryUpgradeProgress {
  sequence: number;
  message: string;
  completed: boolean;
  /** How far the whole update is, in thousandths, once its size is known. */
  permille?: number | null;
}

function isProgress(value: unknown): value is LibraryUpgradeProgress {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<LibraryUpgradeProgress>;
  return typeof candidate.sequence === "number"
    && Number.isSafeInteger(candidate.sequence) && candidate.sequence > 0
    && typeof candidate.message === "string" && candidate.message.length <= 512
    && typeof candidate.completed === "boolean"
    && (candidate.permille === undefined || candidate.permille === null
      || (Number.isInteger(candidate.permille) && candidate.permille >= 0 && candidate.permille <= 1000));
}

/** Advisory only: database-backed readiness remains the startup gate's authority. */
export async function observeLibraryUpgrade(
  changed: (progress: LibraryUpgradeProgress) => void,
): Promise<() => void> {
  if (!isTauri()) return () => {};
  let active = true;
  let sequence = 0;
  const unsubscribe = await listen<unknown>("ostudy-library-upgrade-progress", ({ payload }) => {
    if (!active || !isProgress(payload) || payload.sequence <= sequence) return;
    sequence = payload.sequence;
    changed(payload);
  });
  // Subscribe before replay. A racing older snapshot cannot overwrite a newer
  // phase. A lost advisory request must neither stall startup nor leak cleanup.
  void emit("ostudy-library-upgrade-progress-request").catch(() => {});
  return () => {
    active = false;
    unsubscribe();
  };
}
