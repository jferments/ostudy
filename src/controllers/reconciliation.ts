import { getErrorMessage } from "../ipc/common";

export interface ReconciliationOutcome<T> {
  committed: T;
  reconciliationError: string | null;
}

/**
 * Runs optional post-commit reconciliation without turning durable success into
 * a failed mutation. Callers may update local state from `committed` first and
 * surface `reconciliationError` as a recoverable refresh notice.
 */
export async function reconcileCommitted<T>(
  committed: T,
  reconcile: () => Promise<void>,
): Promise<ReconciliationOutcome<T>> {
  try {
    await reconcile();
    return { committed, reconciliationError: null };
  } catch (error: unknown) {
    return { committed, reconciliationError: getErrorMessage(error) };
  }
}
