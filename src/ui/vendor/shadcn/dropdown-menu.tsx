// Adapted from shadcn/ui Base UI dropdown-menu; see README.md.
import { Menu } from "@base-ui/react/menu";
import { cn } from "../../internal/cn";
export const DropdownMenu = Menu.Root;
export const DropdownMenuTrigger = Menu.Trigger;
export function DropdownMenuContent({ className, anchor, ...props }: Omit<Menu.Popup.Props, "className"> & { className?: string; anchor?: Menu.Positioner.Props["anchor"] }) {
  // A menu with an anchor opens from that point, such as where a right-click was.
  return <Menu.Portal><Menu.Positioner side="bottom" align={anchor ? "start" : "end"} sideOffset={anchor ? 2 : 6} anchor={anchor} className="tw:isolate tw:z-(--ostudy-layer-popover) tw:outline-none">
    <Menu.Popup {...props} data-slot="dropdown-menu-content" className={cn("tw:max-h-(--available-height) tw:min-w-52 tw:max-w-[calc(100vw-2rem)] tw:overflow-auto tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-popover tw:p-1.5 tw:font-sans tw:text-popover-foreground tw:shadow-overlay tw:outline-none", className)} />
  </Menu.Positioner></Menu.Portal>;
}
export function DropdownMenuItem({ className, destructive = false, ...props }: Omit<Menu.Item.Props, "className"> & { className?: string; destructive?: boolean }) {
  return <Menu.Item {...props} data-slot="dropdown-menu-item" className={cn("tw:flex tw:min-h-10 tw:cursor-default tw:items-center tw:rounded-md tw:px-3 tw:py-2 tw:text-small tw:font-medium tw:outline-none tw:select-none tw:data-highlighted:bg-accent tw:data-highlighted:text-accent-foreground tw:data-disabled:pointer-events-none tw:data-disabled:opacity-50 tw:coarse:min-h-touch", destructive && "tw:text-destructive tw:data-highlighted:bg-destructive-soft tw:data-highlighted:text-destructive", className)} />;
}
/** An item that shows whether it is the current choice, such as a sort order. */
export function DropdownMenuCheckboxItem({ className, children, ...props }: Omit<Menu.CheckboxItem.Props, "className"> & { className?: string }) {
  return <Menu.CheckboxItem {...props} data-slot="dropdown-menu-checkbox-item" className={cn("tw:flex tw:min-h-10 tw:cursor-default tw:items-center tw:gap-2 tw:rounded-md tw:py-2 tw:pr-3 tw:pl-2 tw:text-small tw:font-medium tw:outline-none tw:select-none tw:data-highlighted:bg-accent tw:data-highlighted:text-accent-foreground tw:data-disabled:pointer-events-none tw:data-disabled:opacity-50 tw:coarse:min-h-touch", className)}>
    <span aria-hidden="true" className="tw:grid tw:size-4 tw:shrink-0 tw:place-items-center">
      <Menu.CheckboxItemIndicator><svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M5 12l5 5 9-10" /></svg></Menu.CheckboxItemIndicator>
    </span>
    {children}
  </Menu.CheckboxItem>;
}
export function DropdownMenuSeparator() { return <Menu.Separator className="tw:my-1.5 tw:h-px tw:bg-border" />; }
export const DropdownMenuSub = Menu.SubmenuRoot;
export function DropdownMenuSubTrigger({ className, children, ...props }: Omit<Menu.SubmenuTrigger.Props, "className"> & { className?: string }) {
  return <Menu.SubmenuTrigger {...props} data-slot="dropdown-menu-sub-trigger" className={cn("tw:flex tw:min-h-10 tw:cursor-default tw:items-center tw:justify-between tw:gap-3 tw:rounded-md tw:px-3 tw:py-2 tw:text-small tw:font-medium tw:outline-none tw:select-none tw:data-highlighted:bg-accent tw:data-highlighted:text-accent-foreground tw:data-popup-open:bg-accent tw:data-disabled:pointer-events-none tw:data-disabled:opacity-50 tw:coarse:min-h-touch", className)}>
    {children}
    <svg aria-hidden="true" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M9 6l6 6-6 6" /></svg>
  </Menu.SubmenuTrigger>;
}
export function DropdownMenuSubContent({ className, ...props }: Omit<Menu.Popup.Props, "className"> & { className?: string }) {
  return <Menu.Portal><Menu.Positioner side="right" align="start" sideOffset={4} className="tw:isolate tw:z-(--ostudy-layer-popover) tw:outline-none">
    <Menu.Popup {...props} data-slot="dropdown-menu-sub-content" className={cn("tw:max-h-(--available-height) tw:min-w-44 tw:max-w-[calc(100vw-2rem)] tw:overflow-auto tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-popover tw:p-1.5 tw:font-sans tw:text-popover-foreground tw:shadow-overlay tw:outline-none", className)} />
  </Menu.Positioner></Menu.Portal>;
}
