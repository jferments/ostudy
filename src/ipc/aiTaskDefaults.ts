import { invoke } from "@tauri-apps/api/core";

import type { AiReasoningEffort, AiTaskDefaults } from "./aiProfiles";

export type AiTaskKind = "flashcard_generation" | "tutor" | "embeddings";

export interface AiTaskDefaultInput {
  operationId: string;
  taskKind: AiTaskKind;
  profileId: string | null;
  modelId: string | null;
  reasoningEffort: AiReasoningEffort | null;
}

/** The owner retains this input when a write acknowledgement is ambiguous. */
export function setAiTaskDefault(input: AiTaskDefaultInput): Promise<AiTaskDefaults> {
  return invoke<AiTaskDefaults>("set_ai_task_default", { input });
}
