import type { MapLayout, MapTopic } from "../../../ipc/documentAnnotations";
import { applyEdits, type AnnotationEdit, type PageAnnotations } from "./annotationHistory";
import { createAnnotationId, TOPIC_BORDER, TOPIC_PADDING_X, type Annotation, type Point, type Size, type TextShape } from "./annotationModel";

/**
 * Mind maps on a notebook page. A topic is a text note with a `topic`; the
 * central topic has no parent and names the map. The map places its topics:
 * every change to a map is followed by laying it out again from where its
 * central topic stands, so its branches stay tidy, and the branch lines are
 * drawn from the tree rather than stored.
 */

export type Topic = Annotation & { shape: TextShape & { topic: MapTopic } };

export function isTopic(annotation: Annotation | null | undefined): annotation is Topic {
  return annotation?.shape.kind === "text" && annotation.shape.topic !== undefined;
}

/** Space between a topic and the topics branching from it, and between neighbouring branches, in points. */
const LEVEL_GAP = 56;
const SIBLING_GAP = 14;
const DOWN_LEVEL_GAP = 48;
const DOWN_SIBLING_GAP = 20;
/** A dropped topic that lands on another becomes one of its branches. */
const DROP_SLOP = 4;

/** Topic widths and text sizes, by depth: the central topic, first-level branches, the rest. */
const TOPIC_WIDTHS = [220, 180, 160] as const;
const MIN_TOPIC_WIDTH = 56;
const TOPIC_FONT_SIZES = [20, 16, 14] as const;
export const CENTRAL_TOPIC_COLOR = "#334155";
/** First-level branches take these in turn; deeper topics take their branch's. */
export const BRANCH_COLORS = ["#2563eb", "#dc2626", "#16a34a", "#ea580c", "#7c3aed", "#0891b2"] as const;

/** One map on a page: its topics by id, and each topic's branches in order. */
export interface MapTree {
  id: string;
  root: Topic | null;
  topics: ReadonlyMap<string, Topic>;
  children: ReadonlyMap<string, readonly Topic[]>;
}

function byOrder(a: Topic, b: Topic): number {
  return a.shape.topic.order - b.shape.topic.order || a.createdAt - b.createdAt || (a.id < b.id ? -1 : 1);
}

/** The maps on a page. A topic whose parent is gone, or not in its map, hangs on nothing and stays where it is. */
export function mapsOn(annotations: readonly Annotation[]): Map<string, MapTree> {
  const maps = new Map<string, { root: Topic | null; topics: Map<string, Topic>; children: Map<string, Topic[]> }>();
  for (const annotation of annotations) {
    if (!isTopic(annotation)) continue;
    const id = annotation.shape.topic.map;
    let map = maps.get(id);
    if (!map) {
      map = { root: null, topics: new Map(), children: new Map() };
      maps.set(id, map);
    }
    map.topics.set(annotation.id, annotation);
    if (annotation.shape.topic.parent === undefined && annotation.id === id) map.root = annotation;
  }
  const trees = new Map<string, MapTree>();
  for (const [id, map] of maps) {
    for (const topic of map.topics.values()) {
      const parent = topic.shape.topic.parent;
      if (parent === undefined || !map.topics.has(parent)) continue;
      const list = map.children.get(parent) ?? [];
      list.push(topic);
      map.children.set(parent, list);
    }
    for (const list of map.children.values()) list.sort(byOrder);
    trees.set(id, { id, ...map });
  }
  return trees;
}

/** A topic and everything under it, reached from its map's tree (a loop is cut where it closes). */
export function subtree(tree: MapTree, id: string): Topic[] {
  const found: Topic[] = [];
  const seen = new Set<string>();
  const visit = (topicId: string) => {
    if (seen.has(topicId)) return;
    seen.add(topicId);
    const topic = tree.topics.get(topicId);
    if (topic) found.push(topic);
    for (const child of tree.children.get(topicId) ?? []) visit(child.id);
  };
  visit(id);
  return found;
}

/** How deep a topic is: 0 for the central topic. */
export function depthOf(tree: MapTree, topic: Topic): number {
  let depth = 0;
  let current: Topic | undefined = topic;
  const seen = new Set<string>();
  while (current?.shape.topic.parent !== undefined && !seen.has(current.id)) {
    seen.add(current.id);
    current = tree.topics.get(current.shape.topic.parent);
    depth += 1;
  }
  return depth;
}

/** The topics folded away under folded topics, which are neither drawn nor picked. */
export function hiddenTopics(annotations: readonly Annotation[]): ReadonlySet<string> {
  if (!annotations.some((annotation) => isTopic(annotation) && annotation.shape.topic.folded)) return EMPTY_SET;
  const hidden = new Set<string>();
  for (const tree of mapsOn(annotations).values()) {
    for (const topic of tree.topics.values()) {
      if (!topic.shape.topic.folded) continue;
      for (const child of tree.children.get(topic.id) ?? []) {
        for (const under of subtree(tree, child.id)) hidden.add(under.id);
      }
    }
  }
  return hidden;
}
const EMPTY_SET: ReadonlySet<string> = new Set();

/** A page's marks without the topics folded away; the same array when nothing is folded. */
export function visibleMarks(annotations: readonly Annotation[]): readonly Annotation[] {
  const hidden = hiddenTopics(annotations);
  return hidden.size === 0 ? annotations : annotations.filter((annotation) => !hidden.has(annotation.id));
}

/** How many topics a folded topic holds out of sight. */
export function foldedCount(tree: MapTree, topic: Topic): number {
  return topic.shape.topic.folded ? subtree(tree, topic.id).length - 1 : 0;
}

type Direction = "left" | "right" | "down";

/** Where each first-level branch goes in a map spread both ways: as chosen, the rest evening out the sides, right first. */
function sides(tree: MapTree, root: Topic): Map<string, "left" | "right"> {
  const first = tree.children.get(root.id) ?? [];
  const result = new Map<string, "left" | "right">();
  // Branches with no side chosen fill the right first, then the left, as a clock's hands go.
  const rightShare = Math.ceil(first.length / 2) - first.filter((topic) => topic.shape.topic.side === "right").length;
  let index = 0;
  for (const topic of first) {
    const side = topic.shape.topic.side ?? (index++ < rightShare ? "right" : "left");
    result.set(topic.id, side);
  }
  return result;
}

function layoutOf(tree: MapTree): MapLayout {
  return tree.root?.shape.topic.layout ?? "both";
}

/**
 * Where the map puts each of its topics, from where its central topic is:
 * branches to either side (or one side) stacked so no two overlap, or, laid
 * out downward, in rows like an outline turned on its side. Folded branches
 * take no room.
 */
export function layoutMap(tree: MapTree, height: (topic: Topic) => number): Map<string, Point> {
  const placed = new Map<string, Point>();
  const root = tree.root;
  if (!root) return placed;
  const size = (topic: Topic): Size => ({ width: topic.shape.width, height: height(topic) });
  const shownChildren = (topic: Topic) => (topic.shape.topic.folded ? [] : tree.children.get(topic.id) ?? []);
  const visiting = new Set<string>();

  const span = new Map<string, number>();
  const spanOf = (topic: Topic, direction: Direction): number => {
    const known = span.get(topic.id);
    if (known !== undefined) return known;
    if (visiting.has(topic.id)) return 0;
    visiting.add(topic.id);
    const own = direction === "down" ? size(topic).width : size(topic).height;
    const children = shownChildren(topic);
    const gap = direction === "down" ? DOWN_SIBLING_GAP : SIBLING_GAP;
    const total = children.reduce((sum, child) => sum + spanOf(child, direction), 0) + gap * Math.max(0, children.length - 1);
    const result = Math.max(own, total);
    span.set(topic.id, result);
    visiting.delete(topic.id);
    return result;
  };

  /** Places a topic with its near edge at `edge` (its top for downward maps) and its middle across at `middle`. */
  const place = (topic: Topic, edge: number, middle: number, direction: Direction) => {
    if (placed.has(topic.id)) return;
    const { width, height: tall } = size(topic);
    const point = direction === "down"
      ? { x: middle - width / 2, y: edge }
      : { x: direction === "right" ? edge : edge - width, y: middle - tall / 2 };
    placed.set(topic.id, point);
    placeChildren(topic, point, shownChildren(topic), direction);
  };

  const placeChildren = (topic: Topic, at: Point, children: readonly Topic[], direction: Direction) => {
    if (children.length === 0) return;
    const { width, height: tall } = size(topic);
    const gap = direction === "down" ? DOWN_SIBLING_GAP : SIBLING_GAP;
    const total = children.reduce((sum, child) => sum + spanOf(child, direction), 0) + gap * (children.length - 1);
    if (direction === "down") {
      let start = at.x + width / 2 - total / 2;
      for (const child of children) {
        const childSpan = spanOf(child, direction);
        place(child, at.y + tall + DOWN_LEVEL_GAP, start + childSpan / 2, direction);
        start += childSpan + gap;
      }
      return;
    }
    let start = at.y + tall / 2 - total / 2;
    const edge = direction === "right" ? at.x + width + LEVEL_GAP : at.x - LEVEL_GAP;
    for (const child of children) {
      const childSpan = spanOf(child, direction);
      place(child, edge, start + childSpan / 2, direction);
      start += childSpan + gap;
    }
  };

  const at = { x: root.shape.x, y: root.shape.y };
  placed.set(root.id, at);
  const layout = layoutOf(tree);
  const first = shownChildren(root);
  if (layout === "down") placeChildren(root, at, first, "down");
  else if (layout === "right") placeChildren(root, at, first, "right");
  else {
    const side = sides(tree, root);
    placeChildren(root, at, first.filter((topic) => side.get(topic.id) === "right"), "right");
    placeChildren(root, at, first.filter((topic) => side.get(topic.id) === "left"), "left");
  }
  return placed;
}

/** A branch line from a topic to one of its branches. */
export interface BranchLine { id: string; d: string; color: string; width: number; }

/** The lines from each shown topic to its shown branches, as smooth curves in page points. */
export function branchLines(annotations: readonly Annotation[], height: (annotation: Annotation) => number): BranchLine[] {
  const lines: BranchLine[] = [];
  for (const tree of mapsOn(annotations).values()) {
    const root = tree.root;
    if (!root) continue;
    const layout = layoutOf(tree);
    const side = layout === "both" ? sides(tree, root) : null;
    for (const [parentId, children] of tree.children) {
      const parent = tree.topics.get(parentId);
      if (!parent || parent.shape.topic.folded) continue;
      const parentHeight = height(parent);
      for (const child of children) {
        const childHeight = height(child);
        const color = child.shape.color;
        const width = parent.id === root.id ? 3 : 2;
        if (layout === "down") {
          const from = { x: parent.shape.x + parent.shape.width / 2, y: parent.shape.y + parentHeight };
          const to = { x: child.shape.x + child.shape.width / 2, y: child.shape.y };
          const mid = (from.y + to.y) / 2;
          lines.push({ id: child.id, color, width, d: `M ${from.x} ${from.y} C ${from.x} ${mid} ${to.x} ${mid} ${to.x} ${to.y}` });
          continue;
        }
        // A branch leaves its topic on the side it grows toward.
        const leftward = child.shape.x + child.shape.width / 2 < parent.shape.x + parent.shape.width / 2
          || (parent.id === root.id && side?.get(child.id) === "left");
        const from = { x: leftward ? parent.shape.x : parent.shape.x + parent.shape.width, y: parent.shape.y + parentHeight / 2 };
        const to = { x: leftward ? child.shape.x + child.shape.width : child.shape.x, y: child.shape.y + childHeight / 2 };
        const mid = (from.x + to.x) / 2;
        lines.push({ id: child.id, color, width, d: `M ${from.x} ${from.y} C ${mid} ${from.y} ${mid} ${to.y} ${to.x} ${to.y}` });
      }
    }
  }
  return lines;
}

/** The count shown on a folded topic, at the edge its branches would grow from. */
const BADGE_OFFSET = 10;

export interface FoldBadge { id: string; x: number; y: number; count: number; color: string; }

export function foldBadges(annotations: readonly Annotation[], height: (annotation: Annotation) => number): FoldBadge[] {
  if (!annotations.some((annotation) => isTopic(annotation) && annotation.shape.topic.folded)) return [];
  const badges: FoldBadge[] = [];
  for (const tree of mapsOn(annotations).values()) {
    const root = tree.root;
    const down = layoutOf(tree) === "down";
    for (const topic of tree.topics.values()) {
      const count = foldedCount(tree, topic);
      if (count === 0) continue;
      const tall = height(topic);
      const leftward = !down && root !== null && topic.id !== root.id
        && topic.shape.x + topic.shape.width / 2 < root.shape.x + root.shape.width / 2;
      badges.push({
        id: topic.id,
        count,
        color: topic.shape.color,
        // Just outside the edge its branches would leave from.
        x: down ? topic.shape.x + topic.shape.width / 2 : leftward ? topic.shape.x - BADGE_OFFSET : topic.shape.x + topic.shape.width + BADGE_OFFSET,
        y: down ? topic.shape.y + tall + BADGE_OFFSET : topic.shape.y + tall / 2,
      });
    }
  }
  return badges;
}

function sameTopic(a: MapTopic, b: MapTopic): boolean {
  return a.map === b.map && a.parent === b.parent && a.order === b.order && a.side === b.side
    && a.layout === b.layout && a.folded === b.folded;
}

function withTopic(annotation: Topic, topic: MapTopic): Topic {
  const clean: MapTopic = { map: topic.map, order: topic.order };
  if (topic.parent !== undefined) clean.parent = topic.parent;
  if (topic.side !== undefined) clean.side = topic.side;
  if (topic.layout !== undefined) clean.layout = topic.layout;
  if (topic.folded) clean.folded = true;
  return { ...annotation, shape: { ...annotation.shape, topic: clean } };
}

function moved(edit: AnnotationEdit): edit is AnnotationEdit & { before: Topic; after: Topic } {
  return isTopic(edit.before) && isTopic(edit.after) && edit.before.pageNumber === edit.after.pageNumber
    && (edit.before.shape.x !== edit.after.shape.x || edit.before.shape.y !== edit.after.shape.y)
    && sameTopic(edit.before.shape.topic, edit.after.shape.topic);
}

function centerOf(topic: Topic, height: (annotation: Annotation) => number): Point {
  return { x: topic.shape.x + topic.shape.width / 2, y: topic.shape.y + height(topic) / 2 };
}

/**
 * The edits a change to mind maps really makes: deleting a topic deletes its
 * branches; dropping a topic on another makes it one of that topic's
 * branches, and dropping it elsewhere moves it among its neighbours (or to
 * the other side); then every map the change touched is laid out again from
 * its central topic. Edits to anything else pass through unchanged, and the
 * result is still one step to undo.
 */
export function arrangeMaps(
  edits: readonly AnnotationEdit[],
  pages: PageAnnotations,
  height: (annotation: Annotation) => number,
  pageSize: (pageNumber: number) => Size | null,
): AnnotationEdit[] {
  if (!edits.some((edit) => isTopic(edit.before) || isTopic(edit.after))) return [...edits];
  const result = new Map<string, AnnotationEdit>();
  for (const edit of edits) result.set(edit.id, edit);
  let state = applyEdits(pages, edits);
  const touched = new Map<number, Set<string>>();
  const touch = (topic: Topic) => {
    const maps = touched.get(topic.pageNumber) ?? new Set<string>();
    maps.add(topic.shape.topic.map);
    touched.set(topic.pageNumber, maps);
  };
  const change = (before: Annotation, after: Annotation | null) => {
    const earlier = result.get(before.id);
    result.set(before.id, { id: before.id, before: earlier ? earlier.before : before, after });
    state = applyEdits(state, [{ id: before.id, before, after }]);
  };
  const current = (id: string, pageNumber: number) => state.get(pageNumber)?.find((annotation) => annotation.id === id) ?? null;

  // A deleted topic takes its branches with it.
  for (const edit of edits) {
    if (!isTopic(edit.before) || edit.after) continue;
    touch(edit.before);
    const before = pages.get(edit.before.pageNumber) ?? [];
    const tree = mapsOn(before).get(edit.before.shape.topic.map);
    if (!tree) continue;
    for (const under of subtree(tree, edit.before.id)) {
      const still = current(under.id, under.pageNumber);
      if (still) change(still, null);
    }
  }

  for (const edit of edits) {
    if (isTopic(edit.before)) touch(edit.before);
    if (isTopic(edit.after)) touch(edit.after);
  }

  // A topic given another color gives it to its branches that had its old one.
  for (const edit of edits) {
    if (!isTopic(edit.before) || !isTopic(edit.after) || edit.before.shape.color === edit.after.shape.color) continue;
    const tree = mapsOn(pages.get(edit.before.pageNumber) ?? []).get(edit.before.shape.topic.map);
    if (!tree) continue;
    for (const under of subtree(tree, edit.before.id)) {
      if (under.id === edit.before.id || under.shape.color !== edit.before.shape.color) continue;
      const latest = current(under.id, under.pageNumber);
      if (isTopic(latest)) change(latest, { ...latest, shape: { ...latest.shape, color: edit.after.shape.color } });
    }
  }

  // A new topic takes its place among its parent's branches; the later ones make room.
  for (const edit of edits) {
    if (edit.before || !isTopic(edit.after) || edit.after.shape.topic.parent === undefined) continue;
    const { map, parent, order } = edit.after.shape.topic;
    const tree = mapsOn(state.get(edit.after.pageNumber) ?? []).get(map);
    for (const sibling of tree?.children.get(parent) ?? []) {
      if (sibling.id === edit.after.id || sibling.shape.topic.order < order) continue;
      change(sibling, withTopic(sibling, { ...sibling.shape.topic, order: sibling.shape.topic.order + 1 }));
    }
  }

  // A topic dragged on its own finds its new place in the tree; one moved with its central topic only follows it.
  const movedRoots = new Set(edits.filter((edit) => moved(edit) && edit.after.shape.topic.parent === undefined)
    .map((edit) => edit.after?.shape.kind === "text" ? edit.after.shape.topic?.map : undefined));
  for (const edit of edits) {
    if (!moved(edit)) continue;
    const topic = edit.after.shape.topic;
    if (topic.parent === undefined || movedRoots.has(topic.map)) continue;
    const page = state.get(edit.after.pageNumber) ?? [];
    const trees = mapsOn(page);
    const tree = trees.get(topic.map);
    const self = tree?.topics.get(edit.after.id);
    if (!tree || !self) continue;
    const own = new Set(subtree(tree, self.id).map((under) => under.id));
    const center = centerOf(self, height);
    const target = page.filter(isTopic).reverse().find((candidate) => (
      !own.has(candidate.id)
      && center.x >= candidate.shape.x - DROP_SLOP && center.x <= candidate.shape.x + candidate.shape.width + DROP_SLOP
      && center.y >= candidate.shape.y - DROP_SLOP && center.y <= candidate.shape.y + height(candidate) + DROP_SLOP
    ));
    if (target) {
      // Dropped on a topic: it becomes that topic's last branch, and its own branches come along.
      const targetTree = trees.get(target.shape.topic.map);
      const siblings = targetTree?.children.get(target.id) ?? [];
      const order = siblings.reduce((most, sibling) => Math.max(most, sibling.shape.topic.order + 1), 0);
      for (const under of subtree(tree, self.id)) {
        const next: MapTopic = under.id === self.id
          ? { map: target.shape.topic.map, parent: target.id, order }
          : { ...under.shape.topic, map: target.shape.topic.map };
        change(under, withTopic(under, next));
      }
      touch(target);
      continue;
    }
    // Dropped elsewhere: it takes its place among its neighbours by where it was let go.
    const parent = tree.topics.get(topic.parent);
    if (!parent) continue;
    const layout = layoutOf(tree);
    const atRoot = parent.id === tree.root?.id;
    const side: "left" | "right" | undefined = layout === "both" && atRoot
      ? (center.x < centerOf(parent, height).x ? "left" : "right")
      : topic.side;
    const neighbours = (tree.children.get(parent.id) ?? []).filter((sibling) => (
      sibling.id === self.id || layout !== "both" || !atRoot || (sides(tree, tree.root ?? parent).get(sibling.id) ?? "right") === side
    ));
    const along = (sibling: Topic) => {
      const point = centerOf(sibling, height);
      return layout === "down" ? point.x : point.y;
    };
    const ordered = [...neighbours].sort((a, b) => along(a) - along(b));
    // Orders are kept apart from the other side's, so each side keeps its own run.
    const others = (tree.children.get(parent.id) ?? []).filter((sibling) => !neighbours.includes(sibling));
    const all = [...ordered, ...others];
    all.forEach((sibling, order) => {
      const latest = current(sibling.id, sibling.pageNumber);
      if (!isTopic(latest)) return;
      const next: MapTopic = { ...latest.shape.topic, order, ...(sibling.id === self.id && side ? { side } : {}) };
      if (!sameTopic(next, latest.shape.topic)) change(latest, withTopic(latest, next));
    });
  }

  // Each map the change touched is laid out again.
  for (const [pageNumber, mapIds] of touched) {
    const size = pageSize(pageNumber);
    for (const mapId of mapIds) {
      // Each topic is as wide as its words, up to its level's width.
      const unfitted = mapsOn(state.get(pageNumber) ?? []).get(mapId);
      for (const topic of unfitted?.topics.values() ?? []) {
        const width = fittedWidth(topic, depthOf(unfitted as MapTree, topic));
        if (width !== topic.shape.width) change(topic, { ...topic, shape: { ...topic.shape, width } });
      }
      const tree = mapsOn(state.get(pageNumber) ?? []).get(mapId);
      if (!tree) continue;
      const placed = layoutMap(tree, height);
      for (const [id, point] of placed) {
        const topic = tree.topics.get(id);
        if (!topic) continue;
        const x = round(size ? clamp(point.x, 0, Math.max(0, size.width - topic.shape.width)) : point.x);
        const y = round(size ? clamp(point.y, 0, Math.max(0, size.height - height(topic))) : point.y);
        if (x === topic.shape.x && y === topic.shape.y) continue;
        change(topic, { ...topic, shape: { ...topic.shape, x, y } });
      }
    }
  }
  return [...result.values()].filter((edit) => edit.before !== edit.after);
}

function clamp(value: number, low: number, high: number): number {
  return Math.min(high, Math.max(low, value));
}

function round(value: number): number {
  return Math.round(value * 100) / 100;
}

/** The look of a topic at a depth: its width and text size. */
/** About how wide a topic's words are, at most its level's width (scaled with its type). */
export function fittedWidth(topic: Topic, depth: number): number {
  const look = topicLook(depth);
  const widest = look.width * topic.shape.fontSize / look.fontSize;
  const longest = topic.shape.text.split("\n").reduce((most, line) => Math.max(most, line.length), 0);
  const words = longest * topic.shape.fontSize * 0.58 + 2 * (TOPIC_PADDING_X + TOPIC_BORDER) + 4;
  return round(clamp(words, Math.min(widest, MIN_TOPIC_WIDTH), widest));
}

export function topicLook(depth: number): { width: number; fontSize: number } {
  const index = Math.min(depth, TOPIC_WIDTHS.length - 1);
  return { width: TOPIC_WIDTHS[index] ?? 150, fontSize: TOPIC_FONT_SIZES[index] ?? 14 };
}

/** A new map's central topic, its middle at `point`, empty for typing. */
export function newCentralTopic(pageNumber: number, point: Point, page: Size, createdAt: number): Topic {
  const id = createAnnotationId();
  const { width, fontSize } = topicLook(0);
  const tall = fontSize * 1.3 + 16;
  return {
    id,
    pageNumber,
    createdAt,
    shape: {
      kind: "text",
      x: round(clamp(point.x - width / 2, 0, Math.max(0, page.width - width))),
      y: round(clamp(point.y - tall / 2, 0, Math.max(0, page.height - tall))),
      width,
      text: "",
      color: CENTRAL_TOPIC_COLOR,
      fontSize,
      topic: { map: id, order: 0 },
    },
  };
}

/**
 * A new empty topic under `parent` (after its last branch), or beside
 * `sibling` (right after it), placed near where the map will put it.
 */
export function newTopic(tree: MapTree, relative: { parent: Topic } | { after: Topic }, createdAt: number): Topic | null {
  const parent = "parent" in relative ? relative.parent : tree.topics.get(relative.after.shape.topic.parent ?? "");
  if (!parent) return null;
  const siblings = tree.children.get(parent.id) ?? [];
  const depth = depthOf(tree, parent) + 1;
  const { width, fontSize } = topicLook(depth);
  const color = depth === 1
    ? BRANCH_COLORS[siblings.length % BRANCH_COLORS.length] ?? BRANCH_COLORS[0]
    : parent.shape.color;
  let order = siblings.reduce((most, sibling) => Math.max(most, sibling.shape.topic.order + 1), 0);
  let side: "left" | "right" | undefined;
  if ("after" in relative) {
    order = relative.after.shape.topic.order + 1;
    side = relative.after.shape.topic.side;
  }
  if (parent.id === tree.root?.id && layoutOf(tree) === "both") {
    // A new first branch goes to the side with fewer, so a map grows both ways and the others never swap sides.
    const placed = [...sides(tree, parent).values()];
    side = placed.filter((one) => one === "right").length <= placed.filter((one) => one === "left").length ? "right" : "left";
  }
  return {
    id: createAnnotationId(),
    pageNumber: parent.pageNumber,
    createdAt,
    shape: {
      kind: "text",
      x: parent.shape.x + parent.shape.width + LEVEL_GAP,
      y: parent.shape.y,
      width,
      text: "",
      color,
      fontSize,
      topic: { map: tree.id, parent: parent.id, order, ...(side ? { side } : {}) },
    },
  };
}

/**
 * Makes room for a topic inserted at `order` among its parent's branches:
 * the topics from there on move one place later.
 */
export function makeRoom(tree: MapTree, parentId: string, order: number): AnnotationEdit[] {
  return (tree.children.get(parentId) ?? [])
    .filter((sibling) => sibling.shape.topic.order >= order)
    .map((sibling) => ({
      id: sibling.id,
      before: sibling,
      after: withTopic(sibling, { ...sibling.shape.topic, order: sibling.shape.topic.order + 1 }),
    }));
}

/** A topic changed in its place in the map: folded or not, laid out another way, and so on. */
export function retopic(topic: Topic, patch: Partial<MapTopic>): Topic {
  const next = { ...topic.shape.topic, ...patch };
  if (patch.folded === undefined && "folded" in patch) delete next.folded;
  return withTopic(topic, next);
}

/**
 * Moves a topic up a level: it becomes the branch after its parent, keeping
 * its own branches. Nothing happens to a central or first-level topic.
 */
export function promote(tree: MapTree, topic: Topic): AnnotationEdit[] {
  const parent = tree.topics.get(topic.shape.topic.parent ?? "");
  const grandparent = parent ? tree.topics.get(parent.shape.topic.parent ?? "") : undefined;
  if (!parent || !grandparent) return [];
  const order = parent.shape.topic.order + 1;
  const room = makeRoom(tree, grandparent.id, order);
  const next = withTopic(topic, { map: tree.id, parent: grandparent.id, order, ...(parent.shape.topic.side ? { side: parent.shape.topic.side } : {}) });
  return [...room, { id: topic.id, before: topic, after: next }];
}

/** The topic an arrow key goes to from `topic`, as the map is drawn. */
export function topicToward(tree: MapTree, topic: Topic, key: "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown"): Topic | null {
  const layout = layoutOf(tree);
  const parent = tree.topics.get(topic.shape.topic.parent ?? "");
  const children = topic.shape.topic.folded ? [] : tree.children.get(topic.id) ?? [];
  const siblings = parent ? tree.children.get(parent.id) ?? [] : [];
  const neighbour = (step: number) => {
    const visible = parent?.id === tree.root?.id && layout === "both"
      ? siblings.filter((sibling) => (sides(tree, parent as Topic).get(sibling.id)) === sides(tree, parent as Topic).get(topic.id))
      : siblings;
    const index = visible.findIndex((sibling) => sibling.id === topic.id);
    return visible[index + step] ?? null;
  };
  if (layout === "down") {
    if (key === "ArrowUp") return parent ?? null;
    if (key === "ArrowDown") return children[0] ?? null;
    return neighbour(key === "ArrowLeft" ? -1 : 1);
  }
  if (key === "ArrowUp" || key === "ArrowDown") return neighbour(key === "ArrowUp" ? -1 : 1);
  const leftSide = topic.shape.x + topic.shape.width / 2 < (tree.root ? tree.root.shape.x + tree.root.shape.width / 2 : 0);
  const outward = (key === "ArrowRight") !== leftSide;
  if (topic.id === tree.root?.id) {
    const side = sides(tree, topic);
    return children.find((child) => side.get(child.id) === (key === "ArrowRight" ? "right" : "left")) ?? null;
  }
  return outward ? children[0] ?? null : parent ?? null;
}

/** A map as an indented list, two spaces a level, folded branches included. */
export function mapAsList(tree: MapTree): string {
  const lines: string[] = [];
  const visit = (topic: Topic, depth: number, seen: Set<string>) => {
    if (seen.has(topic.id)) return;
    seen.add(topic.id);
    lines.push(`${"  ".repeat(depth)}- ${topic.shape.text.replace(/\s*\n\s*/g, " ")}`);
    for (const child of tree.children.get(topic.id) ?? []) visit(child, depth + 1, seen);
  };
  if (tree.root) visit(tree.root, 0, new Set());
  return lines.join("\n");
}
