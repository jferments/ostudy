import { Notification, type NotificationAction, type NotificationMessage } from "../ui/primitives/Notification";
export type ToastAction = NotificationAction;
export type ToastMessage = NotificationMessage;
export type ShowToast = (kind: ToastMessage["kind"], text: string, action?: ToastAction) => void;
interface ToastProps { toast: ToastMessage | null; onDismiss: () => void; }
export function Toast({ toast, onDismiss }: ToastProps) {
  return toast ? <Notification key={toast.id} message={toast} onDismiss={onDismiss} /> : null;
}
