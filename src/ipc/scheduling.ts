import { invoke } from "@tauri-apps/api/core";

export type ScheduleAction =
  | "suspend"
  | "resume"
  | "bury"
  | "unbury"
  | "reset"
  | "reschedule";

export type RescheduleIntent =
  | { kind: "due_now" }
  | { kind: "tomorrow" }
  | { kind: "calendar_days"; days: number }
  | { kind: "calendar_date"; date: string };

export interface ScheduleActionInput {
  mutationId: string;
  cardIds: string[];
  action: ScheduleAction;
  rescheduleIntent?: RescheduleIntent;
  sessionId?: string;
}

export interface ScheduleActionResult {
  affectedCount: number;
  requestedCount: number;
  changedCount: number;
  unchangedCount: number;
  rejectedCount: number;
  action: ScheduleAction;
  resolvedDueAt: number | null;
}

export async function applyScheduleAction(input: ScheduleActionInput): Promise<ScheduleActionResult> {
  return invoke<ScheduleActionResult>("apply_schedule_action", { input });
}
