import type { ReactElement } from "react";
import { TooltipView } from "../vendor/shadcn/tooltip";
export const TOOLTIP_POINTER_DELAY_MS = 360;
export interface TooltipProps { content: string; children: ReactElement<{ "aria-describedby"?: string }>; }
export function Tooltip(props: TooltipProps) { return <TooltipView {...props} delay={TOOLTIP_POINTER_DELAY_MS} />; }
