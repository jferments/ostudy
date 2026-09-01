import { act, renderHook } from "@testing-library/react";
import type { PointerEvent as ReactPointerEvent, RefObject } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { DeckSummary } from "../../../ipc/decks";
import type { DeckLibraryOrganization } from "../../../ipc/deckFolders";
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
    "captures an activated %s pointer and emits exactly one validated intent",
    (pointerType) => {
      const harness = libraryHarness();
      const onMove = vi.fn();
      const onExpand = vi.fn();
      const clicked = vi.fn();
      harness.origin.addEventListener("click", clicked);
      const { result, unmount } = renderHook(() =>
        useDeckLibraryPointerSession(
          harness.ref,
          snapshot,
          false,
          "stable",
          onMove,
          onExpand,
        ),
      );

      act(() => {
        result.current.beginPointerSession(
          reactPointer(harness.origin, harness.originArea, {
            pointerType,
            clientX: 20,
            clientY: 520,
          }),
          { kind: "deck", id: deckB.id },
          deckB.name,
        );
        window.dispatchEvent(pointerEvent("pointermove", 60, 100));
        window.dispatchEvent(pointerEvent("pointerup", 60, 100));
      });

      expect(harness.origin.setPointerCapture).toHaveBeenCalledWith(9);
      expect(harness.origin.releasePointerCapture).toHaveBeenCalledWith(9);
      expect(onMove).toHaveBeenCalledTimes(1);
      expect(onMove).toHaveBeenCalledWith({
        kind: "deck",
        deckId: deckB.id,
        targetFolderId: folder.id,
        targetIndex: 0,
        source: "pointer",
      });
      expect(harness.folderSurface.dataset.dragState).toBeUndefined();
      expect(harness.folderDeck.dataset.dragInsertion).toBeUndefined();
      expect(result.current.feedback.candidate?.announcement).toContain("Moved Biology");
      harness.origin.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      expect(clicked).not.toHaveBeenCalled();
      harness.origin.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      expect(clicked).toHaveBeenCalledTimes(1);
      unmount();
    },
  );

  it("starts production timing before capture and brackets animation-frame work", () => {
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
    });

    expect(beginWork).toHaveBeenNthCalledWith(1, "deck-drag-activation");
    expect(beginWork.mock.invocationCallOrder[0] as number).toBeLessThan(
      harness.origin.setPointerCapture.mock.invocationCallOrder[0] as number,
    );
    expect(endWork).toHaveBeenCalledTimes(1);

    act(() => frames.shift()?.(0));

    expect(beginWork).toHaveBeenNthCalledWith(2, "deck-drag-frame");
    expect(endWork).toHaveBeenCalledTimes(2);
  });

  it("keeps an activated window-listener drag cancellable when pointer capture throws", () => {
    const harness = libraryHarness();
    const onMove = vi.fn();
    harness.origin.setPointerCapture.mockImplementation(() => {
      throw new DOMException("Synthetic pointer is not active", "NotFoundError");
    });
    harness.origin.hasPointerCapture.mockReturnValue(false);
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
    expect(harness.origin.setPointerCapture).toHaveBeenCalledWith(9);
    expect(result.current.feedback.active).toBe(true);

    act(() => window.dispatchEvent(pointerEvent("pointercancel", 60, 100)));

    expect(result.current.feedback.active).toBe(false);
    expect(onMove).not.toHaveBeenCalled();
    expect(harness.origin.releasePointerCapture).not.toHaveBeenCalled();
  });

  it("suppresses the compatibility click after an activated cancellation", () => {
    const harness = libraryHarness();
    const clicked = vi.fn();
    harness.origin.addEventListener("click", clicked);
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
      window.dispatchEvent(pointerEvent("pointercancel", 60, 100));
    });

    harness.originArea.click();
    expect(clicked).not.toHaveBeenCalled();
    harness.originArea.click();
    expect(clicked).toHaveBeenCalledTimes(1);
  });

  it("lets unrelated clicks pass without consuming the pending origin compatibility click", () => {
    const harness = libraryHarness();
    const unrelated = document.createElement("button");
    const unrelatedClicked = vi.fn();
    const originClicked = vi.fn();
    unrelated.addEventListener("click", unrelatedClicked);
    harness.origin.addEventListener("click", originClicked);
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
    expect(unrelatedClicked).toHaveBeenCalledTimes(1);
    harness.originArea.click();
    expect(originClicked).not.toHaveBeenCalled();
    harness.originArea.click();
    expect(originClicked).toHaveBeenCalledTimes(1);
  });

  it.each([
    {
      label: "valid before",
      status: "valid",
      payload: { kind: "deck", id: deckB.id } as const,
      clientY: 100,
      insertionTarget: "deck",
      insertionEdge: "before",
    },
    {
      label: "valid after",
      status: "valid",
      payload: { kind: "deck", id: deckB.id } as const,
      clientY: 280,
      insertionTarget: "deck",
      insertionEdge: "after",
    },
    {
      label: "no-op empty",
      status: "noop",
      payload: { kind: "deck", id: deckA.id } as const,
      clientY: 100,
      insertionTarget: "destination",
      insertionEdge: "empty",
    },
    {
      label: "invalid before",
      status: "invalid",
      payload: { kind: "deck", id: "missing" } as const,
      clientY: 100,
      insertionTarget: "deck",
      insertionEdge: "before",
    },
  ] as const)(
    "reapplies $label target and insertion styling after same-candidate geometry recapture without feedback commits",
    ({ status, payload, clientY, insertionTarget, insertionEdge }) => {
      const frames: FrameRequestCallback[] = [];
      vi.mocked(window.requestAnimationFrame).mockImplementation((callback) => {
        frames.push(callback);
        return frames.length;
      });
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
          payload,
          deckB.name,
        );
        window.dispatchEvent(pointerEvent("pointermove", 60, clientY));
        frames.shift()?.(0);
      });
      expect(harness.folderSurface.dataset.dragState).toBe(status);
      expect(harness.folderSurface.dataset.dragInsertion).toBe(
        insertionTarget === "destination" ? insertionEdge : undefined,
      );
      expect(harness.folderDeck.dataset.dragInsertion).toBe(
        insertionTarget === "deck" ? insertionEdge : undefined,
      );
      const feedbackBeforeRecapture = result.current.feedback;
      const rendersBeforeRecapture = renders;

      const recapturedSurface = harness.folderSurface.cloneNode(false) as HTMLElement;
      rect(recapturedSurface, 0, 0, 700, 320);
      harness.root.replaceChild(recapturedSurface, harness.folderSurface);
      recapturedSurface.append(harness.folderDeck);
      act(() => {
        result.current.invalidateGeometry();
        frames.shift()?.(16);
      });

      expect(recapturedSurface.dataset.dragState).toBe(status);
      expect(recapturedSurface.dataset.dragInsertion).toBe(
        insertionTarget === "destination" ? insertionEdge : undefined,
      );
      expect(harness.folderDeck.dataset.dragInsertion).toBe(
        insertionTarget === "deck" ? insertionEdge : undefined,
      );
      expect(result.current.feedback).toBe(feedbackBeforeRecapture);
      expect(renders).toBe(rendersBeforeRecapture);
      act(() => result.current.cancel());
      expect(recapturedSurface.dataset.dragState).toBeUndefined();
      expect(harness.folderDeck.dataset.dragInsertion).toBeUndefined();
    },
  );

  it("moves and clears destination and insertion attributes independently", () => {
    const frames: FrameRequestCallback[] = [];
    vi.mocked(window.requestAnimationFrame).mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    const harness = libraryHarness();
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
      frames.shift()?.(0);
    });
    expect(harness.folderSurface.dataset.dragState).toBe("valid");
    expect(harness.folderDeck.dataset.dragInsertion).toBe("before");

    act(() => {
      window.dispatchEvent(pointerEvent("pointermove", 900, 500));
      frames.shift()?.(16);
    });
    expect(harness.folderSurface.dataset.dragState).toBeUndefined();
    expect(harness.folderDeck.dataset.dragInsertion).toBeUndefined();

    act(() => result.current.cancel());
  });

  it("keeps movement below threshold as an ordinary click without capture or move", () => {
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
      window.dispatchEvent(pointerEvent("pointermove", 24, 520));
      window.dispatchEvent(pointerEvent("pointerup", 24, 520));
      harness.origin.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });

    expect(harness.origin.setPointerCapture).not.toHaveBeenCalled();
    expect(onMove).not.toHaveBeenCalled();
    expect(clicked).toHaveBeenCalledTimes(1);
  });

  it("cancels active work on Escape and a disabled transition", () => {
    const harness = libraryHarness();
    const onMove = vi.fn();
    const { result, rerender } = renderHook(
      ({ disabled }) =>
        useDeckLibraryPointerSession(
          harness.ref,
          snapshot,
          disabled,
          "stable",
          onMove,
          vi.fn(),
        ),
      { initialProps: { disabled: false } },
    );

    act(() => {
      result.current.beginPointerSession(
        reactPointer(harness.origin, harness.originArea, { clientX: 20, clientY: 520 }),
        { kind: "deck", id: deckB.id },
        deckB.name,
      );
      window.dispatchEvent(pointerEvent("pointermove", 60, 100));
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    });
    expect(onMove).not.toHaveBeenCalled();
    expect(harness.origin.releasePointerCapture).toHaveBeenCalledTimes(1);

    act(() => {
      result.current.beginPointerSession(
        reactPointer(harness.origin, harness.originArea, { clientX: 20, clientY: 520 }),
        { kind: "deck", id: deckB.id },
        deckB.name,
      );
      window.dispatchEvent(pointerEvent("pointermove", 60, 100));
      rerender({ disabled: true });
    });
    expect(onMove).not.toHaveBeenCalled();
    expect(harness.origin.releasePointerCapture).toHaveBeenCalledTimes(2);
  });

  it.each(["pointercancel", "lostpointercapture", "blur", "unmount"])(
    "releases active state without a move on %s",
    (reason) => {
      const harness = libraryHarness();
      const onMove = vi.fn();
      const { result, unmount } = renderHook(() =>
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
        if (reason === "pointercancel") window.dispatchEvent(pointerEvent(reason, 60, 100));
        else if (reason === "lostpointercapture") {
          harness.origin.dispatchEvent(pointerEvent(reason, 60, 100));
        } else if (reason === "blur") window.dispatchEvent(new Event("blur"));
        else unmount();
      });
      expect(onMove).not.toHaveBeenCalled();
      expect(harness.origin.releasePointerCapture).toHaveBeenCalledTimes(1);
      if (reason !== "unmount") unmount();
    },
  );

  it("cancels stale revision work before pointer completion", () => {
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
    act(() => {
      rerender({
        currentSnapshot: createDeckLibrarySnapshot([deckA, deckB], {
          ...organization,
          revision: organization.revision + 1,
        }),
      });
    });
    act(() => window.dispatchEvent(pointerEvent("pointerup", 60, 100)));
    expect(onMove).not.toHaveBeenCalled();
    expect(result.current.feedback.active).toBe(false);
    expect(harness.origin.releasePointerCapture).toHaveBeenCalledTimes(1);
  });

  it("cancels an unexpected structural change at the captured revision", () => {
    const harness = libraryHarness();
    const onMove = vi.fn();
    const { result, rerender } = renderHook(
      ({ structureKey }) =>
        useDeckLibraryPointerSession(
          harness.ref,
          snapshot,
          false,
          structureKey,
          onMove,
          vi.fn(),
        ),
      { initialProps: { structureKey: "stable" } },
    );
    act(() => {
      result.current.beginPointerSession(
        reactPointer(harness.origin, harness.originArea, { clientX: 20, clientY: 520 }),
        { kind: "deck", id: deckB.id },
        deckB.name,
      );
      window.dispatchEvent(pointerEvent("pointermove", 60, 100));
      rerender({ structureKey: "changed" });
    });
    expect(onMove).not.toHaveBeenCalled();
    expect(result.current.feedback.active).toBe(false);
    expect(harness.origin.releasePointerCapture).toHaveBeenCalledTimes(1);
  });

  it("ignores other pointers and flushes the active pointer-up coordinate", () => {
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
        reactPointer(harness.origin, harness.originArea, { clientX: 20, clientY: 520 }),
        { kind: "deck", id: deckB.id },
        deckB.name,
      );
      window.dispatchEvent(pointerEvent("pointermove", 60, 100, 10));
      window.dispatchEvent(pointerEvent("pointerup", 60, 100, 10));
      window.dispatchEvent(pointerEvent("pointermove", 60, 500));
      window.dispatchEvent(pointerEvent("pointerup", 60, 100));
    });
    expect(onMove).toHaveBeenCalledWith({
      kind: "deck",
      deckId: deckB.id,
      targetFolderId: folder.id,
      targetIndex: 0,
      source: "pointer",
    });
    expect(onMove).toHaveBeenCalledTimes(1);
  });

  it.each([
    ["no-op", 60, 530],
    ["invalid", 900, 530],
  ])("does not emit a move for a %s pointer-up candidate", (_label, clientX, clientY) => {
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
        reactPointer(harness.origin, harness.originArea, { clientX: 20, clientY: 520 }),
        { kind: "deck", id: deckB.id },
        deckB.name,
      );
      window.dispatchEvent(pointerEvent("pointermove", clientX, clientY));
      window.dispatchEvent(pointerEvent("pointerup", clientX, clientY));
    });
    expect(onMove).not.toHaveBeenCalled();
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
    vi.useRealTimers();
  });
});

function libraryHarness() {
  const main = document.createElement("main");
  const root = document.createElement("div");
  const folderSurface = document.createElement("section");
  const folderDeck = document.createElement("article");
  const origin = document.createElement("article") as HTMLElement & {
    setPointerCapture: ReturnType<typeof vi.fn>;
    hasPointerCapture: ReturnType<typeof vi.fn>;
    releasePointerCapture: ReturnType<typeof vi.fn>;
  };
  const originArea = document.createElement("span");

  main.append(root);
  root.append(folderSurface, origin);
  folderSurface.append(folderDeck);
  origin.append(originArea);
  document.body.append(main);
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
  Object.assign(origin.dataset, {
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
  rect(folderDeck, 20, 80, 680, 150);
  rect(origin, 20, 500, 680, 570);
  Object.assign(main, { scrollBy: vi.fn() });
  Object.assign(origin, {
    setPointerCapture: vi.fn(),
    hasPointerCapture: vi.fn().mockReturnValue(true),
    releasePointerCapture: vi.fn(),
  });
  return {
    main,
    root,
    folderSurface,
    folderDeck,
    origin,
    originArea,
    ref: { current: root } as RefObject<HTMLElement>,
  };
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
    id: seed.repeat(32), name, description: "", sourceNoteCount: 1,
    generatedCardCount: 1, studyableDueCount: 1, archivedAt: null,
    createdAt: 1, updatedAt: 1, revision: 1,
  };
}

function makeFolder(seed: string, name: string, sortOrder: number) {
  return {
    id: seed.repeat(32), name, sortOrder, createdAt: 1, updatedAt: 1, revision: 1,
  };
}
