import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { IconButton } from "../icons/IconButton";
import { DialogActions, Sheet } from "../overlays/Dialog";
import { Button } from "../primitives/Button";
import { startTutorial } from "../tutorials/TourProvider";
import { helpRegistry, type HelpTopicId } from "./helpRegistry";

interface HelpContextValue { open(topicId: HelpTopicId): void; close(): void; }
const HelpContext = createContext<HelpContextValue>({ open: () => undefined, close: () => undefined });

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

  return <HelpContext.Provider value={value}>
    {children}
    {topic ? <Sheet title={topic.title} description={topic.summary} width="small" onClose={close}>
      <div className="tw:grid tw:min-w-0 tw:gap-5 tw:p-4 tw:sm:p-5">
        <p className="tw:m-0 tw:break-words tw:text-label tw:leading-relaxed">{topic.details}</p>
        <code className="tw:block tw:min-w-0 tw:whitespace-normal tw:break-all tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-muted tw:p-3 tw:text-caption tw:text-muted-foreground">{topic.documentation}</code>
        <DialogActions>
          {topic.tutorialModuleId ? <Button onClick={() => {
            const moduleId = topic.tutorialModuleId;
            close();
            if (moduleId) window.requestAnimationFrame(() => startTutorial(moduleId));
          }}>Start guided tutorial</Button> : null}
          <Button variant="primary" onClick={close}>Done</Button>
        </DialogActions>
      </div>
    </Sheet> : null}
  </HelpContext.Provider>;
}

export function HelpTrigger({ topicId, label }: { topicId: HelpTopicId; label?: string }) {
  const help = useContext(HelpContext);
  const topic = helpRegistry[topicId];
  return <IconButton icon="information" iconSize={16} label={label ?? `Help: ${topic.title}`} tooltip={topic.summary} onClick={() => help.open(topicId)} />;
}
export function useHelp(): HelpContextValue { return useContext(HelpContext); }
