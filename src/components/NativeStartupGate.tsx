import { type ReactNode, useEffect, useState } from "react";

import { getSettings } from "../ipc/settings";
import { LibraryRestoreObserver } from "./LibraryRestoreObserver";
import { BrandMark } from "./Brand";

const STARTUP_POLL_INTERVAL_MS = 40;
const STARTUP_ATTEMPT_TIMEOUT_MS = 750;
const STARTUP_SLOW_NOTICE_MS = 30_000;

type StartupGateState = "waiting" | "slow" | "ready" | "failed";

export async function waitForNativeStartup(
  timeoutMs: number | null = null,
  attemptTimeoutMs = STARTUP_ATTEMPT_TIMEOUT_MS,
): Promise<void> {
  const deadline = timeoutMs === null ? null : Date.now() + timeoutMs;
  let lastError: unknown = null;

  while (deadline === null || Date.now() < deadline) {
    const remaining = deadline === null ? attemptTimeoutMs : Math.max(1, deadline - Date.now());
    try {
      const ready = await withTimeout(
        getSettings().then(() => true),
        Math.min(attemptTimeoutMs, remaining),
      );
      if (ready) return;
    } catch (error: unknown) {
      if (typeof error === "object" && error !== null && "code" in error && error.code === "library_recovery_required") throw error;
      lastError = error;
    }

    if (deadline === null || Date.now() < deadline) {
      const delay = deadline === null
        ? STARTUP_POLL_INTERVAL_MS
        : Math.min(STARTUP_POLL_INTERVAL_MS, Math.max(1, deadline - Date.now()));
      await new Promise<void>((resolve) => window.setTimeout(resolve, delay));
    }
  }

  const detail = lastError instanceof Error ? lastError.message : String(lastError ?? "timeout");
  throw new Error(`Native startup did not become ready within ${timeoutMs} ms (${detail}).`);
}

async function withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
  let timer: number | undefined;
  try {
    return await Promise.race([
      promise,
      new Promise<never>((_, reject) => {
        timer = window.setTimeout(
          () => reject(new Error(`Native startup readiness probe timed out after ${timeoutMs} ms.`)),
          timeoutMs,
        );
      }),
    ]);
  } finally {
    if (timer !== undefined) window.clearTimeout(timer);
  }
}

export function NativeStartupGate({ children }: { children: ReactNode }) {
  const [state, setState] = useState<StartupGateState>("waiting");

  useEffect(() => {
    let disposed = false;
    const slowNotice = window.setTimeout(() => {
      if (!disposed) setState((current) => current === "waiting" ? "slow" : current);
    }, STARTUP_SLOW_NOTICE_MS);
    void waitForNativeStartup()
      .then(() => {
        if (!disposed) setState("ready");
      })
      .catch((error: unknown) => {
        if (!disposed) {
          console.error("Native startup readiness failed:", error);
          setState("failed");
        }
      });
    return () => {
      disposed = true;
      window.clearTimeout(slowNotice);
    };
  }, []);

  if (state === "ready") return <><LibraryRestoreObserver />{children}</>;

  return (
    <div className="tw:grid tw:min-h-dvh tw:place-content-center tw:gap-4 tw:bg-background tw:p-8 tw:text-center tw:text-foreground">
      <div className="tw:mx-auto"><BrandMark /></div>
      <h1 className="tw:m-0">{state === "failed" ? "Library recovery needs attention" : state === "slow" ? "Still opening ostudy" : "Opening ostudy"}</h1>
      <p className="tw:m-0 tw:max-w-xl tw:text-muted-foreground">
        {state === "failed"
          ? "The library could not be opened safely. Preserve its backup and recovery files, close oStudy, and reopen it to recover the recorded transaction. Do not reset or delete the library."
          : state === "slow"
          ? "Preparing this library is taking longer than usual. ostudy will open automatically when native startup finishes."
          : "Preparing your library…"}
      </p>
    </div>
  );
}
