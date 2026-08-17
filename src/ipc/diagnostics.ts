import { invoke } from "@tauri-apps/api/core";

const MAX_MESSAGE_CHARS = 4_000;
const MAX_STACK_CHARS = 12_000;
const MAX_FIELD_CHARS = 2_000;
const MAX_BATCH_EVENTS = 32;
const FLUSH_DELAY_MS = 250;
const EVENT_PATTERN = /^[a-z][a-z0-9_]{0,95}$/u;
const TARGET_PATTERN = /^[a-z][a-z0-9_.-]{0,127}$/u;
const SENSITIVE_FIELD_PATTERN =
  /(authorization|api.?key|credential|password|prompt|completion|request.?body|response.?body|secret|token|qr.?contents|document.?text|card.?content)/iu;

export type DiagnosticLevel = "error" | "warn" | "info" | "debug" | "trace";

export interface DiagnosticConfiguration {
  filter: string;
  logPath: string | null;
  droppedEvents: number;
  queueCapacity: number;
  maximumFileBytes: number;
}

export interface FrontendDiagnosticEvent {
  level: DiagnosticLevel;
  target: string;
  event: string;
  message: string;
  stack?: string;
  fields?: Record<string, unknown>;
}

const pendingEvents: FrontendDiagnosticEvent[] = [];
let flushTimer: ReturnType<typeof setTimeout> | null = null;
let flushInProgress = false;
let globalHandlersInstalled = false;

function boundedText(value: string, maximum: number): string {
  const characters = Array.from(value);
  if (characters.length <= maximum) return value;
  const omitted = characters.length - maximum;
  return `${characters.slice(0, maximum).join("")}… [truncated ${omitted} characters]`;
}

function diagnosticError(value: unknown): { message: string; stack?: string } {
  if (value instanceof Error) {
    return { message: value.message || value.name || "WebView error", stack: value.stack };
  }
  if (typeof value === "string" && value.trim() !== "") return { message: value };
  return { message: "Unhandled WebView error" };
}

function normalizedEvent(event: string): string {
  const normalized = event.trim().toLowerCase().replaceAll("-", "_");
  return EVENT_PATTERN.test(normalized) ? normalized : "webview_error";
}

function normalizedTarget(target: string): string {
  const normalized = target.trim().toLowerCase().replaceAll("::", ".");
  return TARGET_PATTERN.test(normalized) ? normalized : "frontend.webview";
}

function sanitizeFields(
  fields: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined {
  if (!fields) return undefined;
  const entries = Object.entries(fields)
    .slice(0, 64)
    .map(([key, value]) => {
      const boundedKey = boundedText(key, 96);
      if (SENSITIVE_FIELD_PATTERN.test(boundedKey)) return [boundedKey, "[redacted]"] as const;
      if (typeof value === "string") {
        return [boundedKey, boundedText(value, MAX_FIELD_CHARS)] as const;
      }
      if (typeof value === "number" || typeof value === "boolean" || value === null) {
        return [boundedKey, value] as const;
      }
      return [boundedKey, "[structured value omitted by frontend diagnostics]"] as const;
    });
  return Object.fromEntries(entries);
}

function normalizedDiagnostic(event: FrontendDiagnosticEvent): FrontendDiagnosticEvent {
  const fields = sanitizeFields(event.fields);
  return {
    level: event.level,
    target: normalizedTarget(event.target),
    event: normalizedEvent(event.event),
    message: boundedText(event.message, MAX_MESSAGE_CHARS),
    ...(event.stack ? { stack: boundedText(event.stack, MAX_STACK_CHARS) } : {}),
    ...(fields ? { fields } : {}),
  };
}

export async function getDiagnosticConfiguration(): Promise<DiagnosticConfiguration> {
  return invoke<DiagnosticConfiguration>("get_diagnostic_configuration");
}

export async function setDiagnosticFilter(
  filter: string,
  expiresInSeconds?: number,
): Promise<DiagnosticConfiguration> {
  return invoke<DiagnosticConfiguration>("set_diagnostic_filter", {
    input: { filter, expiresInSeconds },
  });
}

export async function flushNativeDiagnostics(): Promise<boolean> {
  try {
    return await invoke<boolean>("flush_diagnostics");
  } catch {
    return false;
  }
}

export async function recordWebviewDiagnostic(
  event: string,
  value: unknown,
  supplementalStack?: string,
): Promise<void> {
  const error = diagnosticError(value);
  const stack = supplementalStack ?? error.stack;
  try {
    await invoke<void>("record_webview_diagnostic", {
      input: {
        event: normalizedEvent(event),
        message: boundedText(error.message, MAX_MESSAGE_CHARS),
        ...(stack ? { stack: boundedText(stack, MAX_STACK_CHARS) } : {}),
      },
    });
  } catch {
    // Diagnostics must never create an unhandled rejection or crash loop.
  }
}

export function queueFrontendDiagnostic(event: FrontendDiagnosticEvent): void {
  pendingEvents.push(normalizedDiagnostic(event));
  if (pendingEvents.length >= MAX_BATCH_EVENTS || event.level === "error") {
    void flushFrontendDiagnostics();
    return;
  }
  if (flushTimer === null) {
    flushTimer = setTimeout(() => {
      flushTimer = null;
      void flushFrontendDiagnostics();
    }, FLUSH_DELAY_MS);
  }
}

export async function flushFrontendDiagnostics(): Promise<void> {
  if (flushInProgress || pendingEvents.length === 0) return;
  flushInProgress = true;
  if (flushTimer !== null) {
    clearTimeout(flushTimer);
    flushTimer = null;
  }
  try {
    while (pendingEvents.length > 0) {
      const events = pendingEvents.splice(0, MAX_BATCH_EVENTS);
      try {
        await invoke<void>("record_webview_diagnostic_batch", { input: { events } });
      } catch {
        // A broken diagnostics bridge must not retry recursively or retain an
        // unbounded queue. The native logger separately reports writer drops.
      }
    }
  } finally {
    flushInProgress = false;
  }
}

export function installGlobalWebviewDiagnostics(): void {
  if (globalHandlersInstalled || typeof window === "undefined") return;
  globalHandlersInstalled = true;

  window.addEventListener("error", (event) => {
    void recordWebviewDiagnostic(
      "webview_uncaught_error",
      event.error ?? event.message,
      event.error instanceof Error ? event.error.stack : undefined,
    );
  });
  window.addEventListener("unhandledrejection", (event) => {
    void recordWebviewDiagnostic("webview_unhandled_rejection", event.reason);
  });
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") {
      void flushFrontendDiagnostics();
      void flushNativeDiagnostics();
    }
  });
}
