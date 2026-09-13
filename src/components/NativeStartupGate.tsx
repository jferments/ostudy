import { type ReactNode, useEffect, useState } from "react";

import { BrandMark } from "./Brand";
import { getNativePlatformSnapshot } from "../ipc/platform";
import { Button } from "../ui/primitives/Button";
import { InlineNotice } from "../ui/primitives/InlineNotice";

const STARTUP_POLL_INTERVAL_MS = 25;
const STARTUP_TIMEOUT_MS = 30_000;

type StartupGateState = "waiting" | "ready" | "error";

export async function waitForNativeStartup(
  timeoutMs = STARTUP_TIMEOUT_MS,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  let lastError: unknown = null;
  while (Date.now() < deadline) {
    try {
      // PlatformRuntime is managed immediately after Database during durable
      // native startup. If this command succeeds, database-backed commands can
      // no longer race Tauri's state publication on a cold Android launch.
      await getNativePlatformSnapshot();
      return;
    } catch (error: unknown) {
      lastError = error;
    }
    await new Promise<void>((resolve) => window.setTimeout(resolve, STARTUP_POLL_INTERVAL_MS));
  }
  const detail = lastError instanceof Error ? lastError.message : String(lastError ?? "timeout");
  throw new Error(`Native startup did not become ready within ${timeoutMs} ms (${detail}).`);
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
