import { formulaRenderingError } from "./source";

/** Application-owned lifecycle; consumers never hold a MathLive instance. */
export interface FormulaInput {
  focus(): void;
  applyTemplate(template: string): boolean;
  showKeyboard(): boolean;
  toggleKeyboard(): boolean;
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
  if (signal.aborted || !host.isConnected || !keyboardHost.isConnected) return null;
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
  field.value = options.source;
  field.style.cssText = "display:block;box-sizing:border-box;width:100%;min-width:0;min-height:4rem;max-width:100%;font-size:1.4rem";

  const keyboard = window.mathVirtualKeyboard;
  const previousContainer = keyboard.container;
  const owner = {};
  let disposed = false;
  let resolveMount: ((ready: boolean) => void) | undefined;
  let rejectMount: ((error: unknown) => void) | undefined;
  const mounted = new Promise<boolean>((resolve, reject) => {
    resolveMount = resolve;
    rejectMount = reject;
  });
  const didMount = () => {
    if (disposed) return;
    try {
      // In MathLive 0.110 menuItems requires its internal mounted mathfield.
      // DOM attachment alone is not readiness: subscribe before append, then
      // configure the menu before making the input available to the dialog.
      field.menuItems = [];
      resolveMount?.(true);
    } catch (error) {
      rejectMount?.(error);
    }
  };
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
    if (error) {
      onError(error);
      return;
    }
    field.insert(source, { format: "latex" });
    changed();
  };
  const applyTemplate = (template: string): boolean => {
    if (disposed) return false;
    field.focus();
    // MathLive owns selection-aware #@/#0/#? expansion. Using the public insert
    // API preserves its structured selection semantics instead of rewriting
    // LaTeX in oStudy or inventing a second formula model.
    const inserted = field.insert(template, {
      format: "latex",
      selectionMode: "placeholder",
    });
    if (!inserted) {
      onError("That mathematical structure could not be inserted here.");
      return false;
    }
    changed();
    return true;
  };
  const dispose = () => {
    if (disposed) return;
    disposed = true;
    signal.removeEventListener("abort", dispose);
    field.removeEventListener("mount", didMount);
    resolveMount?.(false);
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
    field.addEventListener("mount", didMount, { once: true });
    host.append(field);
    if (!(await mounted) || disposed || signal.aborted) {
      dispose();
      return null;
    }
    // Assigning initial source must not normalize the session's unchanged save.
    // Only actual input events are forwarded to the caller.
    return {
      focus: () => {
        if (!disposed) field.focus();
      },
      applyTemplate,
      showKeyboard: () => {
        if (disposed) return false;
        field.focus();
        if (!keyboard.visible) keyboard.show();
        geometryChanged();
        return keyboard.visible;
      },
      toggleKeyboard: () => {
        if (disposed) return false;
        field.focus();
        if (keyboard.visible) keyboard.hide();
        else keyboard.show();
        geometryChanged();
        return keyboard.visible;
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
