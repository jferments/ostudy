import { useEffect, useRef, useState } from "react";
import { FormulaPreview } from "../math/FormulaPreview";
import { mountFormulaInput, type FormulaInput } from "../math/mathlive";
import { formulaRenderingError, formulaSourceError } from "../math/source";
import { usePlatformCapabilities } from "../platform/PlatformProvider";
import { IconButton } from "../ui/icons/IconButton";
import { Dialog } from "../ui/overlays/Dialog";
import type { OverlayCloseReason } from "../ui/overlays/overlayContext";
import { Button } from "../ui/primitives/Button";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { NativeSelect } from "../ui/primitives/NativeSelect";
import type { FormulaEditSession, FormulaPlacement } from "./formula";
import { FormulaToolbox } from "./FormulaToolbox";
import contentStyles from "./RichContent.module.css";

/** Draft state is local. Only the captured session can change the card. */
export function FormulaDialog({ session, onClose }: {
  session: FormulaEditSession;
  onClose(): void;
}) {
  const platform = usePlatformCapabilities();
  const [source, setSource] = useState(session.initialValue.latex);
  const [placement, setPlacement] = useState(session.initialValue.placement);
  const [ready, setReady] = useState(false);
  const [keyboardVisible, setKeyboardVisible] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Dialog content is portaled after the parent effect can first run. Callback
  // refs make node availability an explicit input to the mounting lifecycle.
  const [host, setHost] = useState<HTMLDivElement | null>(null);
  const [keyboardHost, setKeyboardHost] = useState<HTMLDivElement | null>(null);
  const input = useRef<FormulaInput | null>(null);
  const initialFocus = useRef<HTMLButtonElement>(null);
  const finished = useRef(false);
  const lifecycle = useRef<AbortController | null>(null);
  const androidKeyboardOpened = useRef(false);
  const isAndroid = platform.native?.operatingSystem === "android";
  const mobileRuntime = platform.native?.runtimeFamily === "mobile";
  const showHardwareKeyboardHelp = platform.native
    ? !mobileRuntime
    : !(platform.coarsePointer && !platform.hoverAvailable);

  useEffect(() => {
    if (!host || !keyboardHost || finished.current) return;
    const controller = new AbortController();
    lifecycle.current = controller;
    setReady(false);
    setKeyboardVisible(false);
    setError(null);
    void mountFormulaInput({
      host,
      keyboardHost,
      source: session.initialValue.latex,
      signal: controller.signal,
      onChange: setSource,
      onError: setError,
    })
      .then((mounted) => {
        if (controller.signal.aborted || finished.current) {
          mounted?.dispose();
          return;
        }
        input.current = mounted;
        setReady(mounted !== null);
        mounted?.focus();
      })
      .catch((failure: unknown) => {
        if (!controller.signal.aborted) {
          setError(
            failure instanceof Error ? failure.message : "The formula editor could not load.",
          );
        }
      });
    return () => {
      controller.abort();
      if (lifecycle.current === controller) lifecycle.current = null;
      input.current?.dispose();
      input.current = null;
      // StrictMode repeats effects. Cleanup releases the view, not the session.
    };
  }, [session, host, keyboardHost]);

  useEffect(() => {
    if (!ready || !isAndroid || androidKeyboardOpened.current || !input.current) return;
    androidKeyboardOpened.current = true;
    setKeyboardVisible(input.current.showKeyboard());
  }, [isAndroid, ready]);

  function cancel(reason: OverlayCloseReason): void {
    if (finished.current) return;
    if (
      (reason === "escape" || reason === "platform_back") &&
      input.current?.hideKeyboard()
    ) {
      setKeyboardVisible(false);
      return;
    }
    finished.current = true;
    lifecycle.current?.abort();
    input.current?.dispose();
    session.cancel();
    onClose();
  }

  function save(): void {
    if (!ready || finished.current) return;
    const invalid = formulaSourceError(source) || formulaRenderingError(source);
    if (invalid) {
      setError(invalid);
      return;
    }
    const result = session.save({ latex: source, placement });
    if (!result.ok) {
      setError(result.message);
      return;
    }
    finished.current = true;
    lifecycle.current?.abort();
    input.current?.dispose();
    onClose();
  }

  function toggleMathKeyboard(): void {
    if (!input.current) return;
    setKeyboardVisible(input.current.toggleKeyboard());
  }

  return (
    <Dialog
      title="Edit formula"
      width="large"
      initialFocusRef={initialFocus}
      onClose={cancel}
      description="Use the visual formula tools, mathematical keyboard, or type directly. Changes apply only when you save."
      actions={<>
        <Button type="button" variant="secondary" onClick={() => cancel("cancel_action")}>Cancel</Button>
        <Button
          type="button"
          variant="primary"
          disabled={!ready || !!formulaSourceError(source)}
          onClick={save}
        >
          Save formula
        </Button>
      </>}
    >
        <div className="tw:relative tw:min-w-0" data-formula-input-shell>
          <div ref={setHost} className={contentStyles.formulaInput} />
          <IconButton
            ref={initialFocus}
            icon="keyboard"
            label={keyboardVisible ? "Hide math keyboard" : "Show math keyboard"}
            tooltip={mobileRuntime ? false : keyboardVisible ? "Hide math keyboard" : "Show math keyboard"}
            disabled={!ready}
            aria-pressed={keyboardVisible}
            className="tw:absolute tw:top-2 tw:right-2 tw:z-(--ostudy-layer-raised)"
            onPointerDown={(event) => event.preventDefault()}
            onClick={toggleMathKeyboard}
          />
        </div>
        <div
          ref={setKeyboardHost}
          className={contentStyles.formulaKeyboard}
          data-formula-keyboard-host
        />
        {!ready && !error ? <p role="status">Loading formula editor…</p> : null}

        <FormulaToolbox
          input={ready ? input.current : null}
          disabled={!ready}
          showKeyboardHelp={showHardwareKeyboardHelp}
        />

        <div className="tw:grid tw:min-w-0 tw:gap-1">
          <span className="tw:text-caption tw:font-medium tw:text-muted-foreground">Preview</span>
          <FormulaPreview source={source} block={placement === "block"} />
        </div>
        {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}

        <label className="tw:grid tw:w-fit tw:gap-1 tw:text-small">
          Placement
          <NativeSelect
            aria-label="Formula placement"
            value={placement}
            disabled={!ready}
            onChange={(event) =>
              setPlacement(event.currentTarget.value as FormulaPlacement)
            }
          >
            <option value="inline">Inline with text</option>
            <option value="block">On its own line</option>
          </NativeSelect>
        </label>
    </Dialog>
  );
}
