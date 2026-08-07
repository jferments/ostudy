import { useId, useRef, type KeyboardEvent, type ReactNode } from "react";

import styles from "./Tabs.module.css";

export interface TabDefinition<T extends string> {
  value: T;
  label: string;
  panel: ReactNode;
}

interface TabsProps<T extends string> {
  value: T;
  onValueChange: (value: T) => void;
  tabs: ReadonlyArray<TabDefinition<T>>;
  ariaLabel: string;
}

export function Tabs<T extends string>({ value, onValueChange, tabs, ariaLabel }: TabsProps<T>) {
  const rootId = useId();
  const buttonsRef = useRef<Array<HTMLButtonElement | null>>([]);

  function activateAt(index: number): void {
    const tab = tabs[index];
    if (!tab) return;
    onValueChange(tab.value);
    buttonsRef.current[index]?.focus();
  }

  function handleKeyDown(event: KeyboardEvent<HTMLButtonElement>, index: number): void {
    let nextIndex: number | null = null;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % tabs.length;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + tabs.length) % tabs.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = tabs.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    activateAt(nextIndex);
  }

  return (
    <div className={styles.root}>
      <div className={styles.tabListScroller}>
        <div className={styles.tabList} role="tablist" aria-label={ariaLabel}>
          {tabs.map((tab, index) => {
            const selected = tab.value === value;
            const tabId = `${rootId}-tab-${tab.value}`;
            const panelId = `${rootId}-panel-${tab.value}`;
            return (
              <button
                key={tab.value}
                ref={(node) => {
                  buttonsRef.current[index] = node;
                }}
                id={tabId}
                type="button"
                role="tab"
                aria-selected={selected}
                aria-controls={panelId}
                tabIndex={selected ? 0 : -1}
                className={styles.tab}
                data-selected={selected}
                onClick={() => onValueChange(tab.value)}
                onKeyDown={(event) => handleKeyDown(event, index)}
              >
                {tab.label}
              </button>
            );
          })}
        </div>
      </div>
      {tabs.map((tab) => {
        const selected = tab.value === value;
        return (
          <section
            key={tab.value}
            id={`${rootId}-panel-${tab.value}`}
            role="tabpanel"
            aria-labelledby={`${rootId}-tab-${tab.value}`}
            hidden={!selected}
            tabIndex={0}
            className={styles.panel}
          >
            {tab.panel}
          </section>
        );
      })}
    </div>
  );
}
