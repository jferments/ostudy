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

export interface OverlayContextValue {
  topId: string | null;
  isTop(id: string): boolean;
  register(entry: OverlayEntry): () => void;
  confirm(options: ConfirmOptions): Promise<boolean>;
}

const fallback: OverlayContextValue = {
  topId: null,
  isTop: () => true,
  register: () => () => undefined,
  confirm: async () => false,
};

export const OverlayContext = createContext<OverlayContextValue>(fallback);
export const OverlayParentContext = createContext<string | null>(null);

export function useOverlayContext(): OverlayContextValue {
  return useContext(OverlayContext);
}

export function useOverlayParentId(): string | null {
  return useContext(OverlayParentContext);
}

export function useConfirm(): OverlayContextValue["confirm"] {
  return useOverlayContext().confirm;
}
