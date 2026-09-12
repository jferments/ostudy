import type { ReactNode } from "react";
import { Icon, type IconName } from "../icons/Icon";
import { Alert, AlertDescription, AlertTitle } from "../vendor/shadcn/alert";
interface InlineNoticeProps { kind?: "info" | "success" | "warning" | "error"; title?: string; children: ReactNode; className?: string; }
const icons: Record<NonNullable<InlineNoticeProps["kind"]>, IconName> = { info: "information", success: "check", warning: "warning", error: "warning" };
export function InlineNotice({ kind = "info", title, children, className }: InlineNoticeProps) {
  return <Alert variant={kind} role={kind === "error" ? "alert" : "status"} className={className}>
    <div className="tw:flex tw:items-start tw:gap-3">
      <span className="tw:mt-0.5 tw:shrink-0" aria-hidden="true"><Icon name={icons[kind]} size={20} /></span>
      <div className="tw:min-w-0 tw:flex-1">{title ? <AlertTitle>{title}</AlertTitle> : null}<AlertDescription>{children}</AlertDescription></div>
    </div>
  </Alert>;
}
export function InlineProgress({ children }: { children: ReactNode }) {
  return <Alert role="status" aria-live="polite" className="tw:flex tw:items-center tw:gap-3 tw:text-muted-foreground">
    <span className="tw:size-4 tw:shrink-0 tw:animate-spin tw:rounded-full tw:border-2 tw:border-solid tw:border-border tw:border-t-primary tw:reduce-motion:animate-none" aria-hidden="true" />
    <div className="tw:min-w-0 tw:flex-1">{children}</div>
  </Alert>;
}
