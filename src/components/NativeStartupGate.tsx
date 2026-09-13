import { type ReactNode, useEffect, useState } from "react";

import { getSettings } from "../ipc/settings";
import { Button } from "../ui/primitives/Button";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { BrandMark } from "./Brand";

const STARTUP_POLL_INTERVAL_MS = 40;
const STARTUP_ATTEMPT_TIMEOUT_MS = 750;
const STARTUP_TIMEOUT_MS = 30_000;

type StartupGateState = "waiting" | "ready" | "error";

export async function waitForNativeStartup(
  timeoutMs = STARTUP_TIMEOUT_MS,
  attemptTimeoutMs = STARTUP_ATTEMPT_TIMEOUT_MS,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  let lastError: unknown = null;

  while (Date.now() < deadline) {
    const remaining = Math.max(1, deadline - Date.now());
    try {
      const ready = await withTimeout(
        getSettings().then(() => true),
        Math.min(attemptTimeoutMs, remaining),
      );
      if (ready) return;
    } catch (error: unknown) {
      lastError = error;
    }

    if (Date.now() < deadline) {
      await new Promise<void>((resolve) =>
        window.setTimeout(resolve, Math.min(STARTUP_POLL_INTERVAL_MS, deadline - Date.now())),
      );
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
    void waitForNativeStartup()
      .then(() => {
        if (!disposed) setState("ready");
      })
      .catch((error: unknown) => {
        if (!disposed) {
          console.error("Native startup readiness failed:", error);
          setState("error");
        }
      });
    return () => {
      disposed = true;
    };
  }, []);

  if (state === "ready") return children;

  return (
    <div className="tw:grid tw:min-h-dvh tw:place-content-center tw:gap-4 tw:bg-background tw:p-8 tw:text-center tw:text-foreground">
      {state === "waiting" ? (
        <>
          <div className="tw:mx-auto"><BrandMark /></div>
          <h1 className="tw:m-0">Opening ostudy</h1>
          <p className="tw:m-0 tw:text-muted-foreground">Preparing your library…</p>
        </>
      ) : (
        <>
          <h1 className="tw:m-0">ostudy could not finish starting</h1>
          <InlineNotice kind="error">
            Native startup did not become ready. Restart the app; your library was not intentionally reset.
          </InlineNotice>
          <Button variant="primary" onClick={() => window.location.reload()}>
            Try again
          </Button>
        </>
      )}
    </div>
  );
}
