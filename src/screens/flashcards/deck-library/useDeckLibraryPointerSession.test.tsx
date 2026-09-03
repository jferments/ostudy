import { act, renderHook } from "@testing-library/react";
import type { PointerEvent as ReactPointerEvent, RefObject } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { DeckSummary } from "../../../ipc/decks";
import type { DeckFolder, DeckLibraryOrganization } from "../../../ipc/deckFolders";
import * as uiPerformanceInstrumentation from "../../../testing/uiPerformanceInstrumentation";
import { createDeckLibrarySnapshot } from "./organizationModel";
import { useDeckLibraryPointerSession } from "./useDeckLibraryPointerSession";

const folder = makeFolder("f", "Nursing", 1_000);
const deckA = deck("a", "Anatomy");
const deckB = deck("b", "Biology");
const organization: DeckLibraryOrganization = {
  revision: 7,
  folders: [folder],
  placements: [
    { deckId: deckA.id, folderId: folder.id, sortOrder: 1_000 },
    { deckId: deckB.id, folderId: null, sortOrder: 1_000 },
  ],
};
const snapshot = createDeckLibrarySnapshot([deckA, deckB], organization);

describe("useDeckLibraryPointerSession", () => {
  beforeEach(() => {
    vi.spyOn(window, "requestAnimationFrame").mockImplementation(() => 41);
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    document.body.replaceChildren();
  });

  it.each(["mouse", "pen", "touch"] as const)(
    "moves an exact full-row clone for %s and commits one validated intent",
    (pointerType) => {
      const frames: FrameRequestCallback[] = [];
      vi.mocked(window.requestAnimationFrame).mockImplementation((callback) => {
        frames.push(callback);
        return frames.length;
      });
      const harness = libraryHarness();
      const onMove = vi.fn();
      const { result } = renderHook(() =>
        useDeckLibraryPointerSession(
          harness.ref,
          snapshot,
          false,
          "stable",
          onMove,
          vi.fn(),
        ),
      );

      act(() => {
        result.current.beginPointerSession(
          reactPointer(harness.origin, harness.originArea, {
            pointerType,
            clientX: 40,
            clientY: 530,
          }),
          { kind: "deck", id: deckB.id },
          deckB.name,
        );
        window.dispatchEvent(pointerEvent("pointermove", 60, 100));
      });

      expect(harness.root.setPointerCapture).toHaveBeenCalledWith(9);
      expect(harness.sourceRow.style.display).toBe("none");
      expect(harness.root.dataset.libraryDragging).toBe("deck");
      const preview = document.querySelector<HTMLElement>("[data-drag-preview]");
      const placeholder = document.querySelector<HTMLElement>("[data-drag-placeholder]");
      expect(preview).not.toBeNull();
      expect(preview?.textContent).toContain("Biology");
      expect(preview?.querySelector("[data-drag-preview-clone]")).not.toBeNull();
      expect(preview?.style.width).toBe("660px");
      expect(preview?.style.height).toBe("70px");
      expect(preview?.style.transform).toContain("translate3d(40px, 70px");
      expect(placeholder?.style.width).toBe("660px");
      expect(placeholder?.style.height).toBe("70px");

      act(() => frames.shift()?.(0));
      expect(harness.folderSurface.contains(placeholder)).toBe(true);
      expect(placeholder?.nextSibling).toBe(harness.folderDeck);
      expect(result.current.feedback.candidate?.status).toBe("valid");

      act(() => window.dispatchEvent(pointerEvent("pointerup", 60, 100)));

      expect(onMove).toHaveBeenCalledTimes(1);
      expect(onMove).toHaveBeenCalledWith({
        kind: "deck",
        deckId: deckB.id,
        targetFolderId: folder.id,
        targetIndex: 0,
        source: "pointer",
      });
      expect(harness.root.releasePointerCapture).toHaveBeenCalledWith(9);
      expect(harness.sourceRow.style.display).toBe("");
      expect(harness.sourceRow.getAttribute("aria-hidden")).toBeNull();
      expect(harness.root.dataset.libraryDragging).toBeUndefined();
      expect(document.querySelector("[data-drag-preview]")).toBeNull();
      expect(document.querySelector("[data-drag-placeholder]")).toBeNull();
      expect(result.current.feedback.candidate?.announcement).toContain("Moved Biology");
    },
  );

  it("uses the stable library root for capture and still cancels when capture is unavailable", () => {
    const harness = libraryHarness();
    harness.root.setPointerCapture.mockImplementation(() => {
      throw new DOMException("Synthetic pointer is not active", "NotFoundError");
    });
    harness.root.hasPointerCapture.mockReturnValue(false);
    const onMove = vi.fn();
    const { result } = renderHook(() =>
      useDeckLibraryPointerSession(
        harness.ref,
        snapshot,
        false,
        "stable",
        onMove,
        vi.fn(),
      ),
    );

    act(() => {
      result.current.beginPointerSession(
        reactPointer(harness.origin, harness.originArea, { clientX: 20, clientY: 520 }),
        { kind: "deck", id: deckB.id },
        deckB.name,
      );
      window.dispatchEvent(pointerEvent("pointermove", 60, 100));
    });
    expect(result.current.feedback.active).toBe(true);
    expect(document.querySelector("[data-drag-preview]")).not.toBeNull();

    act(() => window.dispatchEvent(pointerEvent("pointercancel", 60, 100)));

    expect(result.current.feedback.active).toBe(false);
    expect(onMove).not.toHaveBeenCalled();
    expect(harness.root.releasePointerCapture).not.toHaveBeenCalled();
    expect(harness.sourceRow.style.display).toBe("");
    expect(document.querySelector("[data-drag-preview]")).toBeNull();
  });

  it("keeps three pixels of mouse motion as an ordinary click", () => {
    const harness = libraryHarness();
    const onMove = vi.fn();
    const clicked = vi.fn();
    harness.origin.addEventListener("click", clicked);
    const { result } = renderHook(() =>
      useDeckLibraryPointerSession(
        harness.ref,
        snapshot,
        false,
        "stable",
        onMove,
        vi.fn(),
      ),
    );

    act(() => {
      result.current.beginPointerSession(
        reactPointer(harness.origin, harness.originArea, { clientX: 20, clientY: 520 }),
        { kind: "deck", id: deckB.id },
        deckB.name,
      );
      window.dispatchEvent(pointerEvent("pointermove", 23, 520));
      window.dispatchEvent(pointerEvent("pointerup", 23, 520));
      harness.originArea.click();
    });

    expect(harness.root.setPointerCapture).not.toHaveBeenCalled();
    expect(onMove).not.toHaveBeenCalled();
    expect(clicked).toHaveBeenCalledTimes(1);
    expect(document.querySelector("[data-drag-preview]")).toBeNull();
  });

  it("suppresses only the compatibility click after an activated drag", () => {
    const harness = libraryHarness();
    const clicked = vi.fn();
    harness.origin.addEventListener("click", clicked);
    const unrelated = document.createElement("button");
    const unrelatedClicked = vi.fn();
    unrelated.addEventListener("click", unrelatedClicked);
    document.body.append(unrelated);
    const { result } = renderHook(() =>
      useDeckLibraryPointerSession(
        harness.ref,
        snapshot,
        false,
        "stable",
        vi.fn(),
        vi.fn(),
      ),
    );

    act(() => {
      result.current.beginPointerSession(
        reactPointer(harness.origin, harness.originArea, { clientX: 20, clientY: 520 }),
        { kind: "deck", id: deckB.id },
        deckB.name,
      );
      window.dispatchEvent(pointerEvent("pointermove", 60, 100));
      window.dispatchEvent(pointerEvent("pointerup", 60, 100));
    });

    unrelated.click();
    expect(unrelatedClicked).toHaveBeenCalledOnce();
    harness.originArea.click();
    expect(clicked).not.toHaveBeenCalled();
    harness.originArea.click();
    expect(clicked).toHaveBeenCalledOnce();
  });

  it("restores the complete DOM on Escape, stale revision, and lost root capture", () => {
    for (const ending of ["escape", "revision", "lostcapture"] as const) {
      const harness = libraryHarness();
      const onMove = vi.fn();
      const { result, rerender } = renderHook(
        ({ currentSnapshot }) =>
          useDeckLibraryPointerSession(
            harness.ref,
            currentSnapshot,
            false,
            "stable",
            onMove,
            vi.fn(),
          ),
        { initialProps: { currentSnapshot: snapshot } },
      );

      act(() => {
        result.current.beginPointerSession(
          reactPointer(harness.origin, harness.originArea, { clientX: 20, clientY: 520 }),
          { kind: "deck", id: deckB.id },
          deckB.name,
        );
        window.dispatchEvent(pointerEvent("pointermove", 60, 100));
      });
      expect(harness.sourceRow.style.display).toBe("none");

      act(() => {
        if (ending === "escape") {
          document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
        } else if (ending === "lostcapture") {
          harness.root.dispatchEvent(pointerEvent("lostpointercapture", 60, 100));
        } else {
          rerender({
            currentSnapshot: {
              ...snapshot,
              organization: { ...snapshot.organization, revision: 8 },
            },
          });
        }
      });

      expect(onMove).not.toHaveBeenCalled();
      expect(result.current.feedback.active).toBe(false);
      expect(harness.sourceRow.style.display).toBe("");
      expect(document.querySelector("[data-drag-preview]")).toBeNull();
      expect(document.querySelector("[data-drag-placeholder]")).toBeNull();
      expect(harness.root.releasePointerCapture).toHaveBeenCalledOnce();
      document.body.replaceChildren();
    }
  });

  it("times activation and animation-frame work without rerendering the source tree", () => {
    const frames: FrameRequestCallback[] = [];
    vi.mocked(window.requestAnimationFrame).mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    const beginWork = vi
      .spyOn(uiPerformanceInstrumentation, "beginUiPerformancePointerWork")
      .mockReturnValue(null);
    const endWork = vi
      .spyOn(uiPerformanceInstrumentation, "endUiPerformancePointerWork")
      .mockImplementation(() => undefined);
    const harness = libraryHarness();
    let renders = 0;
    const { result } = renderHook(() => {
      renders += 1;
      return useDeckLibraryPointerSession(
        harness.ref,
        snapshot,
        false,
        "stable",
        vi.fn(),
        vi.fn(),
      );
    });

    act(() => {
      result.current.beginPointerSession(
        reactPointer(harness.origin, harness.originArea, { clientX: 20, clientY: 520 }),
        { kind: "deck", id: deckB.id },
        deckB.name,
      );
      window.dispatchEvent(pointerEvent("pointermove", 60, 100));
    });
    expect(beginWork).toHaveBeenNthCalledWith(1, "deck-drag-activation");
    expect(beginWork.mock.invocationCallOrder[0] as number).toBeLessThan(
      harness.root.setPointerCapture.mock.invocationCallOrder[0] as number,
    );
    expect(endWork).toHaveBeenCalledOnce();

    const rendersBeforeFrame = renders;
    act(() => frames.shift()?.(0));
    expect(beginWork).toHaveBeenNthCalledWith(2, "deck-drag-frame");
    expect(endWork).toHaveBeenCalledTimes(2);
    expect(renders).toBeGreaterThanOrEqual(rendersBeforeFrame);
    act(() => result.current.cancel());
  });

  it("expands a collapsed folder after the bounded hover delay and scrolls the real owner", () => {
    vi.useFakeTimers();
    const frames: FrameRequestCallback[] = [];
    vi.mocked(window.requestAnimationFrame).mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    const harness = libraryHarness();
    harness.folderSurface.dataset.collapsed = "true";
    const onExpand = vi.fn();
    const { result } = renderHook(() =>
      useDeckLibraryPointerSession(
        harness.ref,
        snapshot,
        false,
        "collapsed",
        vi.fn(),
        onExpand,
      ),
    );

    act(() => {
      result.current.beginPointerSession(
        reactPointer(harness.origin, harness.originArea, { clientX: 20, clientY: 520 }),
        { kind: "deck", id: deckB.id },
        deckB.name,
      );
      window.dispatchEvent(pointerEvent("pointermove", 60, 100));
      frames.shift()?.(0);
      vi.advanceTimersByTime(500);
    });
    expect(onExpand).toHaveBeenCalledWith(folder.id);

    act(() => {
      window.dispatchEvent(pointerEvent("pointermove", 60, 695));
      frames.shift()?.(16);
    });
    expect(harness.main.scrollBy).toHaveBeenCalledWith({ top: 17, behavior: "auto" });
    act(() => result.current.cancel());
  });

  it("moves a full folder clone and reflows the other folder around its placeholder", () => {
    const frames: FrameRequestCallback[] = [];
    vi.mocked(window.requestAnimationFrame).mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    const harness = folderLibraryHarness();
    const onMove = vi.fn();
    const { result } = renderHook(() =>
      useDeckLibraryPointerSession(
        harness.ref,
        harness.snapshot,
        false,
        "folders",
        onMove,
        vi.fn(),
      ),
    );

    act(() => {
      result.current.beginPointerSession(
        reactPointer(harness.origin, harness.originArea, { clientX: 50, clientY: 80 }),
        { kind: "folder", id: harness.firstFolder.id },
        `${harness.firstFolder.name} folder`,
      );
      window.dispatchEvent(pointerEvent("pointermove", 50, 360));
      frames.shift()?.(0);
    });

    const preview = document.querySelector<HTMLElement>("[data-drag-preview]");
    const placeholder = document.querySelector<HTMLElement>("[data-drag-placeholder]");
    expect(preview?.textContent).toContain("Nursing");
    expect(harness.root.contains(placeholder)).toBe(true);
    expect(placeholder?.previousSibling).toBe(harness.secondRow);

    act(() => window.dispatchEvent(pointerEvent("pointerup", 50, 360)));
    expect(onMove).toHaveBeenCalledWith({
      kind: "folder",
      folderId: harness.firstFolder.id,
      targetIndex: 1,
      source: "pointer",
    });
    expect(document.querySelector("[data-drag-preview]")).toBeNull();
    expect(harness.firstRow.style.display).toBe("");
  });
});

function libraryHarness() {
  const main = document.createElement("main") as HTMLElement & {
    scrollBy: ReturnType<typeof vi.fn>;
  };
  const root = pointerCaptureElement("div");
  const folderSurface = document.createElement("section");
  const deckList = document.createElement("div");
  const folderDeck = document.createElement("article");
  const sourceRow = document.createElement("article");
  const origin = document.createElement("div");
  const originArea = document.createElement("span");
  const title = document.createElement("strong");
  const detail = document.createElement("small");

  main.append(root);
  root.append(folderSurface, sourceRow);
  folderSurface.append(deckList);
  deckList.append(folderDeck);
  sourceRow.append(origin);
  origin.append(title, detail, originArea);
  document.body.append(main);
  title.textContent = "Biology";
  detail.textContent = "1 card · 1 ready";
  origin.setAttribute("role", "button");
  origin.tabIndex = 0;
  origin.dataset.libraryPointerDragSource = "true";
  deckList.dataset.libraryDeckList = "true";

  Object.assign(folderSurface.dataset, {
    libraryRow: "folder",
    libraryFolderSurface: "true",
    geometryKey: `folder:${folder.id}`,
    itemId: folder.id,
    itemIndex: "0",
    itemName: folder.name,
    folderId: folder.id,
    destinationName: folder.name,
    collapsed: "false",
  });
  Object.assign(folderDeck.dataset, {
    libraryRow: "deck",
    geometryKey: `deck:${deckA.id}`,
    itemId: deckA.id,
    itemIndex: "0",
    itemName: deckA.name,
    folderId: folder.id,
  });
  Object.assign(sourceRow.dataset, {
    libraryRow: "deck",
    geometryKey: `deck:${deckB.id}`,
    itemId: deckB.id,
    itemIndex: "0",
    itemName: deckB.name,
    folderId: "__unsorted__",
  });
  rect(main, 0, 0, 700, 700);
  rect(root, 0, 0, 700, 700);
  rect(folderSurface, 0, 0, 700, 320);
  rect(deckList, 10, 60, 690, 300);
  rect(folderDeck, 20, 80, 680, 150);
  rect(sourceRow, 20, 500, 680, 570);
  rect(origin, 20, 500, 680, 570);
  Object.assign(main, { scrollBy: vi.fn() });
  return {
    main,
    root,
    folderSurface,
    folderDeck,
    sourceRow,
    origin,
    originArea,
    ref: { current: root } as RefObject<HTMLElement>,
  };
}

function folderLibraryHarness() {
  const firstFolder = makeFolder("f", "Nursing", 1_000);
  const secondFolder = makeFolder("e", "Physiology", 2_000);
  const folderOrganization: DeckLibraryOrganization = {
    revision: 3,
    folders: [firstFolder, secondFolder],
    placements: [],
  };
  const folderSnapshot = createDeckLibrarySnapshot([], folderOrganization);
  const main = document.createElement("main") as HTMLElement & {
    scrollBy: ReturnType<typeof vi.fn>;
  };
  const root = pointerCaptureElement("div");
  const firstRow = document.createElement("section");
  const secondRow = document.createElement("section");
  const origin = document.createElement("div");
  const originArea = document.createElement("span");
  const title = document.createElement("strong");
  title.textContent = "Nursing";
  origin.append(title, originArea);
  firstRow.append(origin);
  root.append(firstRow, secondRow);
  main.append(root);
  document.body.append(main);
  origin.dataset.libraryPointerDragSource = "true";
  Object.assign(firstRow.dataset, {
    libraryRow: "folder",
    libraryFolderSurface: "true",
    geometryKey: `folder:${firstFolder.id}`,
    itemId: firstFolder.id,
    itemIndex: "0",
    itemName: firstFolder.name,
    folderId: firstFolder.id,
    destinationName: firstFolder.name,
    collapsed: "false",
  });
  Object.assign(secondRow.dataset, {
    libraryRow: "folder",
    libraryFolderSurface: "true",
    geometryKey: `folder:${secondFolder.id}`,
    itemId: secondFolder.id,
    itemIndex: "1",
    itemName: secondFolder.name,
    folderId: secondFolder.id,
    destinationName: secondFolder.name,
    collapsed: "false",
  });
  rect(main, 0, 0, 700, 700);
  rect(root, 0, 0, 700, 700);
  rect(firstRow, 20, 40, 680, 180);
  rect(origin, 20, 40, 680, 180);
  rect(secondRow, 20, 220, 680, 380);
  Object.assign(main, { scrollBy: vi.fn() });
  return {
    main,
    root,
    firstRow,
    secondRow,
    firstFolder,
    origin,
    originArea,
    snapshot: folderSnapshot,
    ref: { current: root } as RefObject<HTMLElement>,
  };
}

function pointerCaptureElement(tag: "div") {
  const element = document.createElement(tag) as HTMLDivElement & {
    setPointerCapture: ReturnType<typeof vi.fn>;
    hasPointerCapture: ReturnType<typeof vi.fn>;
    releasePointerCapture: ReturnType<typeof vi.fn>;
  };
  Object.assign(element, {
    setPointerCapture: vi.fn(),
    hasPointerCapture: vi.fn().mockReturnValue(true),
    releasePointerCapture: vi.fn(),
  });
  return element;
}

function rect(element: HTMLElement, left: number, top: number, right: number, bottom: number) {
  vi.spyOn(element, "getBoundingClientRect").mockReturnValue({
    left,
    top,
    right,
    bottom,
    width: right - left,
    height: bottom - top,
    x: left,
    y: top,
    toJSON: () => ({}),
  });
}

function pointerEvent(
  type: string,
  clientX: number,
  clientY: number,
  pointerId = 9,
): PointerEvent {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperties(event, {
    pointerId: { value: pointerId },
    clientX: { value: clientX },
    clientY: { value: clientY },
  });
  return event as PointerEvent;
}

function reactPointer(
  currentTarget: HTMLElement,
  target: HTMLElement,
  overrides: Partial<ReactPointerEvent<HTMLElement>>,
): ReactPointerEvent<HTMLElement> {
  return {
    pointerId: 9,
    button: 0,
    isPrimary: true,
    pointerType: "mouse",
    clientX: 0,
    clientY: 0,
    currentTarget,
    target,
    ...overrides,
  } as ReactPointerEvent<HTMLElement>;
}

function deck(seed: string, name: string): DeckSummary {
  return {
    id: seed.repeat(32),
    name,
    description: "",
    sourceNoteCount: 1,
    generatedCardCount: 1,
    studyableDueCount: 1,
    archivedAt: null,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  };
}

function makeFolder(seed: string, name: string, sortOrder: number): DeckFolder {
  return {
    id: seed.repeat(32),
    name,
    sortOrder,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  };
}
