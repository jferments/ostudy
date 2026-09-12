// shadcn/ui Base UI tabs composition, customized with oStudy semantic tokens.
import { Tabs as Primitive } from "@base-ui/react/tabs";
import { cn } from "../../internal/cn";
type Styled<P> = Omit<P, "className"> & { className?: string };

export const TabsRoot = Primitive.Root;
export function TabsList({ className, ...props }: Styled<Primitive.List.Props>) {
  return <Primitive.List {...props} data-slot="tabs-list" className={cn("tw:inline-flex tw:w-fit tw:min-w-max tw:items-center tw:gap-1 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-muted tw:p-1", className)} />;
}
export function TabsTrigger({ className, ...props }: Styled<Primitive.Tab.Props>) {
  return <Primitive.Tab {...props} data-slot="tabs-trigger" className={cn("tw:inline-flex tw:min-h-10 tw:items-center tw:justify-center tw:gap-2 tw:whitespace-nowrap tw:rounded-lg tw:border tw:border-solid tw:border-transparent tw:bg-transparent tw:px-4 tw:py-2 tw:text-label tw:font-medium tw:text-muted-foreground tw:transition-colors tw:hover:text-foreground tw:data-active:border-border tw:data-active:bg-background tw:data-active:text-foreground tw:data-active:shadow-panel tw:focus-visible:outline-2 tw:focus-visible:outline-offset-2 tw:focus-visible:outline-ring tw:disabled:opacity-50 tw:coarse:min-h-touch tw:reduce-motion:transition-none", className)} />;
}
export function TabsContent({ className, ...props }: Styled<Primitive.Panel.Props>) {
  return <Primitive.Panel {...props} data-slot="tabs-content" className={cn("tw:min-w-0 tw:outline-none tw:focus-visible:outline-2 tw:focus-visible:outline-ring", className)} />;
}
