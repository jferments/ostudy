import { createContext, useContext } from "react";

export type OverlayCloseReason =
  | "escape"
  | "backdrop"
  | "close_button"
  | "cancel_action"
  | "confirm_action"
  | "platform_back";

export interface OverlayEntry {
  id: string;
  parentId: string | null;
  modal: boolean;
  requestClose: (reason: OverlayCloseReason) => boolean;
}

export interface ConfirmOptions {
  title: string;
  description: string;
  confirmLabel?: string;
  cancelLabel?: string;
  destructive?: boolean;
}

export interface OverlayActionsContextValue {
  isTop(id: string): boolean;
  register(entry: OverlayEntry): () => void;
  confirm(options: ConfirmOptions): Promise<boolean>;
}

export interface OverlayContextValue extends OverlayActionsContextValue {
  topId: string | null;
}

const fallbackActions: OverlayActionsContextValue = {
  isTop: () => true,
  register: () => () => undefined,
  confirm: async () => false,
};
const fallback: OverlayContextValue = { topId: null, ...fallbackActions };

export const OverlayActionsContext = createContext<OverlayActionsContextValue>(fallbackActions);
export const OverlayContext = createContext<OverlayContextValue>(fallback);
export const OverlayParentContext = createContext<string | null>(null);

export function useOverlayActions(): OverlayActionsContextValue {
  return useContext(OverlayActionsContext);
}

export function useOverlayContext(): OverlayContextValue {
  return useContext(OverlayContext);
}

export function useOverlayParentId(): string | null {
  return useContext(OverlayParentContext);
}

export function useConfirm(): OverlayActionsContextValue["confirm"] {
  return useOverlayActions().confirm;
}
