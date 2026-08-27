import { Profiler, type ReactNode } from "react";

import {
  recordUiPerformanceCommit,
  UI_PERFORMANCE_CAPTURE_ENABLED,
  type UiPerformanceBoundaryId,
} from "./uiPerformanceInstrumentation";

export function PerformanceProfiler({
  id,
  children,
}: {
  id: UiPerformanceBoundaryId;
  children: ReactNode;
}) {
  if (!UI_PERFORMANCE_CAPTURE_ENABLED) return children;
  return (
    <Profiler id={id} onRender={recordUiPerformanceCommit}>
      {children}
    </Profiler>
  );
}
