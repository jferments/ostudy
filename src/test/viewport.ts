import { viewportBreakpoints, viewportQueries } from "../ui/tokens/generated/viewport";

export type ViewportLayout = "compact" | "medium" | "wide";

/** A representative viewport width inside each layout band. */
export const layoutWidths: Readonly<Record<ViewportLayout, number>> = { compact: 390, medium: 900, wide: 1400 };

/** Evaluates the app's generated layout media queries for a simulated viewport width. */
export function matchesLayoutQuery(query: string, width: number): boolean {
  if (query === viewportQueries.compact) return width < viewportBreakpoints.medium;
  if (query === viewportQueries.belowWide) return width < viewportBreakpoints.wide;
  return false;
}
