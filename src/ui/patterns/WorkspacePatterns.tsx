import { createContext, type HTMLAttributes, type ReactNode, useContext } from "react";

import { Icon, type IconName } from "../icons/Icon";
import { cn } from "../internal/cn";
import { Sheet } from "../overlays/Dialog";
import { Button } from "../primitives/Button";
import { Chip, ChipButton } from "../primitives/Chip";

const widths = {
  reading: "tw:max-w-(--ostudy-layout-reading-width)",
  workspace: "tw:max-w-(--ostudy-layout-workspace-width)",
  wide: "tw:max-w-(--ostudy-layout-wide-width)",
  full: "tw:max-w-none",
};

const PageScaffoldContext = createContext(false);

export function PageScaffold({
  width = "wide",
  inset,
  className,
  children,
  ...props
}: HTMLAttributes<HTMLDivElement> & {
  width?: keyof typeof widths;
  inset?: boolean;
  children: ReactNode;
}) {
  const nested = useContext(PageScaffoldContext);
  const resolvedInset = inset ?? !nested;
  return (
    <PageScaffoldContext.Provider value>
      <div
        {...props}
        data-page-scaffold="true"
        className={cn(
          "tw:mx-auto tw:min-h-full tw:w-full tw:min-w-0 tw:[&>*]:min-w-0",
          resolvedInset
            ? "tw:px-page tw:pt-7 tw:pb-[calc(4rem+var(--safe-bottom))] tw:max-[760px]:px-4 tw:max-[760px]:pt-5"
            : "tw:p-0",
          widths[width],
          className,
        )}
      >
        {children}
      </div>
    </PageScaffoldContext.Provider>
  );
}

export function PageHeader({
  eyebrow,
  title,
  description,
  icon,
  actions,
  compact = false,
  children,
}: {
  eyebrow?: string;
  title: ReactNode;
  description?: ReactNode;
  icon?: IconName;
  actions?: ReactNode;
  compact?: boolean;
  children?: ReactNode;
}) {
  return (
    <header
      className={cn(
        "tw:mb-6 tw:flex tw:min-w-0 tw:items-start tw:justify-between tw:gap-5 tw:max-[760px]:flex-col tw:max-[760px]:gap-3",
        compact && "tw:mb-4 tw:items-center tw:max-[760px]:items-stretch",
      )}
    >
      <div className="tw:min-w-0">
        {eyebrow ? (
          <span className="tw:mb-1 tw:block tw:text-xs tw:font-medium tw:tracking-wide tw:text-muted-foreground">{eyebrow}</span>
        ) : null}
        <div className="tw:flex tw:min-w-0 tw:items-center tw:gap-2.5">
          {icon ? <Icon name={icon} size={20} className="tw:shrink-0 tw:text-muted-foreground" /> : null}
          <h1
            className={cn(
              "tw:m-0 tw:min-w-0 tw:font-semibold tw:leading-tight tw:tracking-tight tw:text-foreground",
              compact ? "tw:text-2xl" : "tw:text-3xl",
            )}
          >
            {title}
          </h1>
        </div>
        {description ? (
          <p className="tw:mt-1.5 tw:mb-0 tw:max-w-prose tw:text-sm tw:leading-relaxed tw:text-muted-foreground">{description}</p>
        ) : null}
        {children}
      </div>
      {actions ? (
        <div className="tw:flex tw:min-w-0 tw:shrink-0 tw:flex-wrap tw:items-center tw:justify-end tw:gap-2 tw:max-[760px]:w-full tw:max-[760px]:justify-start">
          {actions}
        </div>
      ) : null}
    </header>
  );
}

export function CommandBar({
  label,
  sticky = false,
  className,
  children,
}: {
  label: string;
  sticky?: boolean;
  className?: string;
  children: ReactNode;
}) {
  return (
    <section
      aria-label={label}
      className={cn(
        "tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:justify-between tw:gap-2 tw:rounded-lg tw:bg-muted/40 tw:p-2",
        sticky && "tw:sticky tw:top-2 tw:z-20 tw:backdrop-blur-sm",
        className,
      )}
    >
      {children}
    </section>
  );
}

export interface ActiveFilter {
  id: string;
  label: string;
}

export function FilterSummary({ filters, onRemove, onClear }: { filters: ReadonlyArray<ActiveFilter>; onRemove: (id: string) => void; onClear: () => void }) {
  if (filters.length === 0) return null;
  return (
    <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2" aria-label="Active filters">
      {filters.map((filter) => (
        <ChipButton key={filter.id} tone="accent" onClick={() => onRemove(filter.id)} aria-label={`Remove ${filter.label} filter`}>
          {filter.label}
          <span aria-hidden="true">×</span>
        </ChipButton>
      ))}
      <Button size="small" variant="text" onClick={onClear}>Clear all</Button>
    </div>
  );
}

export function AdaptiveSheet({ open, title, description, onClose, children }: { open: boolean; title: string; description?: string; onClose: () => void; children: ReactNode }) {
  return open ? <Sheet title={title} description={description} width="medium" onClose={onClose}>{children}</Sheet> : null;
}

export function StatPill({ icon, label, value }: { icon?: IconName; label: string; value: ReactNode }) {
  return (
    <Chip className="tw:justify-between tw:gap-2">
      {icon ? <Icon name={icon} size={16} /> : null}
      <span>{label}</span>
      <strong className="tw:tabular-nums tw:text-foreground">{value}</strong>
    </Chip>
  );
}
