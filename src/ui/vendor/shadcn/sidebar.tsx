// Adapted from shadcn/ui Sidebar's non-collapsing Nova composition. See README.md.
// LayoutModeProvider owns device layout; no second mobile detector, cookie or hotkey.
import type { ComponentProps } from "react";
import { mergeProps } from "@base-ui/react/merge-props";
import { useRender } from "@base-ui/react/use-render";

import { cn } from "../../internal/cn";

export function Sidebar({ className, ...props }: ComponentProps<"aside">) {
  return <aside data-slot="sidebar" className={cn("tw:relative tw:z-10 tw:flex tw:min-h-0 tw:min-w-0 tw:flex-col tw:bg-sidebar tw:text-sidebar-foreground", className)} {...props} />;
}

export function SidebarHeader({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="sidebar-header" className={cn("tw:flex tw:shrink-0 tw:flex-col tw:gap-2 tw:px-4 tw:pt-[max(1.25rem,var(--safe-top))] tw:pb-4", className)} {...props} />;
}

export function SidebarContent({ className, ...props }: ComponentProps<"nav">) {
  return <nav data-slot="sidebar-content" className={cn("tw:flex tw:min-h-0 tw:flex-1 tw:flex-col tw:overflow-auto tw:px-2 tw:pb-[max(0.75rem,var(--safe-bottom))]", className)} {...props} />;
}

export function SidebarGroup({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="sidebar-group" className={cn("tw:relative tw:flex tw:w-full tw:min-w-0 tw:flex-col tw:gap-1", className)} {...props} />;
}

export function SidebarGroupLabel({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="sidebar-group-label" className={cn("tw:px-2 tw:py-1.5 tw:text-xs tw:font-medium tw:text-muted-foreground", className)} {...props} />;
}

export function SidebarMenu({ className, ...props }: ComponentProps<"ul">) {
  return <ul data-slot="sidebar-menu" className={cn("tw:m-0 tw:flex tw:w-full tw:min-w-0 tw:list-none tw:flex-col tw:gap-0.5 tw:p-0", className)} {...props} />;
}

export function SidebarMenuItem({ className, ...props }: ComponentProps<"li">) {
  return <li data-slot="sidebar-menu-item" className={cn("tw:relative tw:min-w-0", className)} {...props} />;
}

export function SidebarMenuButton({
  render,
  isActive = false,
  className,
  ...props
}: useRender.ComponentProps<"button"> & ComponentProps<"button"> & { isActive?: boolean }) {
  return useRender({
    defaultTagName: "button",
    render,
    props: mergeProps<"button">(
      {
        type: "button",
        "aria-current": isActive ? "page" : undefined,
        className: cn(
          "tw:flex tw:h-8 tw:w-full tw:cursor-pointer tw:items-center tw:gap-2 tw:overflow-hidden tw:rounded-lg tw:border-0 tw:bg-transparent tw:px-2 tw:text-start tw:font-sans tw:text-sm tw:font-medium tw:text-muted-foreground tw:outline-none tw:transition-colors tw:reduce-motion:transition-none tw:hover:bg-sidebar-accent tw:hover:text-sidebar-accent-foreground tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:disabled:pointer-events-none tw:disabled:opacity-50 tw:coarse:min-h-touch tw:coarse:gap-3 tw:coarse:px-3 tw:[&_svg]:shrink-0 tw:[&>span:last-child]:truncate",
          isActive && "tw:bg-sidebar-accent tw:text-sidebar-accent-foreground",
          className,
        ),
      },
      props,
    ),
    state: { slot: "sidebar-menu-button", sidebar: "menu-button", active: isActive },
  });
}

export function SidebarInset({ className, ...props }: ComponentProps<"main">) {
  return <main data-slot="sidebar-inset" className={cn("tw:relative tw:min-h-0 tw:min-w-0 tw:overflow-auto tw:overscroll-contain tw:bg-background", className)} {...props} />;
}
