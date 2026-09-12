// Base UI toolbar behavior composed with the owned shadcn Button renderer.
// See README.md for the pinned upstream sources and license.
import { Toolbar as ToolbarPrimitive } from "@base-ui/react/toolbar";
import { Button } from "./button";
import { cn } from "../../internal/cn";

export function ToolbarRoot({ className, ...props }: ToolbarPrimitive.Root.Props) {
  return <ToolbarPrimitive.Root {...props} data-slot="toolbar" className={cn("tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2", className)} />;
}
export function ToolbarGroup({ className, ...props }: ToolbarPrimitive.Group.Props) {
  return <ToolbarPrimitive.Group {...props} data-slot="toolbar-group" className={cn("tw:flex tw:flex-wrap tw:items-center tw:gap-1", className)} />;
}
export function ToolbarButton({ className, ...props }: ToolbarPrimitive.Button.Props) {
  return <ToolbarPrimitive.Button {...props} render={<Button variant="ghost" size="sm" />}
    className={cn("tw:aria-pressed:border-primary tw:aria-pressed:bg-accent tw:aria-pressed:text-accent-foreground", className)} />;
}
