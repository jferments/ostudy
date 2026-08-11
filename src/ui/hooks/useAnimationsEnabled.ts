import { useEffect, useState } from "react";

function readAnimationsEnabled(): boolean {
  return document.documentElement.dataset.animationsEnabled !== "false";
}

/**
 * Mirrors the root animation preference for effects implemented in JavaScript
 * rather than CSS. CSS transitions are disabled directly by the root data
 * attribute; canvas and other scripted effects use this hook.
 */
export function useAnimationsEnabled(): boolean {
  const [enabled, setEnabled] = useState(readAnimationsEnabled);

  useEffect(() => {
    const root = document.documentElement;
    const update = () => setEnabled(readAnimationsEnabled());
    update();
    const observer = new MutationObserver(update);
    observer.observe(root, {
      attributes: true,
      attributeFilter: ["data-animations-enabled"],
    });
    return () => observer.disconnect();
  }, []);

  return enabled;
}
