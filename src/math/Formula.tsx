import { memo, useEffect, useRef } from "react";
import { renderFormula } from "./render";

/** A span remains valid inside headings, links and paragraphs, even in display
 * mode. Only the existing bounded KaTeX port owns descendants; React owns the host.
 */
export const Formula = memo(function Formula({ source, block }: { source: string; block: boolean }) {
  const ref = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    if (ref.current) renderFormula(ref.current, source, block);
  }, [source, block]);
  return <span ref={ref} data-study-math={block ? "block" : "inline"}
    className={block
      ? "tw:block tw:max-w-full tw:overflow-x-auto tw:whitespace-pre-wrap tw:wrap-anywhere"
      : "tw:inline-block tw:max-w-full tw:overflow-x-auto tw:align-middle tw:whitespace-pre-wrap tw:wrap-anywhere"} />;
});
