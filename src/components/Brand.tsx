import type { HTMLAttributes } from "react";

import mascotUrl from "../../assets/images/racoon mascot (logo).png";
import { cn } from "../ui/internal/cn";

export function BrandMark({ className = "", ...props }: HTMLAttributes<HTMLSpanElement>) {
  return (
    <span
      {...props}
      className={cn("tw:grid tw:size-12 tw:shrink-0 tw:place-items-center", className)}
      data-artwork-slot="brand.logo.compact"
      aria-hidden="true"
    >
      <img className="tw:pointer-events-none tw:block tw:size-full tw:object-contain tw:select-none" src={mascotUrl} alt="" draggable={false} />
    </span>
  );
}

export function BrandLockup({ compact = false }: { compact?: boolean }) {
  return (
    <div
      className={cn("tw:flex tw:min-w-0 tw:items-center tw:gap-3 tw:text-foreground", compact && "tw:[&>[data-artwork-slot]]:size-8.5")}
      data-artwork-slot="brand.logo.sidebar"
      aria-label="ostudy"
    >
      <BrandMark />
      <span className="tw:grid tw:min-w-0">
        <strong className={cn("tw:leading-none tw:tracking-tight", compact ? "tw:text-base" : "tw:text-2xl")}>ostudy</strong>
      </span>
    </div>
  );
}
