// shadcn/ui Base UI tooltip composition; see README.md.
import { cloneElement, type ReactElement, useId, useState } from "react";
import { Tooltip } from "@base-ui/react/tooltip";
export function TooltipView({ content, children, delay }: { content: string; children: ReactElement<{ "aria-describedby"?: string }>; delay: number }) {
  const [open, setOpen] = useState(false);
  const id = `tip-${useId()}`;
  const description = [children.props["aria-describedby"], open ? id : undefined].filter(Boolean).join(" ") || undefined;
  return <Tooltip.Provider delay={delay}><Tooltip.Root open={open} onOpenChange={setOpen}>
    <Tooltip.Trigger render={cloneElement(children, { "aria-describedby": description })} aria-describedby={description} />
    <Tooltip.Portal><Tooltip.Positioner side="top" sideOffset={7} className="tw:isolate tw:z-(--ostudy-layer-popover)">
      <Tooltip.Popup role="tooltip" id={id} data-slot="tooltip-content" className="tw:max-w-xs tw:rounded-md tw:border tw:border-solid tw:border-border tw:bg-foreground tw:px-3 tw:py-2 tw:font-sans tw:text-caption tw:leading-relaxed tw:text-background tw:shadow-panel">{content}</Tooltip.Popup>
    </Tooltip.Positioner></Tooltip.Portal>
  </Tooltip.Root></Tooltip.Provider>;
}
