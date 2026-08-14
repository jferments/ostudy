import {
  createContext,
  type ReactNode,
  useContext,
  useMemo,
  useState,
} from "react";

import { Sheet } from "../overlays/Dialog";
import { Button } from "../primitives/Button";
import { Icon } from "../icons/Icon";
import { helpRegistry, type HelpTopicId } from "./helpRegistry";
import styles from "./Help.module.css";

interface HelpContextValue {
  open(topicId: HelpTopicId): void;
  close(): void;
}

const HelpContext = createContext<HelpContextValue>({ open: () => undefined, close: () => undefined });

export function HelpProvider({ children }: { children: ReactNode }) {
  const [activeTopicId, setActiveTopicId] = useState<HelpTopicId | null>(null);
  const value = useMemo<HelpContextValue>(
    () => ({ open: setActiveTopicId, close: () => setActiveTopicId(null) }),
    [],
  );
  const topic = activeTopicId ? helpRegistry[activeTopicId] : null;
  return (
    <HelpContext.Provider value={value}>
      {children}
      {topic ? (
        <Sheet title={topic.title} description={topic.summary} width="small" onClose={() => setActiveTopicId(null)}>
          <div className={styles.sheetBody}>
            <p>{topic.details}</p>
            <code>{topic.documentation}</code>
            <Button variant="primary" onClick={() => setActiveTopicId(null)}>Done</Button>
          </div>
        </Sheet>
      ) : null}
    </HelpContext.Provider>
  );
}

export function HelpTrigger({ topicId, label = "Open help" }: { topicId: HelpTopicId; label?: string }) {
  const help = useContext(HelpContext);
  return (
    <button type="button" className={styles.helpButton} aria-label={label} onClick={() => help.open(topicId)}>
      <Icon name="help" size={16} />
    </button>
  );
}

export function useHelp(): HelpContextValue {
  return useContext(HelpContext);
}
