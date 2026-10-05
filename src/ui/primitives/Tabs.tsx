import { useEffect, useState, type ReactNode } from "react";
import { TabsRoot, TabsList, TabsTrigger, TabsContent } from "../vendor/shadcn/tabs";

export interface TabDefinition<T extends string> {
  value: T;
  label: string;
  panel: ReactNode;
  /** Hidden destinations mount lazily once, then preserve their unsaved drafts. */
  hiddenFromTabList?: boolean;
}
interface TabsProps<T extends string> {
  value: T;
  onValueChange: (value: T) => void;
  tabs: ReadonlyArray<TabDefinition<T>>;
  ariaLabel: string;
}
export function Tabs<T extends string>({ value, onValueChange, tabs, ariaLabel }: TabsProps<T>) {
  const [visited, setVisited] = useState<ReadonlySet<T>>(() => new Set());
  useEffect(() => {
    if (!visited.has(value)) setVisited((previous) => new Set(previous).add(value));
  }, [value, visited]);
  return (
    <TabsRoot value={value} onValueChange={(next) => {
      const tab = tabs.find((item) => item.value === next);
      if (tab) onValueChange(tab.value);
    }} className="tw:flex tw:min-w-0 tw:flex-col tw:gap-6">
      <div className="tw:min-w-0 tw:overflow-x-auto tw:p-1">
        <TabsList aria-label={ariaLabel} activateOnFocus>
          {tabs.filter((tab) => !tab.hiddenFromTabList).map((tab) => <TabsTrigger key={tab.value} value={tab.value}>{tab.label}</TabsTrigger>)}
        </TabsList>
      </div>
      {tabs.map((tab) => <TabsContent key={tab.value} value={tab.value} keepMounted aria-label={tab.hiddenFromTabList ? tab.label : undefined}>
        {!tab.hiddenFromTabList || tab.value === value || visited.has(tab.value) ? tab.panel : null}
      </TabsContent>)}
    </TabsRoot>
  );
}

export interface WorkspaceTab<T extends string> { value: T; label: string; compactLabel?: string; icon?: ReactNode; }
export function WorkspaceTabs<T extends string>({ value, onValueChange, tabs, ariaLabel, tourAnchor, children }: {
  value: T; onValueChange: (value: T) => void; tabs: readonly WorkspaceTab<T>[]; ariaLabel: string; tourAnchor?: string; children: ReactNode;
}) {
  return <TabsRoot value={value} onValueChange={(next) => { const tab = tabs.find((item) => item.value === next); if (tab) onValueChange(tab.value); }} className="tw:grid tw:min-w-0 tw:gap-6">
    <TabsList aria-label={ariaLabel} data-tour-anchor={tourAnchor} activateOnFocus className="tw:compact:w-full tw:compact:min-w-0">
      {tabs.map((tab) => <TabsTrigger key={tab.value} value={tab.value} aria-label={tab.label} onClick={() => { if (tab.value === value) onValueChange(tab.value); }} className="tw:compact:min-w-0 tw:compact:flex-1">
        {tab.icon}<span className={tab.compactLabel ? "tw:compact:hidden" : undefined}>{tab.label}</span>{tab.compactLabel ? <span className="tw:hidden tw:compact:inline" aria-hidden="true">{tab.compactLabel}</span> : null}
      </TabsTrigger>)}
    </TabsList>
    <TabsContent value={value}>{children}</TabsContent>
  </TabsRoot>;
}
