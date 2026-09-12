// Adapted from shadcn/ui Base UI Toast. oStudy currently displays one notification.
import { type ReactNode, useEffect, useRef } from "react";
import { Toast } from "@base-ui/react/toast";
import { Button } from "./button";
import { Icon } from "../../icons/Icon";
import { cn } from "../../internal/cn";
export interface NotificationViewProps {
  id: string; message: string; tone: "success" | "error" | "info";
  timeout: number; busy: boolean; action?: ReactNode; onDismiss: () => void;
}
export function NotificationView(props: NotificationViewProps) {
  return <Toast.Provider limit={1} timeout={props.timeout}><NotificationContent {...props} /></Toast.Provider>;
}
function NotificationContent({ id, message, tone, timeout, busy, action, onDismiss }: NotificationViewProps) {
  const { toasts, add, update } = Toast.useToastManager();
  const dismiss = useRef(onDismiss); dismiss.current = onDismiss;
  useEffect(() => {
    let active = true;
    add({ id, description: message, type: tone, priority: "low", timeout,
      onClose() { if (active) { active = false; dismiss.current(); } },
    });
    // The scoped Provider disposes its store; do not dismiss during StrictMode effect replay.
    return () => { active = false; };
  }, [id, message, tone, timeout, add]);
  useEffect(() => { update(id, { timeout: busy ? 0 : timeout }); }, [id, busy, timeout, update]);
  return <Toast.Portal><Toast.Viewport data-slot="toast-viewport" className="tw:fixed tw:right-4 tw:bottom-[calc(var(--safe-bottom)+var(--android-ime-bottom)+1rem)] tw:z-[700] tw:w-[calc(100vw-2rem)] tw:max-w-sm tw:outline-none tw:max-[760px]:bottom-[calc(var(--safe-bottom)+var(--android-ime-bottom)+5rem)]">
    {toasts.map((item) => <Toast.Root key={item.id} toast={item} role={tone === "error" ? "alert" : "status"} data-slot="toast" className={cn("tw:relative tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-popover tw:font-sans tw:text-popover-foreground tw:shadow-overlay tw:outline-none tw:focus-visible:outline-2 tw:focus-visible:outline-ring", tone === "error" && "tw:border-destructive/40")}>
      <Toast.Content className="tw:flex tw:items-start tw:gap-3 tw:p-4">
        <span className={cn("tw:mt-0.5 tw:shrink-0", tone === "error" ? "tw:text-destructive" : tone === "success" ? "tw:text-success" : "tw:text-primary")} aria-hidden="true"><Icon name={tone === "error" ? "warning" : tone === "success" ? "check" : "info"} size={20} /></span>
        <div className="tw:min-w-0 tw:flex-1 tw:space-y-3"><Toast.Description className="tw:text-sm tw:leading-relaxed tw:break-words" />{action}</div>
        <Toast.Close aria-label="Dismiss notification" render={<Button variant="ghost" size="icon" className="tw:-mr-2 tw:-mt-2" />}><Icon name="close" size={16} /></Toast.Close>
      </Toast.Content>
    </Toast.Root>)}
  </Toast.Viewport></Toast.Portal>;
}
