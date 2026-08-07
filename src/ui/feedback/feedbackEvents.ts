export type FeedbackEvent =
  | { type: "answer_revealed" }
  | { type: "review_committed"; score: 0 | 1 | 2 | 3 }
  | { type: "session_completed" }
  | { type: "daily_goal_reached" }
  | { type: "import_completed"; createdCount: number }
  | { type: "operation_failed" };
