import { invoke } from "@tauri-apps/api/core";

export interface TutorMemory {
  conversationId: string;
  revision: number;
  historyEnabled: boolean;
  retrievalEnabled: boolean;
  startSequence: number;
  summary: string;
  summaryThroughSequence: number;
}

export type MemoryAction = "enable_history" | "disable_history" | "allow_retrieval" | "deny_retrieval" | "forget_earlier";
export interface ChangeTutorMemoryInput {
  operationId: string;
  conversationId: string;
  expectedRevision: number;
  action: MemoryAction;
}

/** Local reads/writes only. Allowing retrieval does not send or embed messages. */
export function getTutorMemory(conversationId: string): Promise<TutorMemory> {
  return invoke<TutorMemory>("get_tutor_memory", { conversationId });
}

/** Retain the whole input for an ambiguous-result retry; never allocate a new operation ID. */
export function changeTutorMemory(input: ChangeTutorMemoryInput): Promise<TutorMemory> {
  return invoke<TutorMemory>("change_tutor_memory", { input });
}
