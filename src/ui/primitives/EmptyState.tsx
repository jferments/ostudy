import type { ReactNode } from "react";
import { cn } from "../internal/cn";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "../vendor/shadcn/empty";
interface EmptyStateProps { title?: string; description?: ReactNode; icon?: ReactNode; actions?: ReactNode; compact?: boolean; large?: boolean; className?: string; }
export function EmptyState({ title, description, icon, actions, compact = false, large = false, className }: EmptyStateProps) {
  return <Empty className={cn(compact ? "tw:min-h-44 tw:py-8" : large ? "tw:min-h-80" : "tw:min-h-64", className)}>
    <EmptyHeader>{icon ? <EmptyMedia aria-hidden="true">{icon}</EmptyMedia> : null}{title ? <EmptyTitle>{title}</EmptyTitle> : null}{description ? <EmptyDescription>{description}</EmptyDescription> : null}</EmptyHeader>
    {actions ? <EmptyContent>{actions}</EmptyContent> : null}
  </Empty>;
}
export function LoadingState({ label = "Loading…" }: { label?: string }) {
  return <div className="tw:flex tw:min-h-32 tw:items-center tw:justify-center tw:gap-3 tw:text-small tw:text-muted-foreground" role="status" aria-live="polite">
    <span className="tw:size-5 tw:animate-spin tw:rounded-full tw:border-2 tw:border-solid tw:border-border tw:border-t-primary tw:reduce-motion:animate-none" aria-hidden="true" /><span>{label}</span>
  </div>;
}
