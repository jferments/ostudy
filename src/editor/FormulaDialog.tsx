import { useEffect, useRef, useState } from "react";
import { FormulaPreview } from "../math/FormulaPreview";
import { mountFormulaInput, type FormulaInput } from "../math/mathlive";
import { formulaRenderingError, formulaSourceError } from "../math/source";
import { Dialog, DialogActions } from "../ui/overlays/Dialog";
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
  const [source, setSource] = useState(session.initialValue.latex);
  const [placement, setPlacement] = useState(session.initialValue.placement);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Dialog content is portaled after the parent effect can first run. Callback
  // refs make node availability an explicit input to the mounting lifecycle.
  const [host, setHost] = useState<HTMLDivElement | null>(null);
  const [keyboardHost, setKeyboardHost] = useState<HTMLDivElement | null>(null);
  const input = useRef<FormulaInput | null>(null);
  const initialFocus = useRef<HTMLButtonElement>(null);
  const finished = useRef(false);
  const lifecycle = useRef<AbortController | null>(null);

  useEffect(() => {
    if (!host || !keyboardHost || finished.current) return;
    const controller = new AbortController();
    lifecycle.current = controller;
    setReady(false);
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

  function cancel(reason: OverlayCloseReason): void {
    if (finished.current) return;
    if (
      (reason === "escape" || reason === "platform_back") &&
      input.current?.hideKeyboard()
    ) {
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

  return (
    <Dialog
      title="Edit formula"
      width="large"
      initialFocusRef={initialFocus}
      onClose={cancel}
      description="Use the visual formula tools, mathematical keyboard, or type directly. Changes apply only when you save."
    >
      <div className="tw:box-border tw:grid tw:w-full tw:min-w-0 tw:gap-4 tw:p-4">
        <div ref={setHost} className={contentStyles.formulaInput} />
        {!ready && !error ? <p role="status">Loading formula editor…</p> : null}

        <div className="tw:flex tw:flex-wrap tw:items-end tw:gap-3">
          <Button
            ref={initialFocus}
            type="button"
            variant="secondary"
            disabled={!ready}
            onClick={() => input.current?.toggleKeyboard()}
          >
            Full math keyboard
          </Button>
          <label className="tw:grid tw:gap-1 tw:text-sm">
            Placement
            <NativeSelect
              aria-label="Formula placement"
              value={placement}
              disabled={!ready}
              onChange={(event) =>
                setPlacement(event.currentTarget.value as FormulaPlacement)
              }
            >
              <option value="inline">Within the text</option>
              <option value="block">On its own line</option>
            </NativeSelect>
          </label>
        </div>

        <FormulaToolbox input={ready ? input.current : null} disabled={!ready} />
        <div ref={setKeyboardHost} className={contentStyles.formulaKeyboard} />

        <div className="tw:grid tw:min-w-0 tw:gap-1">
          <span className="tw:text-xs tw:font-medium tw:text-muted-foreground">Preview</span>
          <FormulaPreview source={source} block={placement === "block"} />
        </div>
        {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
        <DialogActions>
          <Button type="button" variant="secondary" onClick={() => cancel("cancel_action")}>Cancel</Button>
          <Button
            type="button"
            variant="primary"
            disabled={!ready || !!formulaSourceError(source)}
            onClick={save}
          >
            Save formula
          </Button>
        </DialogActions>
      </div>
    </Dialog>
  );
}
