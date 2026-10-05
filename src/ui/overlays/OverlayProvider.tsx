import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { registerPlatformBackHandler } from "../../platform/backNavigation";
import { Dialog } from "./Dialog";
import {
  type ConfirmOptions,
  type OverlayEntry,
  OverlayActionsContext,
  OverlayContext,
} from "./overlayContext";
import { Button } from "../primitives/Button";

interface PendingConfirmation extends ConfirmOptions { id: number; resolve: (accepted: boolean) => void; }
export function OverlayProvider({ children }: { children: ReactNode }) {
  const entriesRef = useRef<OverlayEntry[]>([]);
  const [topId, setTopId] = useState<string | null>(null);
  const [modalDepth, setModalDepth] = useState(0);
  const [confirmations, setConfirmations] = useState<PendingConfirmation[]>([]);
  const confirmationSequence = useRef(1);
  const publishStack = useCallback(() => {
    const entries = entriesRef.current;
    setTopId(resolveTopOverlay(entries)?.id ?? null);
    setModalDepth(entries.filter((entry) => entry.modal).length);
  }, []);
  const isTop = useCallback((id: string) => resolveTopOverlay(entriesRef.current)?.id === id, []);
  const register = useCallback((entry: OverlayEntry) => {
    entriesRef.current = entriesRef.current.filter((current) => current.id !== entry.id);
    entriesRef.current.push(entry);
    publishStack();
    return () => { entriesRef.current = entriesRef.current.filter((current) => current.id !== entry.id); publishStack(); };
  }, [publishStack]);
  const confirm = useCallback((options: ConfirmOptions) => {
    const id = confirmationSequence.current++;
    return new Promise<boolean>((resolve) => setConfirmations((current) => [...current, { ...options, id, resolve }]));
  }, []);
  const settleConfirmation = useCallback((id: number, accepted: boolean) => {
    setConfirmations((current) => {
      current.find((entry) => entry.id === id)?.resolve(accepted);
      return current.filter((entry) => entry.id !== id);
    });
  }, []);
  useEffect(() => {
    if (topId === null) return;
    return registerPlatformBackHandler(() => {
      const top = resolveTopOverlay(entriesRef.current);
      return top ? top.requestClose("platform_back") : false;
    });
  }, [topId]);
  useEffect(() => {
    if (modalDepth === 0) return;
    // App-root inertness also protects native-backed navigation. Browser focus
    // containment and scroll locking are owned by Base UI, not repeated here.
    const root = document.getElementById("root");
    const previousAriaHidden = root?.getAttribute("aria-hidden") ?? null;
    const previousInert = root?.inert ?? false;
    document.documentElement.dataset.overlayActive = "true";
    if (root) { root.inert = true; root.setAttribute("aria-hidden", "true"); }
    return () => {
      delete document.documentElement.dataset.overlayActive;
      if (root) {
        root.inert = previousInert;
        if (previousAriaHidden === null) root.removeAttribute("aria-hidden");
        else root.setAttribute("aria-hidden", previousAriaHidden);
      }
    };
  }, [modalDepth]);
  const actions = useMemo(() => ({ isTop, register, confirm }), [confirm, isTop, register]);
  const value = useMemo(() => ({ topId, ...actions }), [actions, topId]);
  const pending = confirmations[0];
  return <OverlayActionsContext.Provider value={actions}>
    <OverlayContext.Provider value={value}>
    {children}
    {pending ? <Dialog title={pending.title} description={pending.description} role="alertdialog" width="small" onClose={() => settleConfirmation(pending.id, false)}
      actions={<>
        <Button variant="secondary" data-autofocus="true" onClick={() => settleConfirmation(pending.id, false)}>{pending.cancelLabel ?? "Cancel"}</Button>
        <Button variant={pending.destructive ? "danger" : "primary"} onClick={() => settleConfirmation(pending.id, true)}>{pending.confirmLabel ?? "Continue"}</Button>
      </>} /> : null}
    </OverlayContext.Provider>
  </OverlayActionsContext.Provider>;
}
function resolveTopOverlay(entries: OverlayEntry[]): OverlayEntry | undefined {
  let top: OverlayEntry | undefined;
  let topDepth = -1;
  let topOrder = -1;
  entries.forEach((entry, order) => {
    const depth = overlayDepth(entry, entries);
    if (depth > topDepth || (depth === topDepth && order > topOrder)) { top = entry; topDepth = depth; topOrder = order; }
  });
  return top;
}
function overlayDepth(entry: OverlayEntry, entries: OverlayEntry[]): number {
  const byId = new Map(entries.map((candidate) => [candidate.id, candidate]));
  const visited = new Set<string>([entry.id]);
  let depth = 0;
  let parentId = entry.parentId;
  while (parentId && !visited.has(parentId)) { depth += 1; visited.add(parentId); parentId = byId.get(parentId)?.parentId ?? null; }
  return depth;
}
