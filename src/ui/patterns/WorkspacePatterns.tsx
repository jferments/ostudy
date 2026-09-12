import type { HTMLAttributes, ReactNode } from "react";
import { Icon, type IconName } from "../icons/Icon";
import { Sheet } from "../overlays/Dialog";
import { Chip, ChipButton } from "../primitives/Chip";
import { Button } from "../primitives/Button";
import { Surface } from "../primitives/Surface";
import { cn } from "../internal/cn";
const widths = { reading: "tw:max-w-(--ostudy-layout-reading-width)", workspace: "tw:max-w-(--ostudy-layout-workspace-width)", wide: "tw:max-w-(--ostudy-layout-wide-width)", full: "tw:max-w-none" };
export function PageScaffold({ width = "wide", className, children, ...props }: HTMLAttributes<HTMLDivElement> & { width?: keyof typeof widths; children: ReactNode }) {
  return <div {...props} className={cn("tw:mx-auto tw:min-h-full tw:w-full tw:min-w-0 tw:px-page tw:pt-8 tw:pb-[calc(4rem+var(--safe-bottom))] tw:max-[760px]:px-4 tw:max-[760px]:pt-5", widths[width], className)}>{children}</div>;
}
export function PageHeader({ eyebrow, title, description, icon, actions, compact = false, children }: { eyebrow?: string; title: ReactNode; description?: ReactNode; icon?: IconName; actions?: ReactNode; compact?: boolean; children?: ReactNode }) {
  return <header className={cn("tw:mb-7 tw:flex tw:min-w-0 tw:items-start tw:justify-between tw:gap-5 tw:max-[760px]:flex-col", compact && "tw:mb-5 tw:items-center tw:max-[760px]:items-stretch")}>
    <div className="tw:flex tw:min-w-0 tw:items-start tw:gap-3">
      {icon ? <span className="tw:mt-1 tw:grid tw:size-9 tw:shrink-0 tw:place-items-center tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:text-accent-foreground"><Icon name={icon} size={20} /></span> : null}
      <div className="tw:min-w-0">
        {eyebrow ? <span className="tw:mb-1 tw:block tw:text-xs tw:font-medium tw:tracking-wide tw:text-muted-foreground">{eyebrow}</span> : null}
        <h1 className={cn("tw:m-0 tw:font-semibold tw:leading-tight tw:tracking-tight tw:text-foreground", compact ? "tw:text-2xl" : "tw:text-3xl")}>{title}</h1>
        {description ? <p className="tw:mt-2 tw:mb-0 tw:max-w-prose tw:text-sm tw:leading-relaxed tw:text-muted-foreground">{description}</p> : null}{children}
      </div>
    </div>
    {actions ? <div className="tw:flex tw:min-w-0 tw:shrink-0 tw:flex-wrap tw:items-center tw:justify-end tw:gap-2 tw:max-[760px]:w-full tw:max-[760px]:justify-start">{actions}</div> : null}
  </header>;
}
export function CommandBar({ label, sticky = false, className, children }: { label: string; sticky?: boolean; className?: string; children: ReactNode }) {
  return <Surface as="section" material="solid" aria-label={label} className={cn("tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:justify-between tw:gap-3 tw:px-4 tw:py-3", sticky && "tw:sticky tw:top-2 tw:z-20", className)}>{children}</Surface>;
}
export interface ActiveFilter { id: string; label: string; }
export function FilterSummary({ filters, onRemove, onClear }: { filters: ReadonlyArray<ActiveFilter>; onRemove: (id: string) => void; onClear: () => void }) {
  if (filters.length === 0) return null;
  return <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2" aria-label="Active filters">
    {filters.map((filter) => <ChipButton key={filter.id} tone="accent" onClick={() => onRemove(filter.id)} aria-label={`Remove ${filter.label} filter`}>{filter.label}<span aria-hidden="true">×</span></ChipButton>)}
    <Button size="small" variant="text" onClick={onClear}>Clear all</Button>
  </div>;
}
export function AdaptiveSheet({ open, title, description, onClose, children }: { open: boolean; title: string; description?: string; onClose: () => void; children: ReactNode }) {
  return open ? <Sheet title={title} description={description} width="medium" onClose={onClose}>{children}</Sheet> : null;
}
export function StatPill({ icon, label, value }: { icon?: IconName; label: string; value: ReactNode }) {
  return <Chip className="tw:justify-between tw:gap-2">{icon ? <Icon name={icon} size={16} /> : null}<span>{label}</span><strong className="tw:tabular-nums tw:text-foreground">{value}</strong></Chip>;
}
