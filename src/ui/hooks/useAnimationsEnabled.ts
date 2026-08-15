import { useAppearance } from "../appearance/AppearanceProvider";
import type { ResolvedMotion } from "../appearance/appearanceTypes";

/** Returns the resolved motion profile after user and operating-system preferences. */
export function useMotionProfile(): ResolvedMotion {
  return useAppearance().motion;
}

/**
 * Compatibility hook for scripted effects that require full spatial motion.
 * Reduced motion intentionally returns false so card flips, parallax, reorder
 * interpolation, and particle celebrations use their static alternatives.
 */
export function useAnimationsEnabled(): boolean {
  return useMotionProfile() === "full";
}
