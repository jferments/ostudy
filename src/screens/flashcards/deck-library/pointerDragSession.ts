import type {
  DragGeometryDestination,
  DragGeometryRow,
  DragGeometrySnapshot,
  DragRect,
} from "./dragSessionModel";

export const POINTER_ACTIVATION_DISTANCE = 4;
export const TOUCH_POINTER_ACTIVATION_DISTANCE = 8;
export const HOVER_EXPAND_DELAY_MS = 500;
export const EDGE_SCROLL_ZONE_PX = 56;
export const EDGE_SCROLL_MAX_PX = 18;
export const UNSORTED_DATA_VALUE = "__unsorted__";
export type LibraryPointerType = "mouse" | "pen" | "touch";

const SUPPORTED_POINTER_TYPES: readonly LibraryPointerType[] = ["mouse", "pen", "touch"];

const INTERACTIVE_SELECTOR = [
  "button",
  "a[href]",
  "input",
  "select",
  "textarea",
  "label",
  "summary",
  "iframe",
  "audio[controls]",
  "video[controls]",
  "[contenteditable]:not([contenteditable='false'])",
  "[tabindex]:not([tabindex='-1'])",
  "[role='button']",
  "[role='link']",
  "[role='menuitem']",
  "[data-library-no-pointer-drag]",
].join(",");

export interface PointerStartInput {
  button: number;
  isPrimary: boolean;
  pointerType: LibraryPointerType;
  target: EventTarget | null;
  currentTarget: HTMLElement;
}

export interface PointerSessionStart {
  pointerId: number;
  pointerType: LibraryPointerType;
  startX: number;
  startY: number;
  origin: HTMLElement;
  sourceName: string;
}

export function canStartPointerSession(input: PointerStartInput): boolean {
  if (
    input.button !== 0 ||
    !input.isPrimary ||
    !SUPPORTED_POINTER_TYPES.includes(input.pointerType)
  ) {
    return false;
  }
  if (input.currentTarget.closest<HTMLElement>("[data-archived='true']")) return false;
  if (
    !(input.target instanceof Element) ||
    !input.currentTarget.contains(input.target)
  ) {
    return false;
  }

  // The accessible deck-open surface is deliberately both a keyboard button
  // and the broad pointer drag surface. Permit that surface itself while still
  // rejecting every nested checkbox, button, link, menu, or explicit opt-out.
  const interactive = input.target.closest<HTMLElement>(INTERACTIVE_SELECTOR);
  return interactive === null || interactive === input.currentTarget;
}

export function activationDistance(pointerType: LibraryPointerType): number {
  return pointerType === "touch"
    ? TOUCH_POINTER_ACTIVATION_DISTANCE
    : POINTER_ACTIVATION_DISTANCE;
}

export function movementExceedsThreshold(
  startX: number,
  startY: number,
  currentX: number,
  currentY: number,
  threshold = POINTER_ACTIVATION_DISTANCE,
): boolean {
  return Math.hypot(currentX - startX, currentY - startY) >= threshold;
}

export function edgeScrollVelocity(
  pointerY: number,
  ownerRect: Pick<DragRect, "top" | "bottom">,
): number {
  if (pointerY < ownerRect.top + EDGE_SCROLL_ZONE_PX) {
    const pressure = Math.min(
      1,
      (ownerRect.top + EDGE_SCROLL_ZONE_PX - pointerY) / EDGE_SCROLL_ZONE_PX,
    );
    return -Math.ceil(pressure * EDGE_SCROLL_MAX_PX);
  }
  if (pointerY > ownerRect.bottom - EDGE_SCROLL_ZONE_PX) {
    const pressure = Math.min(
      1,
      (pointerY - (ownerRect.bottom - EDGE_SCROLL_ZONE_PX)) / EDGE_SCROLL_ZONE_PX,
    );
    return Math.ceil(pressure * EDGE_SCROLL_MAX_PX);
  }
  return 0;
}

export function captureDragGeometry(root: HTMLElement): {
  snapshot: DragGeometrySnapshot;
  elements: ReadonlyMap<string, HTMLElement>;
} {
  const elements = new Map<string, HTMLElement>();
  const rows: DragGeometryRow[] = [];
  const destinations: DragGeometryDestination[] = [];

  for (const element of root.querySelectorAll<HTMLElement>("[data-library-row]")) {
    if (element.dataset.dragSourceHidden === "true") continue;
    const kind = element.dataset.libraryRow;
    const id = element.dataset.itemId;
    const index = Number(element.dataset.itemIndex);
    const key = element.dataset.geometryKey;
    if ((kind !== "deck" && kind !== "folder") || !id || !key || !Number.isInteger(index)) {
      continue;
    }
    elements.set(key, element);
    rows.push({
      key,
      kind,
      id,
      folderId: decodeFolderId(element.dataset.folderId),
      index,
      name: element.dataset.itemName ?? "item",
      rect: copyRect(element.getBoundingClientRect()),
    });
  }

  for (const element of root.querySelectorAll<HTMLElement>("[data-library-folder-surface]")) {
    if (element.dataset.dragSourceHidden === "true") continue;
    const key = element.dataset.geometryKey;
    if (!key) continue;
    elements.set(key, element);
    destinations.push({
      key,
      folderId: decodeFolderId(element.dataset.folderId),
      name: element.dataset.destinationName ?? "Unsorted decks",
      collapsed: element.dataset.collapsed === "true",
      rect: copyRect(element.getBoundingClientRect()),
    });
  }

  return { snapshot: { rows, destinations }, elements };
}

export function findLibraryScrollOwner(root: HTMLElement): HTMLElement {
  return root.closest<HTMLElement>("main") ?? root;
}

function decodeFolderId(encoded: string | undefined): string | null {
  return !encoded || encoded === UNSORTED_DATA_VALUE ? null : encoded;
}

function copyRect(rect: DOMRect): DragRect {
  return {
    top: rect.top,
    right: rect.right,
    bottom: rect.bottom,
    left: rect.left,
  };
}
