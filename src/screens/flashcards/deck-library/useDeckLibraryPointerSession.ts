import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
  type RefCallback,
} from "react";

import type { DeckLibrarySnapshot } from "./organizationModel";
import { resolveDragCandidate, type DragCandidate } from "./dragSessionModel";
import {
  HOVER_EXPAND_DELAY_MS,
  canStartPointerSession,
  captureDragGeometry,
  edgeScrollVelocity,
  findLibraryScrollOwner,
  movementExceedsThreshold,
  type PointerSessionStart,
} from "./pointerDragSession";
import type { LibraryDragPayload, LibraryMoveIntent } from "./types";

interface ActivePointerSession extends PointerSessionStart {
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
  capturedRevision: number;
  capturedStructureKey: string;
  expectsStructureChange: boolean;
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
  bindPreview: RefCallback<HTMLDivElement>;
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
  const previewRef = useRef<HTMLDivElement | null>(null);
  const snapshotRef = useRef(snapshot);
  const disabledRef = useRef(disabled);
  const structureKeyRef = useRef(structureKey);
  const onMoveRef = useRef(onMove);
  const onExpandFolderRef = useRef(onExpandFolder);
  const clickSuppressionRef = useRef<ClickSuppression | null>(null);
  const [feedback, setFeedback] = useState<PointerDragFeedbackState>(EMPTY_FEEDBACK);
  snapshotRef.current = snapshot;
  disabledRef.current = disabled;
  structureKeyRef.current = structureKey;
  onMoveRef.current = onMove;
  onExpandFolderRef.current = onExpandFolder;

  const clearTarget = useCallback((session: ActivePointerSession): void => {
    if (!session.geometry || !session.candidate?.targetKey) return;
    const element = session.geometry.elements.get(session.candidate.targetKey);
    if (element) {
      delete element.dataset.dragState;
      delete element.dataset.dragInsertion;
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

  const suppressActivatedDragClick = useCallback((origin: HTMLElement): void => {
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
  }, [clearClickSuppression]);

  const removeListeners = useCallback((session: ActivePointerSession): void => {
    window.removeEventListener("pointermove", handlePointerMove, true);
    window.removeEventListener("pointerup", handlePointerUp, true);
    window.removeEventListener("pointercancel", handlePointerCancel, true);
    window.removeEventListener("blur", handleBlur);
    document.removeEventListener("keydown", handleKeyDown, true);
    session.origin.removeEventListener("lostpointercapture", handleLostPointerCapture);
    session.scrollOwner.removeEventListener("scroll", handleScroll);
    window.removeEventListener("resize", handleResize);
  // Stable event functions read sessionRef and are intentionally installed once per session.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const finishSession = useCallback(
    (commit: boolean, completion = "Move cancelled."): void => {
      const session = sessionRef.current;
      if (!session) return;
      sessionRef.current = null;
      if (session.frame !== null) cancelAnimationFrame(session.frame);
      clearHoverTimer(session);
      clearTarget(session);
      removeListeners(session);
      if (
        session.active &&
        typeof session.origin.hasPointerCapture === "function" &&
        session.origin.hasPointerCapture(session.pointerId)
      ) {
        session.origin.releasePointerCapture(session.pointerId);
      }

      const candidate = session.candidate;
      const currentRevision = snapshotRef.current.organization.revision;
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
      if (session.active && commit) suppressActivatedDragClick(session.origin);
    },
    [clearHoverTimer, clearTarget, removeListeners, suppressActivatedDragClick],
  );

  const applyCandidate = useCallback(
    (session: ActivePointerSession, next: DragCandidate): void => {
      const semanticCandidateChanged = session.candidate?.key !== next.key;
      if (!semanticCandidateChanged) {
        if (session.geometry && next.targetKey) {
          const element = session.geometry.elements.get(next.targetKey);
          if (element) {
            element.dataset.dragState = next.status;
            element.dataset.dragInsertion = next.insertionLabel;
          }
        }
        return;
      }
      clearTarget(session);
      session.candidate = next;
      if (session.geometry && next.targetKey) {
        const element = session.geometry.elements.get(next.targetKey);
        if (element) {
          element.dataset.dragState = next.status;
          element.dataset.dragInsertion = next.insertionLabel;
        }
      }
      setFeedback({ active: true, sourceName: session.sourceName, candidate: next });

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
    [clearHoverTimer, clearTarget],
  );

  const processFrame = useCallback(
    (session: ActivePointerSession): void => {
      session.frame = null;
      if (sessionRef.current !== session || !session.active) return;
      const root = rootRef.current;
      if (!root) {
        finishSession(false);
        return;
      }
      if (session.geometryDirty || session.geometry === null) {
        clearTarget(session);
        session.geometry = captureDragGeometry(root);
        session.geometryDirty = false;
      }
      if (previewRef.current) {
        previewRef.current.style.transform = `translate3d(${session.lastX + 14}px, ${session.lastY + 14}px, 0)`;
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
    },
    [applyCandidate, clearTarget, finishSession, rootRef],
  );

  const scheduleFrame = useCallback(
    (session: ActivePointerSession): void => {
      if (session.frame !== null) return;
      session.frame = requestAnimationFrame(() => processFrame(session));
    },
    [processFrame],
  );

  // These declarations are hoisted and keep native listener identity stable.
  function handlePointerMove(event: PointerEvent): void {
    const session = sessionRef.current;
    if (!session || event.pointerId !== session.pointerId) return;
    session.lastX = event.clientX;
    session.lastY = event.clientY;
    if (!session.active) {
      if (!movementExceedsThreshold(session.startX, session.startY, event.clientX, event.clientY)) {
        return;
      }
      session.active = true;
      session.origin.setPointerCapture?.(session.pointerId);
      setFeedback({ active: true, sourceName: session.sourceName, candidate: null });
    }
    event.preventDefault();
    scheduleFrame(session);
  }

  function handlePointerUp(event: PointerEvent): void {
    const session = sessionRef.current;
    if (!session || event.pointerId !== session.pointerId) return;
    session.lastX = event.clientX;
    session.lastY = event.clientY;
    if (session.frame !== null) {
      cancelAnimationFrame(session.frame);
      session.frame = null;
    }
    if (session.active) {
      event.preventDefault();
      processFrame(session);
    }
    finishSession(true);
  }

  function handlePointerCancel(event: PointerEvent): void {
    if (sessionRef.current?.pointerId === event.pointerId) finishSession(false);
  }

  function handleLostPointerCapture(event: PointerEvent): void {
    if (sessionRef.current?.pointerId === event.pointerId) finishSession(false);
  }

  function handleBlur(): void {
    finishSession(false);
  }

  function handleKeyDown(event: KeyboardEvent): void {
    if (event.key !== "Escape") return;
    event.preventDefault();
    finishSession(false);
  }

  function handleScroll(): void {
    const session = sessionRef.current;
    if (session?.active) {
      session.geometryDirty = true;
      scheduleFrame(session);
    }
  }

  function handleResize(): void {
    const session = sessionRef.current;
    if (session?.active) {
      session.geometryDirty = true;
      scheduleFrame(session);
    }
  }

  const beginPointerSession = useCallback(
    (
      event: ReactPointerEvent<HTMLElement>,
      payload: LibraryDragPayload,
      sourceName: string,
    ): void => {
      if (
        disabledRef.current ||
        sessionRef.current ||
        !canStartPointerSession({
          button: event.button,
          isPrimary: event.isPrimary,
          pointerType: event.pointerType,
          target: event.target,
          currentTarget: event.currentTarget,
        })
      ) {
        return;
      }
      const root = rootRef.current;
      if (!root) return;
      const scrollOwner = findLibraryScrollOwner(root);
      const session: ActivePointerSession = {
        payload,
        pointerId: event.pointerId,
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
        capturedRevision: snapshotRef.current.organization.revision,
        capturedStructureKey: structureKeyRef.current,
        expectsStructureChange: false,
      };
      sessionRef.current = session;
      window.addEventListener("pointermove", handlePointerMove, { capture: true, passive: false });
      window.addEventListener("pointerup", handlePointerUp, true);
      window.addEventListener("pointercancel", handlePointerCancel, true);
      window.addEventListener("blur", handleBlur);
      document.addEventListener("keydown", handleKeyDown, true);
      session.origin.addEventListener("lostpointercapture", handleLostPointerCapture);
      scrollOwner.addEventListener("scroll", handleScroll, { passive: true });
      window.addEventListener("resize", handleResize, { passive: true });
    },
    [rootRef],
  );

  const invalidateGeometry = useCallback((): void => {
    const session = sessionRef.current;
    if (session?.active) {
      session.geometryDirty = true;
      scheduleFrame(session);
    }
  }, [scheduleFrame]);

  const bindPreview = useCallback<RefCallback<HTMLDivElement>>((element) => {
    previewRef.current = element;
  }, []);

  useEffect(() => {
    const session = sessionRef.current;
    if (!session) return;
    if (snapshot.organization.revision !== session.capturedRevision) {
      finishSession(false, "The library changed during the drag. Move cancelled.");
    } else if (structureKey !== session.capturedStructureKey) {
      if (!session.expectsStructureChange) {
        finishSession(false, "The library structure changed during the drag. Move cancelled.");
        return;
      }
      session.expectsStructureChange = false;
      session.capturedStructureKey = structureKey;
      session.geometryDirty = true;
      scheduleFrame(session);
    }
  }, [finishSession, scheduleFrame, snapshot.organization.revision, structureKey]);

  useEffect(() => {
    if (disabled) finishSession(false);
  }, [disabled, finishSession]);

  useEffect(
    () => () => {
      finishSession(false);
      clearClickSuppression();
    },
    [clearClickSuppression, finishSession],
  );

  return useMemo(
    () => ({
      feedback,
      beginPointerSession,
      bindPreview,
      invalidateGeometry,
      cancel: () => finishSession(false),
    }),
    [beginPointerSession, bindPreview, feedback, finishSession, invalidateGeometry],
  );
}
