import type { HTMLAttributes } from "react";

import { artworkManifest } from "../ui/artwork/artworkManifest";
import { cn } from "../ui/internal/cn";

const raccoonHead = artworkManifest["brand.logo.compact"].image;

/** The raccoon's head on a soft accent tile. It is the brand, not decorative artwork, so it
 * shows whatever the artwork preference says. The tile rounds in proportion to its size. */
export function BrandMark({ className = "", ...props }: HTMLAttributes<HTMLSpanElement>) {
  return (
    <span
      {...props}
      className={cn("tw:block tw:size-10 tw:shrink-0 tw:overflow-hidden tw:rounded-[30%] tw:bg-accent", className)}
      data-artwork-slot="brand.logo.compact"
      aria-hidden="true"
    >
      <img
        className="tw:pointer-events-none tw:block tw:size-full tw:select-none"
        src={raccoonHead?.src}
        srcSet={raccoonHead?.srcSet}
        alt=""
        draggable={false}
      />
    </span>
  );
}

/** The mark beside the "ostudy" wordmark, whose "o" takes the accent colour. */
export function BrandLockup({ compact = false }: { compact?: boolean }) {
  return (
    <div
      className="tw:flex tw:min-w-0 tw:items-center tw:gap-2.5 tw:text-foreground"
      data-artwork-slot="brand.logo.sidebar"
      aria-label="ostudy"
    >
      <BrandMark className={compact ? "tw:size-9" : undefined} />
      <span
        className={cn(
          "tw:min-w-0 tw:font-heading tw:font-bold tw:leading-none tw:tracking-tighter",
          compact ? "tw:text-[1.25rem]" : "tw:text-[1.3125rem]",
        )}
      >
        <span className="tw:text-primary">o</span>study
      </span>
    </div>
  );
}
