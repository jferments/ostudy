import { useLayoutEffect, useRef, type ReactNode } from "react";

import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { Button } from "../../ui/primitives/Button";

/** Full-width authoring workspace, not a modal or an abbreviated sidebar. The
 * caller retains the hidden editor and owns returning to its exact draft. */
export function CardPreviewWorkspace({ title, onClose, children }: {
  title: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const headerRef = useRef<HTMLElement | null>(null);
  useLayoutEffect(() => {
    headerRef.current?.querySelector<HTMLButtonElement>("button")?.focus({ preventScroll: true });
    headerRef.current?.scrollIntoView?.({ block: "start" });
  }, []);

  return <PageScaffold width="wide" className="tw:max-w-6xl tw:pt-5" role="region" aria-label="Card preview">
    <header ref={headerRef} className="tw:sticky tw:top-0 tw:z-20 tw:mb-5 tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3 tw:border-b tw:border-border tw:bg-background tw:py-3">
      <div className="tw:min-w-0">
        <span className="tw:text-xs tw:font-medium tw:tracking-wide tw:text-muted-foreground">CARD PREVIEW</span>
        <h1 className="tw:m-0 tw:break-words tw:text-2xl tw:font-semibold">{title}</h1>
      </div>
      <Button variant="primary" onClick={onClose}>Close preview</Button>
    </header>
    <div className="tw:grid tw:min-w-0 tw:gap-5">{children}</div>
  </PageScaffold>;
}
