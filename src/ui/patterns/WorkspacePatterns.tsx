import { createContext, type HTMLAttributes, type ReactNode, useContext } from "react";

import { Icon, type IconName } from "../icons/Icon";
import { cn } from "../internal/cn";
import { Sheet } from "../overlays/Dialog";
import { Button } from "../primitives/Button";
import { ChipButton } from "../primitives/Chip";

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
            ? "tw:px-page tw:pt-7 tw:pb-[calc(4rem+var(--safe-bottom))] tw:compact:px-4 tw:compact:pt-5"
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

/**
 * The page title on the type scale, with an optional eyebrow, description and actions. Actions sit
 * beside the title and wrap below it when the row is too narrow; a ResponsiveActionGroup passed as
 * the actions collapses secondary ones into an overflow menu on phones.
 */
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
  /** A smaller title for workspaces whose own controls follow straight away. */
  compact?: boolean;
  children?: ReactNode;
}) {
  return (
    <header
      data-slot="page-header"
      className={cn(
        "tw:mb-6 tw:flex tw:min-w-0 tw:flex-wrap tw:items-end tw:justify-between tw:gap-x-6 tw:gap-y-4 tw:compact:mb-5",
        compact && "tw:mb-4 tw:items-center tw:compact:mb-4",
      )}
    >
      <div className="tw:min-w-0 tw:flex-[1_1_17.5rem]">
        {eyebrow ? (
          <span className="tw:mb-1 tw:block tw:text-caption tw:font-medium tw:tracking-wide tw:text-muted-foreground">{eyebrow}</span>
        ) : null}
        <div className="tw:flex tw:min-w-0 tw:items-center tw:gap-2.5">
          {icon ? <Icon name={icon} size={20} className="tw:shrink-0 tw:text-muted-foreground" /> : null}
          <h1
            className={cn(
              "tw:m-0 tw:min-w-0 tw:break-words tw:font-semibold tw:tracking-tight tw:text-foreground",
              compact ? "tw:text-section" : "tw:text-title",
            )}
          >
            {title}
          </h1>
        </div>
        {description ? (
          <p className="tw:mt-1 tw:mb-0 tw:max-w-prose tw:text-body tw:text-muted-foreground">{description}</p>
        ) : null}
        {children}
      </div>
      {actions ? (
        <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2.5">
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
        sticky && "tw:sticky tw:top-2 tw:z-(--ostudy-layer-sticky) tw:bg-muted",
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

export function AdaptiveSheet({ open, title, description, onClose, actions, children }: { open: boolean; title: string; description?: string; onClose: () => void; actions?: ReactNode; children: ReactNode }) {
  return open ? <Sheet title={title} description={description} width="medium" onClose={onClose} actions={actions}>{children}</Sheet> : null;
}
