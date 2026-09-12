import type { ReactNode } from "react";
import { Icon, type IconName } from "../icons/Icon";
import { useLayoutMode } from "./LayoutModeProvider";
import { Sidebar, SidebarContent, SidebarGroup, SidebarGroupLabel, SidebarHeader, SidebarMenu, SidebarMenuButton, SidebarMenuItem } from "../vendor/shadcn/sidebar";
import { cn } from "../internal/cn";

export interface NavigationItem { id: string; label: string; icon: IconName; active: boolean; onSelect: () => void; secondary?: boolean; }
export function AppNavigation({ items, brand }: { items: readonly NavigationItem[]; brand: ReactNode }) {
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const content = (group: readonly NavigationItem[]) => group.map((item) => <SidebarMenuItem key={item.id}>
    <SidebarMenuButton isActive={item.active} data-testid={`nav-${item.id}`} onClick={item.onSelect}
      className={compact ? "tw:grid tw:min-h-14 tw:justify-items-center tw:gap-1 tw:px-1 tw:py-1.5 tw:text-center tw:text-[0.65rem] tw:font-medium" : undefined}>
      <Icon name={item.icon} size={20} /><span>{item.label}</span>
    </SidebarMenuButton>
  </SidebarMenuItem>);
  if (compact) return <nav aria-label="Main navigation" data-tour-anchor="app.navigation" data-slot="bottom-navigation" className="tw:fixed tw:inset-x-0 tw:bottom-0 tw:z-20 tw:border-t tw:border-solid tw:border-border tw:bg-background tw:pt-1 tw:pr-[max(0.5rem,var(--safe-right))] tw:pb-[max(0.25rem,var(--safe-bottom))] tw:pl-[max(0.5rem,var(--safe-left))]">
    <SidebarMenu className="tw:grid tw:grid-cols-5 tw:gap-1">{content(items.filter((item) => !item.secondary))}</SidebarMenu>
  </nav>;
  return <Sidebar><SidebarHeader>{brand}</SidebarHeader>
    <SidebarContent aria-label="Main navigation" data-tour-anchor="app.navigation">
      <SidebarGroup><SidebarGroupLabel>Workspace</SidebarGroupLabel><SidebarMenu>{content(items.filter((item) => !item.secondary))}</SidebarMenu></SidebarGroup>
      <SidebarGroup className={cn("tw:mt-auto tw:border-t tw:border-solid tw:border-border tw:pt-3", layoutMode === "medium" && "tw:text-sm")}><SidebarMenu>{content(items.filter((item) => item.secondary))}</SidebarMenu></SidebarGroup>
    </SidebarContent>
  </Sidebar>;
}
