import type { HTMLAttributes, ReactNode } from "react";

import { cn } from "../internal/cn";
import { SidebarInset } from "../vendor/shadcn/sidebar";
import { useLayoutMode } from "./LayoutModeProvider";

/**
 * Layout alone: navigation, workflows and provider lifetimes remain app-owned.
 * Wider screens stack the header above one scrolling main area. A phone keeps
 * the header and the bottom navigation fixed and pads the main area between
 * them.
 */
export function AppFrame({ children }: { children: ReactNode }) {
  const { layoutMode } = useLayoutMode();
  return (
    <div
      data-ui-root="true"
      data-frame-layout={layoutMode}
      className={cn(
        "tw:relative tw:z-(--ostudy-layer-base) tw:h-dvh tw:w-full tw:min-w-0 tw:overflow-hidden tw:bg-background tw:font-sans tw:text-foreground",
        // The space below a phone's content is the bottom navigation's own height (its 1px top
        // border, 0.375rem top padding, 3.625rem items and the bottom padding that grows into the
        // safe area, as AppNavigation draws it), so it follows the interface scale.
        layoutMode === "compact"
          ? "tw:block tw:pt-[calc(var(--ostudy-navigation-app-bar-height)+var(--safe-top))] tw:pb-[calc(1px+4rem+max(0.5rem,var(--safe-bottom)))] tw:[[data-android-ime-visible=true]_&]:pb-(--android-ime-bottom)"
          : "tw:grid tw:grid-rows-[auto_minmax(0,1fr)]",
      )}
    >
      {children}
    </div>
  );
}

export function AppContent({ children }: { children: ReactNode }) {
  return <SidebarInset className="tw:h-full tw:w-full">{children}</SidebarInset>;
}

interface AppHeaderProps extends Omit<HTMLAttributes<HTMLElement>, "title"> {
  brand: ReactNode;
  /** A phone's header names the current place between the brand and the actions. */
  title?: ReactNode;
  /** Wider screens carry the main navigation in the header, centered. */
  navigation?: ReactNode;
  actions: ReactNode;
}

/**
 * The application header: the brand at the start and the header actions at the
 * end. On wider screens the main navigation sits between them, and moves to a
 * second row when a large interface scale leaves no room for one; on a phone
 * the header is a fixed bar and the navigation moves to the bottom of the
 * screen.
 */
export function AppHeader({ brand, title, navigation, actions, className, ...props }: AppHeaderProps) {
  const { layoutMode } = useLayoutMode();
  if (layoutMode === "compact") {
    return (
      <header
        {...props}
        data-slot="app-header"
        className={cn(
          "tw:fixed tw:inset-x-0 tw:top-0 tw:z-(--ostudy-layer-chrome) tw:grid tw:min-h-[calc(var(--ostudy-navigation-app-bar-height)+var(--safe-top))] tw:grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] tw:items-center tw:gap-2 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-background tw:pt-(--safe-top) tw:pr-[max(1rem,var(--safe-right))] tw:pb-1 tw:pl-[max(1rem,var(--safe-left))]",
          className,
        )}
      >
        <div className="tw:flex tw:min-w-0 tw:justify-start">{brand}</div>
        <div className="tw:min-w-0 tw:truncate tw:text-center tw:text-small tw:font-semibold tw:text-foreground">
          {title}
        </div>
        <div className="tw:flex tw:min-w-0 tw:items-center tw:justify-end tw:gap-2">{actions}</div>
      </header>
    );
  }
  const layout = layoutMode === "medium" ? MEDIUM_HEADER : WIDE_HEADER;
  return (
    <header
      {...props}
      data-slot="app-header"
      className={cn(
        "tw:@container/app-header tw:relative tw:z-(--ostudy-layer-chrome) tw:pt-[max(1.125rem,var(--safe-top))] tw:pb-2.5",
        layout.header,
        className,
      )}
    >
      <div className={cn("tw:grid tw:grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] tw:items-center tw:gap-x-4 tw:gap-y-3", layout.grid)}>
        <div className="tw:col-start-1 tw:row-start-1 tw:flex tw:min-w-0 tw:justify-start">{brand}</div>
        <div className={cn("tw:col-start-2 tw:row-start-1 tw:flex tw:min-w-0 tw:justify-center", layout.navigation)}>
          {navigation}
        </div>
        <div className={cn("tw:col-start-3 tw:row-start-1 tw:flex tw:min-w-0 tw:items-center tw:justify-end tw:gap-2", layout.actions)}>
          {actions}
        </div>
      </div>
    </header>
  );
}

/*
 * One row needs about 38rem in the medium layout (the brand mark alone and
 * tighter items) and 50rem in the wide one. Below that, at a large interface
 * scale, the navigation takes a second row under the brand and the actions.
 */
const MEDIUM_HEADER = {
  header: "tw:pr-[max(1.25rem,var(--safe-right))] tw:pl-[max(1.25rem,var(--safe-left))]",
  grid: "tw:@max-[38rem]/app-header:grid-cols-[minmax(0,1fr)_auto]",
  navigation: "tw:@max-[38rem]/app-header:col-span-2 tw:@max-[38rem]/app-header:col-start-1 tw:@max-[38rem]/app-header:row-start-2",
  actions: "tw:@max-[38rem]/app-header:col-start-2",
};

const WIDE_HEADER = {
  header: "tw:pr-[max(2rem,var(--safe-right))] tw:pl-[max(2rem,var(--safe-left))]",
  grid: "tw:@max-[50rem]/app-header:grid-cols-[minmax(0,1fr)_auto]",
  navigation: "tw:@max-[50rem]/app-header:col-span-2 tw:@max-[50rem]/app-header:col-start-1 tw:@max-[50rem]/app-header:row-start-2",
  actions: "tw:@max-[50rem]/app-header:col-start-2",
};
