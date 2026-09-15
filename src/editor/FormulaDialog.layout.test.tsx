import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mountFormulaInput } from "../math/mathlive";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { FormulaDialog } from "./FormulaDialog";
import type { FormulaEditSession } from "./formula";

const platformState = vi.hoisted(() => ({
  operatingSystem: "linux",
  runtimeFamily: "desktop",
  coarsePointer: false,
  hoverAvailable: true,
}));

vi.mock("../math/mathlive", () => ({ mountFormulaInput: vi.fn() }));
vi.mock("../platform/PlatformProvider", () => ({
  usePlatformCapabilities: () => ({
    native: {
      operatingSystem: platformState.operatingSystem,
      runtimeFamily: platformState.runtimeFamily,
    },
    coarsePointer: platformState.coarsePointer,
    hoverAvailable: platformState.hoverAvailable,
  }),
}));

beforeEach(() => {
  platformState.operatingSystem = "linux";
  platformState.runtimeFamily = "desktop";
  platformState.coarsePointer = false;
  platformState.hoverAvailable = true;
});

afterEach(() => vi.resetAllMocks());

function session(): FormulaEditSession {
  return {
    initialValue: { latex: "x", placement: "inline" },
    save: vi.fn(() => ({ ok: true as const })),
    cancel: vi.fn(),
  };
}

type MountedFormulaInput = NonNullable<Awaited<ReturnType<typeof mountFormulaInput>>>;

function mountedInput(overrides: Partial<MountedFormulaInput> = {}): MountedFormulaInput {
  return {
    focus: vi.fn(),
    showKeyboard: vi.fn(() => true),
    toggleKeyboard: vi.fn(() => true),
    hideKeyboard: vi.fn(() => false),
    applyTemplate: vi.fn(() => true),
    dispose: vi.fn(),
    ...overrides,
  };
}

it("keeps mount failures and responsive actions inside the padded dialog body", async () => {
  vi.mocked(mountFormulaInput).mockRejectedValue(new Error("Formula editor unavailable"));
  const editSession = session();
  const onClose = vi.fn();
  render(<OverlayProvider><FormulaDialog session={editSession} onClose={onClose} /></OverlayProvider>);
  const alert = await screen.findByRole("alert");
  const save = screen.getByRole("button", { name: "Save formula" }) as HTMLButtonElement;
  const actions = save.parentElement!;
  const body = actions.parentElement!;
  expect(body.classList.contains("tw:p-4")).toBe(true);
  expect(body.classList.contains("tw:box-border")).toBe(true);
  expect(body.classList.contains("tw:min-w-0")).toBe(true);
  expect(body.contains(alert)).toBe(true);
  expect(actions.classList.contains("tw:max-[760px]:flex-col-reverse")).toBe(true);
  expect(actions.classList.contains("tw:max-[760px]:[&>button]:w-full")).toBe(true);
  expect(save.disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  expect(editSession.cancel).toHaveBeenCalledOnce();
  expect(editSession.save).not.toHaveBeenCalled();
});

it("puts the full keyboard directly below the input, graphical tools after it, and placement last", async () => {
  const toggleKeyboard = vi.fn(() => true);
  vi.mocked(mountFormulaInput).mockResolvedValue(mountedInput({ toggleKeyboard }));
  render(<OverlayProvider><FormulaDialog session={session()} onClose={vi.fn()} /></OverlayProvider>);
  const keyboardButton = await screen.findByRole("button", { name: "Show math keyboard" });
  await waitFor(() => expect((keyboardButton as HTMLButtonElement).disabled).toBe(false));

  const placement = screen.getByLabelText("Formula placement") as HTMLSelectElement;
  expect(Array.from(placement.options, (option) => option.text)).toEqual([
    "Inline with text",
    "On its own line",
  ]);
  const inputShell = document.querySelector("[data-formula-input-shell]")!;
  const keyboardHost = document.querySelector("[data-formula-keyboard-host]")!;
  const toolbox = screen.getByRole("region", { name: "Visual formula controls" });
  expect(inputShell.contains(keyboardButton)).toBe(true);
  expect(inputShell.compareDocumentPosition(keyboardHost) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
  expect(keyboardHost.compareDocumentPosition(toolbox) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
  expect(toolbox.compareDocumentPosition(placement) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
  expect(screen.getByRole("button", { name: "Shortcuts & help" })).toBeTruthy();

  fireEvent.click(keyboardButton);
  expect(toggleKeyboard).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "Hide math keyboard" })).toBeTruthy();
});

it("opens the full MathLive keyboard by default on Android and hides desktop shortcut help", async () => {
  platformState.operatingSystem = "android";
  platformState.runtimeFamily = "mobile";
  platformState.coarsePointer = true;
  platformState.hoverAvailable = false;
  const showKeyboard = vi.fn(() => true);
  const toggleKeyboard = vi.fn(() => false);
  vi.mocked(mountFormulaInput).mockResolvedValue(mountedInput({ showKeyboard, toggleKeyboard }));

  render(<OverlayProvider><FormulaDialog session={session()} onClose={vi.fn()} /></OverlayProvider>);
  await waitFor(() => expect(showKeyboard).toHaveBeenCalledOnce());
  const keyboardButton = screen.getByRole("button", { name: "Hide math keyboard" });
  expect(keyboardButton.getAttribute("aria-pressed")).toBe("true");
  expect(screen.queryByRole("button", { name: "Shortcuts & help" })).toBeNull();

  fireEvent.click(keyboardButton);
  expect(toggleKeyboard).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "Show math keyboard" })).toBeTruthy();
});
