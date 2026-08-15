import type { HTMLAttributes } from "react";

import { useAppearance } from "../appearance/AppearanceProvider";
import { Icon } from "../icons/Icon";
import { artworkManifest, type ArtworkSlot } from "./artworkManifest";
import styles from "./Artwork.module.css";

export function useArtwork(slot: ArtworkSlot) {
  const appearance = useAppearance();
  return {
    definition: artworkManifest[slot],
    enabled: appearance.artworkEnabled || artworkManifest[slot].category === "brand",
  };
}

export function Artwork({
  slot,
  className = "",
  ...props
}: HTMLAttributes<HTMLDivElement> & { slot: ArtworkSlot }) {
  const { definition, enabled } = useArtwork(slot);
  if (!enabled) {
    return (
      <div
        {...props}
        className={`${styles.fallback} ${className}`.trim()}
        data-artwork-slot={slot}
        aria-label={definition.decorative ? undefined : definition.fallbackLabel}
        aria-hidden={definition.decorative || undefined}
      >
        <Icon name={fallbackIcon(slot)} size={32} />
      </div>
    );
  }
  return (
    <div
      {...props}
      className={`${styles.artwork} ${styles[definition.category]} ${className}`.trim()}
      data-artwork-slot={slot}
      style={{ aspectRatio: definition.aspectRatio }}
      aria-label={definition.decorative ? undefined : definition.fallbackLabel}
      aria-hidden={definition.decorative || undefined}
    >
      <svg viewBox="0 0 320 240" role="presentation" focusable="false">
        <defs>
          <linearGradient id={`art-${slot.replaceAll(".", "-")}`} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="var(--ostudy-color-accent-base)" stopOpacity=".8" />
            <stop offset="1" stopColor="var(--ostudy-color-status-info)" stopOpacity=".5" />
          </linearGradient>
        </defs>
        <ellipse cx="160" cy="207" rx="105" ry="16" fill="var(--ostudy-color-ambient-vignette)" opacity=".2" />
        <rect x="64" y="48" width="192" height="134" rx="22" fill="var(--ostudy-color-surface-raised)" stroke="var(--ostudy-color-border-subtle)" />
        <rect x="82" y="66" width="156" height="98" rx="16" fill={`url(#art-${slot.replaceAll(".", "-")})`} opacity=".2" />
        <circle cx="152" cy="112" r="42" fill="var(--ostudy-color-text-muted)" opacity=".2" />
        <path d="M116 111c8-27 28-43 48-43s40 16 48 43c-9 30-28 47-48 47s-39-17-48-47Z" fill="var(--ostudy-color-text-secondary)" opacity=".72" />
        <path d="M125 91 105 69l7 38M203 91l20-22-7 38" fill="var(--ostudy-color-text-secondary)" opacity=".72" />
        <path d="M128 112c11-9 23-13 36-13s25 4 36 13c-11 9-23 13-36 13s-25-4-36-13Z" fill="var(--ostudy-color-canvas)" opacity=".88" />
        <circle cx="145" cy="111" r="5" fill="var(--ostudy-color-text-primary)" />
        <circle cx="183" cy="111" r="5" fill="var(--ostudy-color-text-primary)" />
        <path d="M158 126h12l-6 7Z" fill="var(--ostudy-color-text-primary)" />
        <path d="M96 150h48v34H96zM176 150h48v34h-48z" fill="var(--ostudy-color-surface-base)" stroke="var(--ostudy-color-border-strong)" />
        <path d="M105 160h30M105 168h22M185 160h30M185 168h22" stroke="var(--ostudy-color-accent-base)" strokeWidth="3" strokeLinecap="round" />
      </svg>
    </div>
  );
}

function fallbackIcon(slot: ArtworkSlot) {
  if (slot.includes("document")) return "document" as const;
  if (slot.includes("folder")) return "folder" as const;
  if (slot.includes("search")) return "search" as const;
  if (slot.includes("template")) return "template" as const;
  if (slot.includes("complete")) return "check" as const;
  return "cards" as const;
}
