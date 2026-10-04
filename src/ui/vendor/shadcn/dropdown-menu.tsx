// Adapted from shadcn/ui Base UI dropdown-menu; see README.md.
import { Menu } from "@base-ui/react/menu";
import { cn } from "../../internal/cn";
export const DropdownMenu = Menu.Root;
export const DropdownMenuTrigger = Menu.Trigger;
export function DropdownMenuContent({ className, ...props }: Omit<Menu.Popup.Props, "className"> & { className?: string }) {
  return <Menu.Portal><Menu.Positioner side="bottom" align="end" sideOffset={6} className="tw:isolate tw:z-[500] tw:outline-none">
    <Menu.Popup {...props} data-slot="dropdown-menu-content" className={cn("tw:max-h-(--available-height) tw:min-w-52 tw:max-w-[calc(100vw-2rem)] tw:overflow-auto tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-popover tw:p-1.5 tw:font-sans tw:text-popover-foreground tw:shadow-overlay tw:outline-none", className)} />
  </Menu.Positioner></Menu.Portal>;
}
export function DropdownMenuItem({ className, destructive = false, ...props }: Omit<Menu.Item.Props, "className"> & { className?: string; destructive?: boolean }) {
  return <Menu.Item {...props} data-slot="dropdown-menu-item" className={cn("tw:flex tw:min-h-10 tw:cursor-default tw:items-center tw:rounded-md tw:px-3 tw:py-2 tw:text-small tw:font-medium tw:outline-none tw:select-none tw:data-highlighted:bg-accent tw:data-highlighted:text-accent-foreground tw:data-disabled:pointer-events-none tw:data-disabled:opacity-50 tw:coarse:min-h-touch", destructive && "tw:text-destructive tw:data-highlighted:bg-destructive-soft tw:data-highlighted:text-destructive", className)} />;
}
export function DropdownMenuSeparator() { return <Menu.Separator className="tw:my-1.5 tw:h-px tw:bg-border" />; }
