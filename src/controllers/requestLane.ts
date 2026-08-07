import { useEffect, useRef } from "react";

/**
 * Identifies the latest read request in one independent async lane.
 * Native work is not cancelled; obsolete completions are simply prevented from
 * mutating current UI state. Mutations intentionally do not use this primitive.
 */
export class RequestLane {
  #sequence = 0;

  begin(): number {
    this.#sequence += 1;
    return this.#sequence;
  }

  isCurrent(token: number): boolean {
    return token === this.#sequence;
  }

  invalidate(): void {
    this.#sequence += 1;
  }
}

export function useRequestLane(): RequestLane {
  const laneRef = useRef<RequestLane | null>(null);
  if (laneRef.current === null) laneRef.current = new RequestLane();
  useEffect(() => () => laneRef.current?.invalidate(), []);
  return laneRef.current;
}

/** Runs a read and returns its value only when it is still current. */
export async function latestResult<T>(
  lane: RequestLane,
  operation: () => Promise<T>,
): Promise<T | undefined> {
  const token = lane.begin();
  const value = await operation();
  return lane.isCurrent(token) ? value : undefined;
}
