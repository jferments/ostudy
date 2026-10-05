import type { Subject } from "../../ipc/subjects";

// The same colors and initials as Home's subject cards. When Home's model lands
// on main, this file gives way to its shared helpers.

/** The theme's six subject colors as complete utilities. */
export const SUBJECT_BADGE_COLORS = [
  "tw:bg-subject-1",
  "tw:bg-subject-2",
  "tw:bg-subject-3",
  "tw:bg-subject-4",
  "tw:bg-subject-5",
  "tw:bg-subject-6",
] as const;

/** Subjects take the colors in creation order, so moving or sorting one never repaints it. */
export function subjectBadgeColors(subjects: readonly Subject[]): Map<string, string> {
  const byCreation = [...subjects].sort(
    (left, right) => left.createdAt - right.createdAt || (left.id < right.id ? -1 : left.id > right.id ? 1 : 0),
  );
  return new Map(byCreation.map((subject, index) => [subject.id, SUBJECT_BADGE_COLORS[index % SUBJECT_BADGE_COLORS.length]!]));
}

const MINOR_WORDS = new Set(["a", "an", "and", "at", "by", "for", "from", "in", "into", "of", "on", "or", "the", "to", "with"]);

/** "Spanish for clinicians" is SC; a one-word name gives its first two letters. */
export function subjectInitials(name: string): string {
  const words = name
    .split(/\s+/u)
    .map((word) => Array.from(word.replace(/[^\p{L}\p{N}]/gu, "")))
    .filter((letters) => letters.length > 0);
  const significant = words.filter((letters) => !MINOR_WORDS.has(letters.join("").toLowerCase()));
  const [first, second] = significant.length > 0 ? significant : words;
  if (!first) return "?";
  return (second ? `${first[0]}${second[0]}` : first.slice(0, 2).join("")).toLocaleUpperCase();
}
