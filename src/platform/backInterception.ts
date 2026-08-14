export type ApplyBackInterception = (enabled: boolean) => Promise<void>;

/**
 * Serializes native Back-interception updates while coalescing rapid stack
 * changes to the latest desired state. Mobile navigation can briefly transition
 * through overlay -> root -> section handler states; native IPC completion order
 * must never be allowed to leave Android on an older state.
 */
export class BackInterceptionSynchronizer {
  private desired = false;
  private applied: boolean | null = null;
  private running: Promise<void> | null = null;

  request(enabled: boolean, apply: ApplyBackInterception): Promise<void> {
    this.desired = enabled;
    if (this.running) return this.running;

    const run = this.flush(apply).finally(() => {
      if (this.running === run) this.running = null;
    });
    this.running = run;
    return run;
  }

  private async flush(apply: ApplyBackInterception): Promise<void> {
    while (this.applied !== this.desired) {
      const target = this.desired;
      await apply(target);
      this.applied = target;
    }
  }
}
