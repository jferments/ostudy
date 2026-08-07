export type RichDocument = Record<string, unknown>;

export interface DeletionResult {
  affectedCount: number;
  undoToken: string;
  undoExpiresAt: number;
  subjectLabel: string;
}

export interface UndoDeletionResult {
  restoredCount: number;
  entityKind: "deck" | "cards";
  subjectLabel: string;
}

export interface CommandErrorPayload {
  code?: string;
  message?: string;
}

export function createOperationId(): string {
  return crypto.randomUUID().replaceAll("-", "");
}

export function createSubmissionId(): string {
  return crypto.randomUUID().replaceAll("-", "");
}

export function createMutationId(): string {
  return crypto.randomUUID().replaceAll("-", "");
}

export function getErrorCode(error: unknown): string | null {
  if (typeof error === "object" && error !== null) {
    const payload = error as CommandErrorPayload;
    return typeof payload.code === "string" ? payload.code : null;
  }
  return null;
}

export function getErrorMessage(error: unknown): string {
  if (typeof error === "string" && error.trim() !== "") return error;
  if (typeof error === "object" && error !== null) {
    const payload = error as CommandErrorPayload;
    if (typeof payload.message === "string" && payload.message.trim() !== "") {
      return payload.message;
    }
  }
  if (error instanceof Error && error.message.trim() !== "") return error.message;
  return "An unexpected error occurred.";
}
