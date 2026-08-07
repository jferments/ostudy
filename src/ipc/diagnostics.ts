import { invoke } from "@tauri-apps/api/core";

const MAX_MESSAGE_CHARS = 4_000;
const MAX_STACK_CHARS = 12_000;
const EVENT_PATTERN = /^[a-z][a-z0-9_]{0,63}$/u;

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

export async function recordWebviewDiagnostic(
  event: string,
  value: unknown,
  supplementalStack?: string,
): Promise<void> {
  const normalizedEvent = EVENT_PATTERN.test(event) ? event : "webview_error";
  const error = diagnosticError(value);
  const stack = supplementalStack ?? error.stack;
  try {
    await invoke<void>("record_webview_diagnostic", {
      input: {
        event: normalizedEvent,
        message: boundedText(error.message, MAX_MESSAGE_CHARS),
        stack: stack ? boundedText(stack, MAX_STACK_CHARS) : undefined,
      },
    });
  } catch {
    // Diagnostics must never create an unhandled rejection or crash loop.
  }
}
