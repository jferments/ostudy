import type { ScheduleAction, ScheduleActionResult } from "../../ipc";

interface ScheduleActionNotice {
  kind: "success" | "info";
  text: string;
}

/** Builds learner-facing copy from the authoritative transactional batch result. */
export function scheduleActionNotice(result: ScheduleActionResult): ScheduleActionNotice {
  const fragments: string[] = [];
  if (result.changedCount > 0) {
    fragments.push(changedMessage(result.action, result.changedCount));
  }
  if (result.unchangedCount > 0) {
    fragments.push(unchangedMessage(result.action, result.unchangedCount));
  }
  if (result.rejectedCount > 0) {
    fragments.push(
      `${result.rejectedCount} ${plural(result.rejectedCount, "card was", "cards were")} rejected.`,
    );
  }
  if (fragments.length === 0) {
    fragments.push("No cards required a schedule change.");
  }
  return {
    kind: result.changedCount > 0 ? "success" : "info",
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
