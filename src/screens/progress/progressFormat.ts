import type { StatDeltaProps } from "../../ui/primitives/StatTile";

/** Display helpers shared by the Progress sections. Dates are `YYYY-MM-DD` study dates. */

export function formatPercent(value: number | null): string {
  return value === null ? "None yet" : `${Math.round(value * 100)}%`;
}

export function formatStudyTime(ms: number): string {
  if (ms < 60_000) return ms === 0 ? "0 min" : "Under 1 min";
  if (ms < 3_600_000) return `${Math.round(ms / 60_000)} min`;
  const hours = ms / 3_600_000;
  return `${Number(hours.toFixed(hours < 10 ? 1 : 0))} hr`;
}

export function formatResponseTime(ms: number): string {
  if (ms < 1_000) return `${Math.round(ms)} ms`;
  return `${(ms / 1_000).toFixed(ms < 10_000 ? 1 : 0)} s`;
}

export function plural(count: number, one: string, many = `${one}s`): string {
  return `${count.toLocaleString()} ${count === 1 ? one : many}`;
}

/**
 * The change between two rates in whole percentage points, as a delta pill.
 * A higher rate is good news. Undefined when either side has no data.
 */
export function pointsDelta(current: number | null, previous: number | null): StatDeltaProps | undefined {
  if (current === null || previous === null) return undefined;
  return pointsDeltaFrom(current - previous);
}

export function pointsDeltaFrom(change: number | null): StatDeltaProps | undefined {
  if (change === null) return undefined;
  const points = Math.round(change * 100);
  if (points === 0) return { label: "No change", tone: "neutral" };
  return {
    label: `${Math.abs(points)} ${Math.abs(points) === 1 ? "pt" : "pts"}`,
    direction: points > 0 ? "up" : "down",
    tone: points > 0 ? "positive" : "negative",
  };
}

/** A study date as a UTC instant at noon, so formatting never crosses a date line. */
export function localDate(value: string): Date {
  return new Date(`${value}T12:00:00Z`);
}

export function shiftLocalDate(value: string, days: number): string {
  const date = localDate(value);
  date.setUTCDate(date.getUTCDate() + days);
  return date.toISOString().slice(0, 10);
}

export function daysBetween(start: string, end: string): number {
  return Math.round((localDate(end).getTime() - localDate(start).getTime()) / 86_400_000);
}

const dayFormatter = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", timeZone: "UTC" });
const longDayFormatter = new Intl.DateTimeFormat(undefined, { weekday: "short", month: "short", day: "numeric", timeZone: "UTC" });
const longDateFormatter = new Intl.DateTimeFormat(undefined, { month: "long", day: "numeric", timeZone: "UTC" });
const monthFormatter = new Intl.DateTimeFormat(undefined, { month: "short", year: "numeric", timeZone: "UTC" });
const shortMonthFormatter = new Intl.DateTimeFormat(undefined, { month: "short", timeZone: "UTC" });
const weekdayFormatter = new Intl.DateTimeFormat(undefined, { weekday: "long", timeZone: "UTC" });
const narrowWeekdayFormatter = new Intl.DateTimeFormat(undefined, { weekday: "narrow", timeZone: "UTC" });
const dateTimeFormatter = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });
const dateFormatter = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });

export const formatDay = (value: string) => dayFormatter.format(localDate(value));
export const formatLongDay = (value: string) => longDayFormatter.format(localDate(value));
export const formatLongDate = (value: string) => longDateFormatter.format(localDate(value));
export const formatMonth = (value: string) => monthFormatter.format(localDate(value));
export const formatShortMonth = (value: string) => shortMonthFormatter.format(localDate(value));
export const formatWeekday = (value: string) => weekdayFormatter.format(localDate(value));
export const formatNarrowWeekday = (value: string) => narrowWeekdayFormatter.format(localDate(value));
export const formatDateTime = (value: number) => dateTimeFormatter.format(new Date(value));
export const formatDate = (value: number) => dateFormatter.format(new Date(value));

/** Monday is 0, as the activity endpoint counts weekdays. */
export function weekdayName(mondayIndex: number): string {
  // 2024-01-01 was a Monday.
  return formatWeekday(shiftLocalDate("2024-01-01", mondayIndex));
}

/** Monday is 0. */
export function mondayIndex(value: string): number {
  return (localDate(value).getUTCDay() + 6) % 7;
}

/** "Sep 5 to today", or "Sep 5 to Oct 4" for a range that ends earlier. */
export function formatSpan(start: string | null, end: string, today: string): string {
  const last = end === today ? "today" : formatDay(end);
  return start === null ? `Through ${last}` : `${formatDay(start)} to ${last}`;
}
