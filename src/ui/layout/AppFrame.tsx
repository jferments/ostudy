import type { HTMLAttributes, ReactNode } from "react";

import { cn } from "../internal/cn";
import { SidebarInset } from "../vendor/shadcn/sidebar";
import { useLayoutMode } from "./LayoutModeProvider";

/** Layout alone: navigation, workflows and provider lifetimes remain app-owned. */
export function AppFrame({ children }: { children: ReactNode }) {
  const { layoutMode } = useLayoutMode();
  return (
    <div
      data-ui-root="true"
      data-frame-layout={layoutMode}
      className={cn(
        "tw:relative tw:z-1 tw:grid tw:h-dvh tw:w-full tw:min-w-0 tw:overflow-hidden tw:bg-sidebar tw:font-sans tw:text-foreground",
        layoutMode === "compact"
          ? "tw:block tw:pt-[calc(var(--ostudy-navigation-app-bar-height)+var(--safe-top))] tw:pb-[calc(var(--ostudy-navigation-compact-height)+var(--safe-bottom))] tw:[[data-android-ime-visible=true]_&]:pb-(--android-ime-bottom)"
          : layoutMode === "medium"
            ? "tw:grid-cols-[12rem_minmax(0,1fr)] tw:p-2 tw:pl-0"
            : "tw:grid-cols-[15rem_minmax(0,1fr)] tw:p-2 tw:pl-0",
      )}
    >
      {children}
    </div>
  );
}

export function AppContent({ children }: { children: ReactNode }) {
  const { layoutMode } = useLayoutMode();
  return (
    <SidebarInset
      className={cn(
        "tw:h-full tw:w-full",
        layoutMode !== "compact" && "tw:rounded-xl tw:ring-1 tw:ring-sidebar-border",
      )}
    >
      {children}
    </SidebarInset>
  );
}

export function CompactAppHeader({ className, ...props }: HTMLAttributes<HTMLElement>) {
  return (
    <header
      {...props}
      className={cn(
        "tw:fixed tw:inset-x-0 tw:top-0 tw:z-20 tw:hidden tw:min-h-[calc(var(--ostudy-navigation-app-bar-height)+var(--safe-top))] tw:grid-cols-[minmax(70px,1fr)_auto_minmax(70px,1fr)] tw:items-center tw:gap-2 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-background tw:pt-(--safe-top) tw:pr-[max(1rem,var(--safe-right))] tw:pb-1 tw:pl-[max(1rem,var(--safe-left))] tw:max-[760px]:grid",
        className,
      )}
    />
  );
}
