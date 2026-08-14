import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

import { Icon } from "../icons/Icon";
import { Sheet } from "../overlays/Dialog";
import { Button } from "../primitives/Button";
import { startTutorial } from "../tutorials/TourProvider";
import { Tooltip } from "./Tooltip";
import { helpRegistry, type HelpTopicId } from "./helpRegistry";
import styles from "./Help.module.css";

interface HelpContextValue {
  open(topicId: HelpTopicId): void;
  close(): void;
}

const HelpContext = createContext<HelpContextValue>({
  open: () => undefined,
  close: () => undefined,
});

export function HelpProvider({ children }: { children: ReactNode }) {
  const [activeTopicId, setActiveTopicId] = useState<HelpTopicId | null>(null);
  const open = useCallback((topicId: HelpTopicId) => setActiveTopicId(topicId), []);
  const close = useCallback(() => setActiveTopicId(null), []);
  const value = useMemo<HelpContextValue>(() => ({ open, close }), [close, open]);
  const topic = activeTopicId ? helpRegistry[activeTopicId] : null;

  useEffect(() => {
    function handleOpen(event: Event): void {
      const topicId = (event as CustomEvent<{ topicId?: string }>).detail?.topicId;
      if (!topicId || !(topicId in helpRegistry)) return;
      open(topicId as HelpTopicId);
    }
    window.addEventListener("ostudy:open-help", handleOpen);
    return () => window.removeEventListener("ostudy:open-help", handleOpen);
  }, [open]);

  return (
    <HelpContext.Provider value={value}>
      {children}
      {topic ? (
        <Sheet
          title={topic.title}
          description={topic.summary}
          width="small"
          onClose={close}
        >
          <div className={styles.sheetBody}>
            <p>{topic.details}</p>
            <code>{topic.documentation}</code>
            <div className={styles.sheetActions}>
              {topic.tutorialModuleId ? (
                <Button
                  onClick={() => {
                    const moduleId = topic.tutorialModuleId;
                    close();
                    if (moduleId) {
                      window.requestAnimationFrame(() => startTutorial(moduleId));
                    }
                  }}
                >
                  Start guided tutorial
                </Button>
              ) : null}
              <Button variant="primary" onClick={close}>
                Done
              </Button>
            </div>
          </div>
        </Sheet>
      ) : null}
    </HelpContext.Provider>
  );
}

export function HelpTrigger({
  topicId,
  label,
}: {
  topicId: HelpTopicId;
  label?: string;
}) {
  const help = useContext(HelpContext);
  const topic = helpRegistry[topicId];
  return (
    <Tooltip content={topic.summary}>
      <button
        type="button"
        className={styles.helpButton}
        aria-label={label ?? `Help: ${topic.title}`}
        title={topic.summary}
        onClick={() => help.open(topicId)}
      >
        <Icon name="help" size={16} />
      </button>
    </Tooltip>
  );
}

export function useHelp(): HelpContextValue {
  return useContext(HelpContext);
}
