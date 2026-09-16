import type { ImageMask, NormalizedRect } from "../../ipc/questions";
import { sameRectangle, validateRectangle } from "./geometry";

export interface RegionBindings { id: string; maskIds: string[]; }
export type GeometryCommand =
  | { kind: "insert"; mask: ImageMask; index: number; restoreQuestionIds: string[] }
  | { kind: "remove"; maskId: string; expected: NormalizedRect }
  | { kind: "change"; maskId: string; before: NormalizedRect; after: NormalizedRect };
export interface GeometryState<T extends RegionBindings> { masks: ImageMask[]; questions: T[]; }
export interface GeometryHistory { undo: GeometryCommand[]; redo: GeometryCommand[]; }
export interface GeometryTransition<T extends RegionBindings> {
  state: GeometryState<T>;
  history: GeometryHistory;
  applied: boolean;
}
export const EMPTY_GEOMETRY_HISTORY: GeometryHistory = { undo: [], redo: [] };
const HISTORY_LIMIT = 100;

function apply<T extends RegionBindings>(state: GeometryState<T>, command: GeometryCommand): { state: GeometryState<T>; inverse: GeometryCommand } | null {
  if (command.kind === "insert") {
    validateRectangle(command.mask.rect);
    if (state.masks.length >= 128 || state.masks.some((mask) => mask.id === command.mask.id)) return null;
    const masks = [...state.masks];
    masks.splice(Math.min(masks.length, Math.max(0, command.index)), 0, { ...command.mask, rect: { ...command.mask.rect } });
    const questions = state.questions.map((question) => command.restoreQuestionIds.includes(question.id)
      ? { ...question, maskIds: [...new Set([...question.maskIds, command.mask.id])] } : question);
    return { state: { masks, questions }, inverse: { kind: "remove", maskId: command.mask.id, expected: command.mask.rect } };
  }
  const index = state.masks.findIndex((mask) => mask.id === command.maskId);
  if (index < 0) return null;
  const mask = state.masks[index];
  const expected = command.kind === "remove" ? command.expected : command.before;
  if (!sameRectangle(mask.rect, expected)) return null;
  if (command.kind === "change") {
    validateRectangle(command.after);
    if (sameRectangle(command.before, command.after)) return null;
    const masks = state.masks.map((mask) => mask.id === command.maskId ? { ...mask, rect: { ...command.after } } : mask);
    return { state: { masks, questions: state.questions }, inverse: {
      kind: "change", maskId: command.maskId, before: command.after, after: command.before,
    } };
  }
  const restoreQuestionIds = state.questions.filter((question) => question.maskIds.includes(mask.id)).map((question) => question.id);
  return {
    state: {
      masks: state.masks.filter((candidate) => candidate.id !== mask.id),
      // Keep the question and its rich answer as an incomplete draft rather than deleting it.
      questions: state.questions.map((question) => question.maskIds.includes(mask.id)
        ? { ...question, maskIds: question.maskIds.filter((id) => id !== mask.id) } : question),
    },
    inverse: { kind: "insert", mask, index, restoreQuestionIds },
  };
}

export function commitGeometry<T extends RegionBindings>(state: GeometryState<T>, history: GeometryHistory, command: GeometryCommand): GeometryTransition<T> {
  const result = apply(state, command);
  if (!result) return { state, history, applied: false };
  return { state: result.state, history: { undo: [...history.undo, result.inverse].slice(-HISTORY_LIMIT), redo: [] }, applied: true };
}

export function stepGeometryHistory<T extends RegionBindings>(state: GeometryState<T>, history: GeometryHistory, direction: "undo" | "redo"): GeometryTransition<T> {
  const source = history[direction];
  const command = source.at(-1);
  if (!command) return { state, history, applied: false };
  const result = apply(state, command);
  // A conflicting external geometry edit invalidates this history, not the current draft.
  if (!result) return { state, history: EMPTY_GEOMETRY_HISTORY, applied: false };
  const opposite = direction === "undo" ? "redo" : "undo";
  return { state: result.state, history: {
    ...history, [direction]: source.slice(0, -1), [opposite]: [...history[opposite], result.inverse].slice(-HISTORY_LIMIT),
  }, applied: true };
}
