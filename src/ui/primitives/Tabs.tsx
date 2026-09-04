import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";

import styles from "./Tabs.module.css";

export interface TabDefinition<T extends string> {
  value: T;
  label: string;
  panel: ReactNode;
  /**
   * Keeps a panel inside the shared settings workspace while exposing its entry
   * through a separate menu action instead of the primary tab strip. Hidden
   * panels mount only while selected so advanced tools do not perform work in
   * the background.
   */
  hiddenFromTabList?: boolean;
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
  const visibleTabs = tabs.filter((tab) => !tab.hiddenFromTabList);
  const hasVisibleSelection = visibleTabs.some((tab) => tab.value === value);
  const [mountedHiddenValues, setMountedHiddenValues] = useState<ReadonlySet<T>>(
    () => new Set(),
  );

  useEffect(() => {
    const selectedTab = tabs.find((tab) => tab.value === value);
    if (!selectedTab?.hiddenFromTabList || mountedHiddenValues.has(value)) return;
    setMountedHiddenValues((current) => new Set(current).add(value));
  }, [mountedHiddenValues, tabs, value]);

  function activateAt(index: number): void {
    const tab = visibleTabs[index];
    if (!tab) return;
    onValueChange(tab.value);
    buttonsRef.current[index]?.focus();
  }

  function handleKeyDown(event: KeyboardEvent<HTMLButtonElement>, index: number): void {
    let nextIndex: number | null = null;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % visibleTabs.length;
    if (event.key === "ArrowLeft") {
      nextIndex = (index - 1 + visibleTabs.length) % visibleTabs.length;
    }
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = visibleTabs.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    activateAt(nextIndex);
  }

  return (
    <div className={styles.root}>
      <div className={styles.tabListScroller}>
        <div className={styles.tabList} role="tablist" aria-label={ariaLabel}>
          {visibleTabs.map((tab, index) => {
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
                tabIndex={selected || (!hasVisibleSelection && index === 0) ? 0 : -1}
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
            aria-labelledby={tab.hiddenFromTabList ? undefined : `${rootId}-tab-${tab.value}`}
            aria-label={tab.hiddenFromTabList ? tab.label : undefined}
            hidden={!selected}
            tabIndex={0}
            className={styles.panel}
          >
            {tab.hiddenFromTabList && !selected && !mountedHiddenValues.has(tab.value)
              ? null
              : tab.panel}
          </section>
        );
      })}
    </div>
  );
}
