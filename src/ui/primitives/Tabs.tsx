import { useEffect, useState, type ReactNode } from "react";
import { cn } from "../internal/cn";
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

export interface PaneTab<T extends string> { value: T; label: string; icon?: ReactNode; panel: ReactNode; /** Starts a separate group below a gap. */ separated?: boolean; }
/**
 * Sections listed down a shaded pane beside the open one, as Settings shows them on wider screens.
 * The pane carries the page heading; `header` sits above the open section. Panels stay mounted
 * once rendered, so a section's unsaved draft survives switching to another one.
 */
export function PaneTabs<T extends string>({ value, onValueChange, tabs, ariaLabel, heading, header }: {
  value: T; onValueChange: (value: T) => void; tabs: readonly PaneTab<T>[]; ariaLabel: string; heading: ReactNode; header?: ReactNode;
}) {
  return <TabsRoot value={value} orientation="vertical" onValueChange={(next) => { const tab = tabs.find((item) => item.value === next); if (tab) onValueChange(tab.value); }}
    className="tw:grid tw:h-full tw:min-h-0 tw:grid-cols-[minmax(12rem,15rem)_minmax(0,1fr)]">
    <div className="tw:flex tw:min-h-0 tw:min-w-0 tw:flex-col tw:overflow-y-auto tw:border-0 tw:border-r tw:border-solid tw:border-border/70 tw:bg-foreground/4 tw:p-3">
      {heading}
      <TabsList aria-label={ariaLabel} activateOnFocus className="tw:flex tw:w-full tw:min-w-0 tw:flex-col tw:items-stretch tw:gap-0.5 tw:bg-transparent tw:p-0">
        {tabs.map((tab) => <TabsTrigger key={tab.value} value={tab.value}
          className={cn("tw:h-11 tw:w-full tw:justify-start tw:gap-3 tw:rounded-lg tw:px-3 tw:text-body tw:text-muted-foreground tw:data-active:bg-background tw:data-active:font-semibold tw:data-active:text-primary tw:data-active:shadow-xs tw:data-active:ring-0", tab.separated && "tw:mt-3")}>
          {tab.icon}{tab.label}
        </TabsTrigger>)}
      </TabsList>
    </div>
    <div className="tw:flex tw:min-h-0 tw:min-w-0 tw:flex-col">
      {header}
      {tabs.map((tab) => <TabsContent key={tab.value} value={tab.value} keepMounted className={tab.value === value ? "tw:flex tw:min-h-0 tw:flex-1 tw:flex-col" : undefined}>
        {tab.panel}
      </TabsContent>)}
    </div>
  </TabsRoot>;
}
