import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { flushSync } from "react-dom";

import {
  beginUiPerformancePointerWork,
  endUiPerformancePointerWork,
} from "../../../testing/uiPerformanceInstrumentation";
import type { DeckLibrarySnapshot } from "./organizationModel";
import { resolveDragCandidate, type DragCandidate } from "./dragSessionModel";
import {
  HOVER_EXPAND_DELAY_MS,
  activationDistance,
  canStartPointerSession,
  captureDragGeometry,
  edgeScrollVelocity,
  findLibraryScrollOwner,
  movementExceedsThreshold,
  type LibraryPointerType,
  type PointerSessionStart,
} from "./pointerDragSession";
import dragStyles from "./LibraryDragFeedback.module.css";
import type { LibraryDragPayload, LibraryMoveIntent } from "./types";

interface SessionListeners {
  pointerMove: (event: PointerEvent) => void;
  pointerUp: (event: PointerEvent) => void;
  pointerCancel: (event: PointerEvent) => void;
  lostPointerCapture: (event: PointerEvent) => void;
  blur: () => void;
  keyDown: (event: KeyboardEvent) => void;
  scroll: () => void;
  resize: () => void;
}

interface DragDomState {
  sourceRow: HTMLElement;
  sourceDisplay: string;
  sourceAriaHidden: string | null;
  placeholder: HTMLDivElement;
  preview: HTMLDivElement;
  destination: HTMLSpanElement;
  grabOffsetX: number;
  grabOffsetY: number;
}

interface ActivePointerSession extends PointerSessionStart {
  payload: LibraryDragPayload;
  active: boolean;
  lastX: number;
  lastY: number;
  geometry: ReturnType<typeof captureDragGeometry> | null;
  geometryDirty: boolean;
  candidate: DragCandidate | null;
  frame: number | null;
  hoverTimer: number | null;
  hoverFolderId: string | null;
  scrollOwner: HTMLElement;
  captureTarget: HTMLElement;
  capturedRevision: number;
  capturedStructureKey: string;
  expectsStructureChange: boolean;
  dom: DragDomState | null;
  listeners: SessionListeners;
}

interface ClickSuppression {
  listener: (event: MouseEvent) => void;
  timer: number;
}

export interface PointerDragFeedbackState {
  active: boolean;
  sourceName: string;
  candidate: DragCandidate | null;
}

export interface DeckLibraryPointerSessionAdapter {
  feedback: PointerDragFeedbackState;
  beginPointerSession(
    event: ReactPointerEvent<HTMLElement>,
    payload: LibraryDragPayload,
    sourceName: string,
  ): void;
  invalidateGeometry(): void;
  cancel(): void;
}

const EMPTY_FEEDBACK: PointerDragFeedbackState = {
  active: false,
  sourceName: "",
  candidate: null,
};

export function useDeckLibraryPointerSession(
  rootRef: React.RefObject<HTMLElement | null>,
  snapshot: DeckLibrarySnapshot,
  disabled: boolean,
  structureKey: string,
  onMove: (intent: LibraryMoveIntent) => void | Promise<void>,
  onExpandFolder: (folderId: string) => void,
): DeckLibraryPointerSessionAdapter {
  const sessionRef = useRef<ActivePointerSession | null>(null);
  const snapshotRef = useRef(snapshot);
  const disabledRef = useRef(disabled);
  const structureKeyRef = useRef(structureKey);
  const onMoveRef = useRef(onMove);
  const onExpandFolderRef = useRef(onExpandFolder);
  const clickSuppressionRef = useRef<ClickSuppression | null>(null);
  const processFrameRef = useRef<(session: ActivePointerSession) => void>(() => undefined);
  const finishSessionRef = useRef<
    (session: ActivePointerSession, commit: boolean, completion?: string) => void
  >(() => undefined);
  const [feedback, setFeedback] = useState<PointerDragFeedbackState>(EMPTY_FEEDBACK);
  snapshotRef.current = snapshot;
  disabledRef.current = disabled;
  structureKeyRef.current = structureKey;
  onMoveRef.current = onMove;
  onExpandFolderRef.current = onExpandFolder;

  const clearTarget = useCallback((session: ActivePointerSession): void => {
    if (!session.geometry || !session.candidate) return;
    if (session.candidate.targetKey) {
      const destination = session.geometry.elements.get(session.candidate.targetKey);
      if (destination) delete destination.dataset.dragState;
    }
    if (session.candidate.insertionTargetKey) {
      const insertion = session.geometry.elements.get(
        session.candidate.insertionTargetKey,
      );
      if (insertion) delete insertion.dataset.dragInsertion;
    }
  }, []);

  const clearHoverTimer = useCallback((session: ActivePointerSession): void => {
    if (session.hoverTimer !== null) window.clearTimeout(session.hoverTimer);
    session.hoverTimer = null;
    session.hoverFolderId = null;
  }, []);

  const clearClickSuppression = useCallback((): void => {
    const suppression = clickSuppressionRef.current;
    if (!suppression) return;
    window.removeEventListener("click", suppression.listener, true);
    window.clearTimeout(suppression.timer);
    clickSuppressionRef.current = null;
  }, []);

  const suppressActivatedDragClick = useCallback(
    (origin: HTMLElement): void => {
      clearClickSuppression();
      const listener = (event: MouseEvent): void => {
        const target = event.target;
        if (!(target instanceof Node) || (target !== origin && !origin.contains(target))) return;
        event.preventDefault();
        event.stopImmediatePropagation();
        clearClickSuppression();
      };
      const timer = window.setTimeout(clearClickSuppression, 500);
      clickSuppressionRef.current = { listener, timer };
      window.addEventListener("click", listener, true);
    },
    [clearClickSuppression],
  );

  const removeListeners = useCallback((session: ActivePointerSession): void => {
    const listeners = session.listeners;
    window.removeEventListener("pointermove", listeners.pointerMove, true);
    window.removeEventListener("pointerup", listeners.pointerUp, true);
    window.removeEventListener("pointercancel", listeners.pointerCancel, true);
    window.removeEventListener("blur", listeners.blur);
    document.removeEventListener("keydown", listeners.keyDown, true);
    session.captureTarget.removeEventListener(
      "lostpointercapture",
      listeners.lostPointerCapture,
    );
    session.scrollOwner.removeEventListener("scroll", listeners.scroll);
    window.removeEventListener("resize", listeners.resize);
  }, []);

  const restoreDragDom = useCallback((session: ActivePointerSession): void => {
    const dom = session.dom;
    if (!dom) return;
    dom.preview.remove();
    dom.placeholder.remove();
    dom.sourceRow.style.display = dom.sourceDisplay;
    if (dom.sourceAriaHidden === null) dom.sourceRow.removeAttribute("aria-hidden");
    else dom.sourceRow.setAttribute("aria-hidden", dom.sourceAriaHidden);
    delete dom.sourceRow.dataset.dragSourceHidden;
    delete session.captureTarget.dataset.libraryDragging;
    session.dom = null;
  }, []);

  const placePlaceholderAtOrigin = useCallback((session: ActivePointerSession): void => {
    const dom = session.dom;
    if (!dom || !dom.sourceRow.parentElement) return;
    if (dom.placeholder.parentElement !== dom.sourceRow.parentElement || dom.placeholder.nextSibling !== dom.sourceRow) {
      dom.sourceRow.parentElement.insertBefore(dom.placeholder, dom.sourceRow);
      session.geometryDirty = true;
    }
    dom.placeholder.dataset.status = "noop";
  }, []);

  const placePlaceholderForCandidate = useCallback(
    (session: ActivePointerSession, candidate: DragCandidate): void => {
      const dom = session.dom;
      if (!dom || candidate.status !== "valid" || !candidate.insertionTargetKey) {
        placePlaceholderAtOrigin(session);
        return;
      }
      const target = session.geometry?.elements.get(candidate.insertionTargetKey);
      if (!target) {
        placePlaceholderAtOrigin(session);
        return;
      }

      let parent: HTMLElement | null = null;
      let before: ChildNode | null = null;
      if (candidate.insertionEdge === "before") {
        parent = target.parentElement;
        before = target;
      } else if (candidate.insertionEdge === "after") {
        parent = target.parentElement;
        before = target.nextSibling;
      } else if (candidate.insertionEdge === "empty") {
        if (session.payload.kind === "deck") {
          const list = target.matches("[data-library-deck-list]")
            ? target
            : target.querySelector<HTMLElement>("[data-library-deck-list]");
          // A collapsed folder has no list until hover expansion completes.
          if (!list) {
            placePlaceholderAtOrigin(session);
            return;
          }
          parent = list;
          before = list.firstChild;
        } else {
          parent = target.parentElement;
          before = target;
        }
      }
      if (!parent) {
        placePlaceholderAtOrigin(session);
        return;
      }
      if (dom.placeholder.parentElement !== parent || dom.placeholder.nextSibling !== before) {
        parent.insertBefore(dom.placeholder, before);
        session.geometryDirty = true;
      }
      dom.placeholder.dataset.status = candidate.status;
    },
    [placePlaceholderAtOrigin],
  );

  const updatePreview = useCallback((session: ActivePointerSession): void => {
    const dom = session.dom;
    if (!dom) return;
    dom.preview.style.transform = `translate3d(${session.lastX - dom.grabOffsetX}px, ${session.lastY - dom.grabOffsetY}px, 0)`;
    dom.preview.dataset.status = session.candidate?.status ?? "pending";
    dom.destination.textContent = session.candidate
      ? `${session.candidate.destinationName} · ${session.candidate.insertionLabel}`
      : "Choose a destination";
  }, []);

  const startDragDom = useCallback((session: ActivePointerSession): boolean => {
    const sourceRow = session.origin.closest<HTMLElement>("[data-library-row]");
    if (!sourceRow || !sourceRow.parentElement) return false;
    const rect = sourceRow.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return false;

    const placeholder = document.createElement("div");
    placeholder.className = dragStyles.placeholder;
    placeholder.dataset.dragPlaceholder = "true";
    placeholder.dataset.status = "pending";
    placeholder.setAttribute("aria-hidden", "true");
    placeholder.style.width = `${rect.width}px`;
    placeholder.style.height = `${rect.height}px`;
    sourceRow.parentElement.insertBefore(placeholder, sourceRow);

    const preview = document.createElement("div");
    preview.className = dragStyles.preview;
    preview.dataset.dragPreview = "true";
    preview.dataset.dragPanelPreview = "true";
    preview.dataset.status = "pending";
    preview.setAttribute("aria-hidden", "true");
    preview.style.width = `${rect.width}px`;
    preview.style.height = `${rect.height}px`;

    const cloneHost = document.createElement("div");
    cloneHost.className = dragStyles.cloneHost;
    const clone = sourceRow.cloneNode(true) as HTMLElement;
    sanitizePreviewClone(clone);
    cloneHost.append(clone);
    preview.append(cloneHost);

    const destination = document.createElement("span");
    destination.className = dragStyles.destination;
    destination.textContent = "Choose a destination";
    preview.append(destination);
    document.body.append(preview);

    session.dom = {
      sourceRow,
      sourceDisplay: sourceRow.style.display,
      sourceAriaHidden: sourceRow.getAttribute("aria-hidden"),
      placeholder,
      preview,
      destination,
      grabOffsetX: Math.max(0, Math.min(rect.width, session.startX - rect.left)),
      grabOffsetY: Math.max(0, Math.min(rect.height, session.startY - rect.top)),
    };
    sourceRow.dataset.dragSourceHidden = "true";
    sourceRow.style.display = "none";
    sourceRow.setAttribute("aria-hidden", "true");
    session.captureTarget.dataset.libraryDragging = session.payload.kind;
    session.geometryDirty = true;
    updatePreview(session);
    return true;
  }, [updatePreview]);

  const applyCandidateAttributes = useCallback(
    (session: ActivePointerSession, candidate: DragCandidate): void => {
      if (!session.geometry) return;
      if (candidate.targetKey) {
        const destination = session.geometry.elements.get(candidate.targetKey);
        if (destination) destination.dataset.dragState = candidate.status;
      }
      if (candidate.insertionTargetKey && candidate.insertionEdge) {
        const insertion = session.geometry.elements.get(candidate.insertionTargetKey);
        if (insertion) insertion.dataset.dragInsertion = candidate.insertionEdge;
      }
    },
    [],
  );

  const scheduleFrame = useCallback((session: ActivePointerSession): void => {
    if (session.frame !== null) return;
    session.frame = requestAnimationFrame(() => processFrameRef.current(session));
  }, []);

  const applyCandidate = useCallback(
    (session: ActivePointerSession, next: DragCandidate): void => {
      const semanticCandidateChanged = session.candidate?.key !== next.key;
      if (semanticCandidateChanged) {
        clearTarget(session);
        session.candidate = next;
      }
      applyCandidateAttributes(session, next);
      placePlaceholderForCandidate(session, next);
      updatePreview(session);
      if (semanticCandidateChanged) {
        setFeedback({ active: true, sourceName: session.sourceName, candidate: next });
      }

      const destination = session.geometry?.snapshot.destinations.find(
        (candidate) => candidate.key === next.targetKey,
      );
      if (
        session.payload.kind === "deck" &&
        destination?.collapsed &&
        destination.folderId &&
        next.status !== "invalid"
      ) {
        if (session.hoverFolderId === destination.folderId) return;
        const folderId = destination.folderId;
        clearHoverTimer(session);
        session.hoverFolderId = folderId;
        session.hoverTimer = window.setTimeout(() => {
          const current = sessionRef.current;
          if (current !== session || current.hoverFolderId !== folderId) return;
          clearHoverTimer(current);
          current.expectsStructureChange = true;
          onExpandFolderRef.current(folderId);
          current.geometryDirty = true;
          scheduleFrame(current);
        }, HOVER_EXPAND_DELAY_MS);
      } else {
        clearHoverTimer(session);
      }
    },
    [
      applyCandidateAttributes,
      clearHoverTimer,
      clearTarget,
      placePlaceholderForCandidate,
      scheduleFrame,
      updatePreview,
    ],
  );

  const finishSession = useCallback(
    (session: ActivePointerSession, commit: boolean, completion = "Move cancelled."): void => {
      if (sessionRef.current !== session) return;
      sessionRef.current = null;
      if (session.frame !== null) cancelAnimationFrame(session.frame);
      clearHoverTimer(session);
      clearTarget(session);
      removeListeners(session);
      if (
        session.active &&
        typeof session.captureTarget.hasPointerCapture === "function" &&
        session.captureTarget.hasPointerCapture(session.pointerId)
      ) {
        session.captureTarget.releasePointerCapture(session.pointerId);
      }

      const candidate = session.candidate;
      const currentRevision = snapshotRef.current.organization.revision;
      restoreDragDom(session);
      if (!session.active) return;

      if (
        commit &&
        !disabledRef.current &&
        currentRevision === session.capturedRevision &&
        candidate?.status === "valid" &&
        candidate.intent !== null
      ) {
        const intent = candidate.intent;
        setFeedback({
          active: false,
          sourceName: session.sourceName,
          candidate: {
            ...candidate,
            announcement: `Moved ${session.sourceName} to ${candidate.destinationName}, ${candidate.insertionLabel}.`,
          },
        });
        void onMoveRef.current(intent);
      } else {
        const announcement =
          commit && currentRevision !== session.capturedRevision
            ? "The library changed during the drag. Move cancelled."
            : candidate?.status === "noop"
              ? `${session.sourceName} is already in that position. No move made.`
              : candidate?.status === "invalid"
                ? `Invalid destination. No move made for ${session.sourceName}.`
                : completion;
        setFeedback({
          active: false,
          sourceName: session.sourceName,
          candidate: candidate ? { ...candidate, announcement } : null,
        });
      }
      suppressActivatedDragClick(session.origin);
    },
    [
      clearHoverTimer,
      clearTarget,
      removeListeners,
      restoreDragDom,
      suppressActivatedDragClick,
    ],
  );
  finishSessionRef.current = finishSession;

  processFrameRef.current = (session: ActivePointerSession): void => {
    const pointerWork = beginUiPerformancePointerWork("deck-drag-frame");
    try {
      session.frame = null;
      if (sessionRef.current !== session || !session.active) return;
      const root = rootRef.current;
      if (!root) {
        finishSessionRef.current(session, false);
        return;
      }
      if (session.geometryDirty || session.geometry === null) {
        clearTarget(session);
        session.geometry = captureDragGeometry(root);
        session.geometryDirty = false;
      }
      const candidate = resolveDragCandidate(
        session.payload,
        { x: session.lastX, y: session.lastY },
        session.geometry.snapshot,
        snapshotRef.current,
      );
      applyCandidate(session, candidate);

      const velocity = edgeScrollVelocity(
        session.lastY,
        session.scrollOwner.getBoundingClientRect(),
      );
      if (velocity !== 0) {
        session.scrollOwner.scrollBy({ top: velocity, behavior: "auto" });
        session.geometryDirty = true;
        scheduleFrame(session);
      }
    } finally {
      endUiPerformancePointerWork(pointerWork);
    }
  };

  const beginPointerSession = useCallback(
    (
      event: ReactPointerEvent<HTMLElement>,
      payload: LibraryDragPayload,
      sourceName: string,
    ): void => {
      const pointerType = event.pointerType as LibraryPointerType;
      if (
        disabledRef.current ||
        sessionRef.current ||
        !canStartPointerSession({
          button: event.button,
          isPrimary: event.isPrimary,
          pointerType,
          target: event.target,
          currentTarget: event.currentTarget,
        })
      ) {
        return;
      }
      const root = rootRef.current;
      if (!root) return;
      const scrollOwner = findLibraryScrollOwner(root);
      let session!: ActivePointerSession;
      const listeners: SessionListeners = {
        pointerMove(nativeEvent) {
          if (sessionRef.current !== session || nativeEvent.pointerId !== session.pointerId) return;
          const activating =
            !session.active &&
            movementExceedsThreshold(
              session.startX,
              session.startY,
              nativeEvent.clientX,
              nativeEvent.clientY,
              activationDistance(session.pointerType),
            );
          session.lastX = nativeEvent.clientX;
          session.lastY = nativeEvent.clientY;
          if (!session.active && !activating) return;

          const pointerWork = beginUiPerformancePointerWork(
            activating ? "deck-drag-activation" : "deck-drag-pointer",
          );
          try {
            if (activating) {
              if (!startDragDom(session)) {
                finishSessionRef.current(session, false);
                return;
              }
              session.active = true;
              try {
                session.captureTarget.setPointerCapture?.(session.pointerId);
              } catch {
                // Window listeners continue to own delivery when capture is unavailable.
              }
              flushSync(() => {
                setFeedback({ active: true, sourceName: session.sourceName, candidate: null });
              });
            }
            nativeEvent.preventDefault();
            updatePreview(session);
            scheduleFrame(session);
          } finally {
            endUiPerformancePointerWork(pointerWork);
          }
        },
        pointerUp(nativeEvent) {
          if (sessionRef.current !== session || nativeEvent.pointerId !== session.pointerId) return;
          session.lastX = nativeEvent.clientX;
          session.lastY = nativeEvent.clientY;
          if (session.frame !== null) {
            cancelAnimationFrame(session.frame);
            session.frame = null;
          }
          if (session.active) {
            nativeEvent.preventDefault();
            processFrameRef.current(session);
          }
          finishSessionRef.current(session, true);
        },
        pointerCancel(nativeEvent) {
          if (nativeEvent.pointerId === session.pointerId) {
            finishSessionRef.current(session, false);
          }
        },
        lostPointerCapture(nativeEvent) {
          if (nativeEvent.pointerId === session.pointerId) {
            finishSessionRef.current(session, false);
          }
        },
        blur() {
          finishSessionRef.current(session, false);
        },
        keyDown(nativeEvent) {
          if (nativeEvent.key !== "Escape") return;
          nativeEvent.preventDefault();
          finishSessionRef.current(session, false);
        },
        scroll() {
          if (!session.active) return;
          session.geometryDirty = true;
          scheduleFrame(session);
        },
        resize() {
          if (!session.active) return;
          session.geometryDirty = true;
          scheduleFrame(session);
        },
      };
      session = {
        payload,
        pointerId: event.pointerId,
        pointerType,
        startX: event.clientX,
        startY: event.clientY,
        lastX: event.clientX,
        lastY: event.clientY,
        origin: event.currentTarget,
        sourceName,
        active: false,
        geometry: null,
        geometryDirty: true,
        candidate: null,
        frame: null,
        hoverTimer: null,
        hoverFolderId: null,
        scrollOwner,
        captureTarget: root,
        capturedRevision: snapshotRef.current.organization.revision,
        capturedStructureKey: structureKeyRef.current,
        expectsStructureChange: false,
        dom: null,
        listeners,
      };
      sessionRef.current = session;
      window.addEventListener("pointermove", listeners.pointerMove, {
        capture: true,
        passive: false,
      });
      window.addEventListener("pointerup", listeners.pointerUp, true);
      window.addEventListener("pointercancel", listeners.pointerCancel, true);
      window.addEventListener("blur", listeners.blur);
      document.addEventListener("keydown", listeners.keyDown, true);
      root.addEventListener("lostpointercapture", listeners.lostPointerCapture);
      scrollOwner.addEventListener("scroll", listeners.scroll, { passive: true });
      window.addEventListener("resize", listeners.resize, { passive: true });
    },
    [rootRef, scheduleFrame, startDragDom, updatePreview],
  );

  const invalidateGeometry = useCallback((): void => {
    const session = sessionRef.current;
    if (session?.active) {
      session.geometryDirty = true;
      scheduleFrame(session);
    }
  }, [scheduleFrame]);

  useEffect(() => {
    const session = sessionRef.current;
    if (!session) return;
    if (snapshot.organization.revision !== session.capturedRevision) {
      finishSession(session, false, "The library changed during the drag. Move cancelled.");
    } else if (structureKey !== session.capturedStructureKey) {
      if (!session.expectsStructureChange) {
        finishSession(
          session,
          false,
          "The library structure changed during the drag. Move cancelled.",
        );
        return;
      }
      session.expectsStructureChange = false;
      session.capturedStructureKey = structureKey;
      session.geometryDirty = true;
      scheduleFrame(session);
    }
  }, [finishSession, scheduleFrame, snapshot.organization.revision, structureKey]);

  useEffect(() => {
    const session = sessionRef.current;
    if (disabled && session) finishSession(session, false);
  }, [disabled, finishSession]);

  useEffect(
    () => () => {
      const session = sessionRef.current;
      if (session) finishSession(session, false);
      clearClickSuppression();
    },
    [clearClickSuppression, finishSession],
  );

  return useMemo(
    () => ({
      feedback,
      beginPointerSession,
      invalidateGeometry,
      cancel: () => {
        const session = sessionRef.current;
        if (session) finishSession(session, false);
      },
    }),
    [beginPointerSession, feedback, finishSession, invalidateGeometry],
  );
}

function sanitizePreviewClone(clone: HTMLElement): void {
  clone.dataset.dragPreviewClone = "true";
  clone.removeAttribute("id");
  clone.removeAttribute("role");
  clone.removeAttribute("tabindex");
  clone.setAttribute("aria-hidden", "true");
  clone.setAttribute("inert", "");
  for (const element of clone.querySelectorAll<HTMLElement>("*")) {
    element.removeAttribute("id");
    element.removeAttribute("for");
    element.removeAttribute("aria-controls");
    element.removeAttribute("aria-describedby");
    element.removeAttribute("aria-labelledby");
    element.removeAttribute("tabindex");
    if (
      element instanceof HTMLButtonElement ||
      element instanceof HTMLInputElement ||
      element instanceof HTMLSelectElement ||
      element instanceof HTMLTextAreaElement
    ) {
      element.disabled = true;
    }
  }
}
