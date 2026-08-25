import overlayStyles from "../overlays/OverlayPatterns.module.css";
import screenStyles from "../patterns/ScreenPatterns.module.css";
import controlStyles from "../primitives/ControlPatterns.module.css";

export type StyleClassMap = Readonly<Record<string, string>>;
export type OwnedClassValue = string | false | null | undefined;

/**
 * Resolves semantic class keys across explicit CSS Module owners.
 *
 * A selector may intentionally exist in both a shared pattern module and a
 * feature module. In that case both scoped class names are emitted so the
 * shared baseline and the feature-specific relationship apply to the same
 * element without a global selector.
 */
export function ownedClassNames(
  maps: ReadonlyArray<StyleClassMap>,
  ...values: ReadonlyArray<OwnedClassValue>
): string {
  const output: string[] = [];
  const seen = new Set<string>();

  for (const value of values) {
    if (!value) continue;
    for (const token of value.split(/\s+/u)) {
      if (!token) continue;
      const owned = maps.flatMap((map) => {
        const resolved = map[token];
        return resolved ? resolved.split(/\s+/u) : [];
      });
      const resolvedTokens = owned.length > 0 ? owned : [token];
      for (const resolved of resolvedTokens) {
        if (!resolved || seen.has(resolved)) continue;
        seen.add(resolved);
        output.push(resolved);
      }
    }
  }

  return output.join(" ");
}

export const sharedClassMaps = [screenStyles, controlStyles, overlayStyles] as const;
