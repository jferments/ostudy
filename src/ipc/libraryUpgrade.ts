import { isTauri } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";

export interface LibraryUpgradeProgress {
  sequence: number;
  message: string;
  completed: boolean;
}

function isProgress(value: unknown): value is LibraryUpgradeProgress {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<LibraryUpgradeProgress>;
  return typeof candidate.sequence === "number"
    && Number.isSafeInteger(candidate.sequence) && candidate.sequence > 0
    && typeof candidate.message === "string" && candidate.message.length <= 512
    && typeof candidate.completed === "boolean";
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
