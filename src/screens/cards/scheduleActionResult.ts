import {
  type ScheduleAction,
  type ScheduleActionResult,
} from "../../ipc/scheduling";

interface ScheduleActionNotice {
  kind: "success" | "info";
  text: string;
}

/**
 * Builds learner-facing copy from the authoritative changed-card count.
 * Invalid selections fail atomically, so successful operations have no
 * rejected cards; unchanged cards are the requested set minus affectedCount.
 */
export function scheduleActionNotice(
  result: ScheduleActionResult,
  requestedCount: number,
): ScheduleActionNotice {
  const changedCount = Math.max(0, Math.min(requestedCount, result.affectedCount));
  const unchangedCount = Math.max(0, requestedCount - changedCount);
  const fragments: string[] = [];

  if (changedCount > 0) {
    fragments.push(changedMessage(result.action, changedCount));
  }
  if (unchangedCount > 0) {
    fragments.push(unchangedMessage(result.action, unchangedCount));
  }
  if (fragments.length === 0) {
    fragments.push("No cards required a schedule change.");
  }

  return {
    kind: changedCount > 0 ? "success" : "info",
    text: fragments.join(" "),
  };
}

function changedMessage(action: ScheduleAction, count: number): string {
  const cards = plural(count, "card", "cards");
  switch (action) {
    case "suspend":
      return `Suspended ${count} ${cards}.`;
    case "resume":
      return `Resumed ${count} ${cards}.`;
    case "bury":
      return `Buried ${count} ${cards} until tomorrow.`;
    case "unbury":
      return `Unburied ${count} ${cards}.`;
    case "reset":
      return `Reset ${count} ${cards} to New.`;
    case "reschedule":
      return `Rescheduled ${count} ${cards}.`;
  }
}

function unchangedMessage(action: ScheduleAction, count: number): string {
  const subject = `${count} ${plural(count, "card was", "cards were")}`;
  switch (action) {
    case "suspend":
      return `${subject} already suspended.`;
    case "resume":
      return `${subject} already active.`;
    case "bury":
      return `${subject} already buried through at least tomorrow.`;
    case "unbury":
      return `${subject} already unburied.`;
    case "reset":
      return `${subject} already New and due.`;
    case "reschedule":
      return `${subject} already set to that due date.`;
  }
}

function plural(count: number, singular: string, pluralValue: string): string {
  return count === 1 ? singular : pluralValue;
}
