import { formulaRenderingError } from "./source";

/** Application-owned lifecycle; consumers never hold a MathLive instance. */
export interface FormulaInput {
  focus(): void;
  toggleKeyboard(): void;
  hideKeyboard(): boolean;
  dispose(): void;
}

interface MountOptions {
  host: HTMLElement;
  keyboardHost: HTMLElement;
  source: string;
  signal: AbortSignal;
  onChange(source: string): void;
  onError(message: string | null): void;
}

let activeOwner: object | null = null;

/** Load the packaged editor only on explicit authoring intent, never in review. */
export async function mountFormulaInput(options: MountOptions): Promise<FormulaInput | null> {
  const { host, keyboardHost, signal, onChange, onError } = options;
  const initialError = options.source.trim() ? formulaRenderingError(options.source) : null;
  if (initialError) throw new Error(initialError);
  const { MathfieldElement } = await import("mathlive");
  // A dismissed dialog must not attach a keyboard or steal focus after loading.
  if (signal.aborted || !host.isConnected) return null;
  if (activeOwner) throw new Error("Another formula editor is already open.");

  // Fonts are imported by the foundation stylesheet and rewritten by Vite.
  // Disable every optional dynamic loader/evaluator and external-link action.
  MathfieldElement.fontsDirectory = null;
  MathfieldElement.soundsDirectory = null;
  MathfieldElement.computeEngine = null;
  MathfieldElement.openUrl = () => undefined;
  MathfieldElement.restoreFocusWhenDocumentFocused = false;

  const field = new MathfieldElement();
  field.setAttribute("aria-label", "Formula");
  field.setAttribute("spellcheck", "false");
  field.mathVirtualKeyboardPolicy = "manual";
  // The shared application dialog owns dismissal. Do not create a second
  // untracked context-menu/scrim stack, or offer evaluation/speech commands.
  field.menuItems = [];
  field.value = options.source;
  field.style.cssText = "display:block;min-height:4rem;max-width:100%;font-size:1.4rem";

  const keyboard = window.mathVirtualKeyboard;
  const previousContainer = keyboard.container;
  const owner = {};
  let disposed = false;
  let lastAccepted = options.source;
  const geometryChanged = () => {
    keyboardHost.style.height = keyboard.visible ? `${keyboard.boundingRect.height}px` : "0px";
  };
  const changed = () => {
    const source = field.value;
    const error = source.trim() ? formulaRenderingError(source) : null;
    if (error) {
      field.value = lastAccepted;
      onError(error);
      return;
    }
    lastAccepted = source;
    onError(null);
    onChange(source);
  };
  const pasted = (event: ClipboardEvent) => {
    // Do not allow HTML/MathML clipboard payloads to bypass source admission.
    event.preventDefault();
    event.stopPropagation();
    const source = event.clipboardData?.getData("text/plain") ?? "";
    const error = formulaRenderingError(source) || formulaRenderingError(lastAccepted + source);
    if (error) { onError(error); return; }
    field.insert(source, { format: "latex" });
    changed();
  };
  const dispose = () => {
    if (disposed) return;
    disposed = true;
    signal.removeEventListener("abort", dispose);
    field.removeEventListener("input", changed);
    field.removeEventListener("paste", pasted, true);
    keyboard.removeEventListener("geometrychange", geometryChanged);
    if (activeOwner === owner) {
      keyboard.hide();
      keyboard.container = previousContainer;
      activeOwner = null;
    }
    field.remove();
    keyboardHost.style.height = "0px";
  };
  activeOwner = owner;
  try {
    keyboard.hide();
    keyboard.container = keyboardHost;
    keyboard.addEventListener("geometrychange", geometryChanged);
    field.addEventListener("input", changed);
    field.addEventListener("paste", pasted, true);
    signal.addEventListener("abort", dispose, { once: true });
    host.append(field);
    // Assigning initial source must not normalize the session's unchanged save.
    // Only actual input events are forwarded to the caller.
    return {
      focus: () => { if (!disposed) field.focus(); },
      toggleKeyboard: () => {
        if (disposed) return;
        field.focus();
        if (keyboard.visible) keyboard.hide(); else keyboard.show();
        geometryChanged();
      },
      hideKeyboard: () => {
        if (disposed || !keyboard.visible) return false;
        keyboard.hide();
        geometryChanged();
        return true;
      },
      dispose,
    };
  } catch (error) {
    dispose();
    throw error;
  }
}
