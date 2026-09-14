import { useEffect, useRef, useState } from "react";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { renderFormula } from "./render";

export function FormulaPreview({ source, block }: { source: string; block: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (ref.current) setError(source.trim() ? renderFormula(ref.current, source, block) : null);
    if (!source.trim()) ref.current?.replaceChildren();
  }, [source, block]);
  return <div className="tw:grid tw:min-w-0 tw:gap-2">
    <div ref={ref} aria-label="Formula preview" className="tw:max-w-full tw:overflow-x-auto tw:whitespace-pre-wrap tw:wrap-anywhere" />
    {error ? <InlineNotice kind="warning">{error}</InlineNotice> : null}
  </div>;
}
