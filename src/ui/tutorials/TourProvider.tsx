import {
  cloneElement,
  createContext,
  type CSSProperties,
  type ReactElement,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";

import { registerPlatformBackHandler } from "../../platform/backNavigation";
import { Icon } from "../icons/Icon";
import { useOverlayContext } from "../overlays/overlayContext";
import { Button } from "../primitives/Button";
import {
  tutorialModuleOrder,
  tutorialModules,
  type TutorialModuleId,
  type TutorialPlacement,
} from "./tutorialRegistry";
import styles from "./TourProvider.module.css";

interface TourContextValue {
  activeModuleId: TutorialModuleId | null;
  start(moduleId: TutorialModuleId): void;
  dismiss(): void;
  restart(moduleId?: TutorialModuleId): void;
}

interface ActiveTour {
  moduleId: TutorialModuleId;
  stepIndex: number;
}

interface StoredTutorialState {
  schemaVersion: 1;
  completed: Partial<Record<TutorialModuleId, number>>;
  dismissed: Partial<Record<TutorialModuleId, number>>;
}

interface AnchorBox {
  top: number;
  left: number;
  width: number;
  height: number;
}

const STORAGE_KEY = "ostudy.tutorials.v1";
const CONTEXTUAL_AUTO_START_ENABLED =
  import.meta.env.VITE_OSTUDY_ANDROID_TEST_BRIDGE !== "1";
const EMPTY_STATE: StoredTutorialState = {
  schemaVersion: 1,
  completed: {},
  dismissed: {},
};
const TourContext = createContext<TourContextValue>({
  activeModuleId: null,
  start: () => undefined,
  dismiss: () => undefined,
  restart: () => undefined,
});

export function TourProvider({
  enabled,
  children,
}: {
  enabled: boolean;
  children: ReactNode;
}) {
  const [stored, setStored] = useState<StoredTutorialState>(() => readStoredState());
  const [active, setActive] = useState<ActiveTour | null>(null);
  const [anchorBox, setAnchorBox] = useState<AnchorBox | null>(null);
  const [anchorFound, setAnchorFound] = useState(false);
  const actionRef = useRef<HTMLButtonElement | null>(null);
  const previousFocus = useRef<HTMLElement | null>(null);
  const autoStartTimer = useRef<number | null>(null);
  const { topId } = useOverlayContext();

  const persist = useCallback((next: StoredTutorialState) => {
    setStored(next);
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    } catch {
      // Tutorial progress is optional presentation state. Storage failure must
      // never block study workflows or durable content.
    }
  }, []);

  const close = useCallback(() => {
    setActive(null);
    setAnchorBox(null);
    setAnchorFound(false);
    window.requestAnimationFrame(() => previousFocus.current?.focus());
  }, []);

  const dismiss = useCallback(() => {
    if (!active) return;
    const module = tutorialModules[active.moduleId];
    persist({
      ...stored,
      dismissed: { ...stored.dismissed, [module.id]: module.version },
    });
    close();
  }, [active, close, persist, stored]);

  const start = useCallback(
    (moduleId: TutorialModuleId) => {
      if (!enabled) return;
      previousFocus.current =
        document.activeElement instanceof HTMLElement ? document.activeElement : null;
      setActive({ moduleId, stepIndex: 0 });
    },
    [enabled],
  );

  const restart = useCallback(
    (moduleId?: TutorialModuleId) => {
      const next = moduleId
        ? {
            ...stored,
            completed: { ...stored.completed, [moduleId]: undefined },
            dismissed: { ...stored.dismissed, [moduleId]: undefined },
          }
        : EMPTY_STATE;
      persist(next);
      if (moduleId) start(moduleId);
    },
    [persist, start, stored],
  );

  useEffect(() => {
    if (!enabled) {
      close();
      return;
    }

    function handleStart(event: Event): void {
      const detail = (event as CustomEvent<{ moduleId?: TutorialModuleId }>).detail;
      const moduleId = detail?.moduleId;
      if (moduleId && moduleId in tutorialModules) start(moduleId);
    }
    window.addEventListener("ostudy:start-tutorial", handleStart);
    return () => window.removeEventListener("ostudy:start-tutorial", handleStart);
  }, [close, enabled, start]);

  useEffect(() => {
    if (!enabled || active || !CONTEXTUAL_AUTO_START_ENABLED) return;

    function findContextualModule(): TutorialModuleId | null {
      for (const moduleId of tutorialModuleOrder) {
        const module = tutorialModules[moduleId];
        if (stored.completed[moduleId] === module.version) continue;
        if (stored.dismissed[moduleId] === module.version) continue;
        if (document.querySelector(`[data-tour-anchor="${module.autoStartAnchor}"]`)) {
          return moduleId;
        }
      }
      return null;
    }

    function schedule(): void {
      if (autoStartTimer.current !== null) return;
      const moduleId = findContextualModule();
      if (!moduleId) return;
      autoStartTimer.current = window.setTimeout(() => {
        autoStartTimer.current = null;
        if (!active && document.visibilityState !== "hidden") start(moduleId);
      }, moduleId === "orientation" ? 700 : 500);
    }

    schedule();
    const observer = new MutationObserver(schedule);
    observer.observe(document.body, { childList: true, subtree: true });
    return () => {
      observer.disconnect();
      if (autoStartTimer.current !== null) {
        window.clearTimeout(autoStartTimer.current);
        autoStartTimer.current = null;
      }
    };
  }, [active, enabled, start, stored.completed, stored.dismissed]);

  const module = active ? tutorialModules[active.moduleId] : null;
  const step = module?.steps[active?.stepIndex ?? 0] ?? null;

  useLayoutEffect(() => {
    if (!step) return;
    const anchor = step.anchor;
    let target: HTMLElement | null = null;

    function update(): void {
      target = document.querySelector<HTMLElement>(`[data-tour-anchor="${anchor}"]`);
      if (!target) {
        setAnchorFound(false);
        setAnchorBox(null);
        return;
      }
      const rect = target.getBoundingClientRect();
      setAnchorFound(rect.width > 0 || rect.height > 0);
      setAnchorBox({
        top: rect.top,
        left: rect.left,
        width: rect.width,
        height: rect.height,
      });
    }

    update();
    const observer = new MutationObserver(update);
    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["hidden", "style", "class"],
    });
    window.addEventListener("resize", update);
    window.addEventListener("scroll", update, true);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", update);
      window.removeEventListener("scroll", update, true);
    };
  }, [step]);

  useEffect(() => {
    if (!active) return;
    actionRef.current?.focus();
    return registerPlatformBackHandler(() => {
      dismiss();
      return true;
    });
  }, [active, dismiss]);

  useEffect(() => {
    if (!active || topId !== null) return;
    function handleKeyDown(event: KeyboardEvent): void {
      if (event.key !== "Escape") return;
      event.preventDefault();
      dismiss();
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [active, dismiss, topId]);

  function completeModule(): void {
    if (!module) return;
    persist({
      ...stored,
      completed: { ...stored.completed, [module.id]: module.version },
    });
    close();
  }

  function next(): void {
    if (!active || !module) return;
    if (active.stepIndex >= module.steps.length - 1) {
      completeModule();
      return;
    }
    setActive({ ...active, stepIndex: active.stepIndex + 1 });
  }

  function previous(): void {
    if (!active || active.stepIndex === 0) return;
    setActive({ ...active, stepIndex: active.stepIndex - 1 });
  }

  function openHelp(): void {
    if (!step?.helpTopicId) return;
    window.dispatchEvent(
      new CustomEvent("ostudy:open-help", {
        detail: { topicId: step.helpTopicId },
      }),
    );
  }

  const value = useMemo<TourContextValue>(
    () => ({
      activeModuleId: active?.moduleId ?? null,
      start,
      dismiss,
      restart,
    }),
    [active?.moduleId, dismiss, restart, start],
  );

  return (
    <TourContext.Provider value={value}>
      {children}
      {active && module && step
        ? createPortal(
            <div className={styles.layer} data-anchor-found={anchorFound || undefined}>
              {anchorBox ? (
                <div className={styles.spotlight} style={spotlightStyle(anchorBox)} />
              ) : null}
              <section
                className={styles.popover}
                role="dialog"
                aria-modal="false"
                aria-labelledby="ostudy-tour-title"
                aria-describedby="ostudy-tour-body"
                style={popoverStyle(anchorBox, step.placement ?? "auto")}
              >
                <header className={styles.header}>
                  <span className={styles.moduleTitle}>{module.title}</span>
                  <button
                    type="button"
                    className={styles.close}
                    aria-label="Close tutorial"
                    onClick={dismiss}
                  >
                    <Icon name="close" size={18} />
                  </button>
                </header>
                <div
                  className={styles.progress}
                  aria-label={`Step ${active.stepIndex + 1} of ${module.steps.length}`}
                >
                  {module.steps.map((candidate, index) => (
                    <span
                      key={candidate.id}
                      data-current={index === active.stepIndex || undefined}
                      data-complete={index < active.stepIndex || undefined}
                    />
                  ))}
                </div>
                <h2 id="ostudy-tour-title">{step.title}</h2>
                <p id="ostudy-tour-body">{step.body}</p>
                {!anchorFound ? (
                  <p className={styles.anchorNotice}>
                    This control is not visible in the current layout. Continue to keep the guide moving.
                  </p>
                ) : null}
                <footer className={styles.actions}>
                  <button type="button" className={styles.skip} onClick={dismiss}>
                    Skip tutorial
                  </button>
                  {step.helpTopicId ? (
                    <button type="button" className={styles.help} onClick={openHelp}>
                      More help
                    </button>
                  ) : null}
                  <span className={styles.spacer} />
                  {active.stepIndex > 0 ? (
                    <Button onClick={previous}>Back</Button>
                  ) : null}
                  <Button ref={actionRef} variant="primary" onClick={next}>
                    {active.stepIndex === module.steps.length - 1 ? "Finish" : "Next"}
                  </Button>
                </footer>
              </section>
            </div>,
            document.body,
          )
        : null}
    </TourContext.Provider>
  );
}

export function useTour(): TourContextValue {
  return useContext(TourContext);
}

export function TourAnchor({
  id,
  children,
}: {
  id: string;
  children: ReactElement<Record<string, unknown>>;
}) {
  return cloneElement(children, { "data-tour-anchor": id });
}

export function startTutorial(moduleId: TutorialModuleId): void {
  window.dispatchEvent(
    new CustomEvent("ostudy:start-tutorial", { detail: { moduleId } }),
  );
}

function spotlightStyle(box: AnchorBox): CSSProperties {
  const margin = 8;
  return {
    top: Math.max(4, box.top - margin),
    left: Math.max(4, box.left - margin),
    width: Math.max(1, box.width + margin * 2),
    height: Math.max(1, box.height + margin * 2),
  };
}

function popoverStyle(
  box: AnchorBox | null,
  placement: TutorialPlacement,
): CSSProperties {
  if (!box) {
    return { top: "50%", left: "50%", transform: "translate(-50%, -50%)" };
  }
  const viewportWidth = window.innerWidth;
  const viewportHeight = window.innerHeight;
  const panelWidth = Math.min(360, Math.max(280, viewportWidth - 32));
  const panelHeight = 300;
  const gap = 14;
  const preferred = placement === "auto" ? choosePlacement(box, panelWidth, panelHeight) : placement;
  let top = box.top;
  let left = box.left + box.width / 2 - panelWidth / 2;
  if (preferred === "top") top = box.top - panelHeight - gap;
  if (preferred === "bottom") top = box.top + box.height + gap;
  if (preferred === "start") {
    top = box.top + box.height / 2 - panelHeight / 2;
    left = box.left - panelWidth - gap;
  }
  if (preferred === "end") {
    top = box.top + box.height / 2 - panelHeight / 2;
    left = box.left + box.width + gap;
  }
  return {
    width: panelWidth,
    top: clamp(top, 16, Math.max(16, viewportHeight - panelHeight - 16)),
    left: clamp(left, 16, Math.max(16, viewportWidth - panelWidth - 16)),
  };
}

function choosePlacement(
  box: AnchorBox,
  panelWidth: number,
  panelHeight: number,
): TutorialPlacement {
  if (window.innerHeight - (box.top + box.height) >= panelHeight + 20) return "bottom";
  if (box.top >= panelHeight + 20) return "top";
  if (window.innerWidth - (box.left + box.width) >= panelWidth + 20) return "end";
  return "start";
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}

function readStoredState(): StoredTutorialState {
  try {
    const serialized = window.localStorage.getItem(STORAGE_KEY);
    if (!serialized) return EMPTY_STATE;
    const parsed = JSON.parse(serialized) as Partial<StoredTutorialState>;
    if (parsed.schemaVersion !== 1) return EMPTY_STATE;
    return {
      schemaVersion: 1,
      completed: parsed.completed ?? {},
      dismissed: parsed.dismissed ?? {},
    };
  } catch {
    return EMPTY_STATE;
  }
}
