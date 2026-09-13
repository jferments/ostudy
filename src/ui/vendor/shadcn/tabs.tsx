// shadcn/ui Base UI Nova tabs composition, customized with oStudy semantic tokens.
import { Tabs as Primitive } from "@base-ui/react/tabs";

import { cn } from "../../internal/cn";
type Styled<P> = Omit<P, "className"> & { className?: string };

export const TabsRoot = Primitive.Root;
export function TabsList({ className, ...props }: Styled<Primitive.List.Props>) {
  return <Primitive.List {...props} data-slot="tabs-list" className={cn("tw:inline-flex tw:w-fit tw:min-w-max tw:items-center tw:gap-0.5 tw:rounded-lg tw:bg-muted tw:p-0.5", className)} />;
}
export function TabsTrigger({ className, ...props }: Styled<Primitive.Tab.Props>) {
  return <Primitive.Tab {...props} data-slot="tabs-trigger" className={cn("tw:inline-flex tw:h-7 tw:items-center tw:justify-center tw:gap-1.5 tw:whitespace-nowrap tw:rounded-md tw:border-0 tw:bg-transparent tw:px-2.5 tw:text-sm tw:font-medium tw:text-muted-foreground tw:outline-none tw:transition-colors tw:hover:text-foreground tw:data-active:bg-background tw:data-active:text-foreground tw:data-active:ring-1 tw:data-active:ring-border tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:disabled:opacity-50 tw:coarse:min-h-touch tw:coarse:px-3 tw:reduce-motion:transition-none", className)} />;
}
export function TabsContent({ className, ...props }: Styled<Primitive.Panel.Props>) {
  return <Primitive.Panel {...props} data-slot="tabs-content" className={cn("tw:min-w-0 tw:outline-none tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50", className)} />;
}
