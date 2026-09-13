import type { HTMLAttributes } from "react";

import { Skeleton as ShadcnSkeleton } from "../vendor/shadcn/skeleton";

export function Skeleton({
  width = "100%", height = "1rem", radius = "var(--ostudy-radius-small)", style, ...props
}: HTMLAttributes<HTMLSpanElement> & { width?: string; height?: string; radius?: string }) {
  return <ShadcnSkeleton {...props} aria-hidden="true" style={{ ...style, width, height, borderRadius: radius }} />;
}
