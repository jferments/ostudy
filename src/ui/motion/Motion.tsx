import { type ReactNode, useEffect, useState } from "react";

import { useMotionProfile } from "../hooks/useAnimationsEnabled";
import styles from "./Motion.module.css";

export function Presence({
  present,
  children,
  className = "",
}: {
  present: boolean;
  children: ReactNode;
  className?: string;
}) {
  const motion = useMotionProfile();
  const [rendered, setRendered] = useState(present);

  useEffect(() => {
    if (present) {
      setRendered(true);
      return;
    }
    if (motion === "none") {
      setRendered(false);
      return;
    }
    const timer = window.setTimeout(() => setRendered(false), motion === "reduced" ? 90 : 240);
    return () => window.clearTimeout(timer);
  }, [motion, present]);

  if (!rendered) return null;
  return (
    <div className={`${styles.presence} ${className}`.trim()} data-state={present ? "entered" : "exiting"}>
      {children}
    </div>
  );
}

export function RouteTransition({
  routeKey,
  direction = "forward",
  relation = "hierarchy",
  children,
  className = "",
}: {
  routeKey: string | number;
  direction?: "forward" | "back" | "neutral";
  relation?: "hierarchy" | "sibling";
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      key={routeKey}
      className={`${styles.route} ${className}`.trim()}
      data-direction={direction}
      data-relation={relation}
      data-motion-kind="spatial"
    >
      {children}
    </div>
  );
}
