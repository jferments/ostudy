import type { HTMLAttributes, ReactNode } from "react";

import { Badge as ShadcnBadge } from "../vendor/shadcn/badge";

export type BadgeTone = "neutral" | "accent" | "success" | "warning" | "danger" | "info";
const variants = { neutral: "secondary", accent: "default", success: "success", warning: "warning", danger: "destructive", info: "info" } as const;

export function Badge({ tone = "neutral", children, ...props }:
  HTMLAttributes<HTMLSpanElement> & { tone?: BadgeTone; children: ReactNode }) {
  return <ShadcnBadge {...props} variant={variants[tone]}>{children}</ShadcnBadge>;
}
