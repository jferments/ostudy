import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Checkbox } from "../../ui/primitives/FormControls";
import { TextField } from "../../ui/primitives/TextField";
import { useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent } from "react";
import { EditorContent, useEditor } from "@tiptap/react";
import type { Editor } from "@tiptap/core";
import { NodeSelection } from "@tiptap/pm/state";
import { FormulaDialog } from "../FormulaDialog";
import type { FormulaEditSession } from "../formula";
import { EditorToolbar } from "./EditorToolbar";

import { importImage, importMedia } from "../../ipc/assets";
import { getErrorMessage } from "../../ipc/common";
import { managedAssetUrl } from "../../platform/assets";
import { selectExternalImport } from "../../platform/files";
import { Button } from "../../ui/primitives/Button";
import { Dialog } from "../../ui/overlays/Dialog";
import contentStyles from "../RichContent.module.css";
import { editorStyles as styles } from "../RichTextEditor.presentation";
import type { RichTextEditorProps } from "../RichTextEditor";
import {
  cardEditorPairCanCopy,
  copyCardEditorChecklistToAnswer,
  registerCardEditorPairMember,
  subscribeCardEditorPair,
  type CardEditorSide,
} from "../editorPairRegistry";
import { createEditorExtensions } from "../extensions";
import { isAllowedExternalLink, normalizeExternalLink } from "../links";
import { canonicalizeDocument, hydrateDocument } from "../richText";
import { createTiptapEditorAdapter } from "../tiptapAdapter";


interface ChecklistDraftItem {
  id: string;
  text: string;
  correct: boolean;
}

export function TiptapRichTextEditor({
  label,
  description,
  document,
  assets,
  assetDraftId = null,
  onChange,
  onAssetImported,
  onRequestImage,
  minHeight = 120,
  disabled = false,
}: RichTextEditorProps) {
  const extensions = useMemo(() => createEditorExtensions(), []);
  const canonicalIncoming = useMemo(() => canonicalizeDocument(document), [document]);
  const hydrated = useMemo(
    () => hydrateDocument(canonicalIncoming, assets),
    [assets, canonicalIncoming],
  );
  const [toolbarVersion, setToolbarVersion] = useState(0);
  const [pairVersion, setPairVersion] = useState(0);
  const [linkOpen, setLinkOpen] = useState(false);
  const [linkValue, setLinkValue] = useState("");
  const [tableOpen, setTableOpen] = useState(false);
  const [tableRows, setTableRows] = useState(3);
  const [tableColumns, setTableColumns] = useState(3);
  const [tableHeader, setTableHeader] = useState(true);
  const [checklistOpen, setChecklistOpen] = useState(false);
  const [checklistItems, setChecklistItems] = useState<ChecklistDraftItem[]>(() =>
    defaultChecklistItems(),
  );
  const [editorError, setEditorError] = useState<string | null>(null);
  const [isImportingImage, setIsImportingImage] = useState(false);
  const [isImportingMedia, setIsImportingMedia] = useState(false);
  const [formulaSession, setFormulaSession] = useState<FormulaEditSession | null>(null);
  const editorRef = useRef<Editor | null>(null);

  function openFormulaEditor(): void {
    const current = editorRef.current;
    if (!current || !current.isEditable || current.isDestroyed) return;
    const session = createTiptapEditorAdapter(current).beginFormulaEdit();
    if (!session) { setEditorError("Select text or a formula before opening the formula editor."); return; }
    setEditorError(null);
    setFormulaSession(session);
  }

  const editor = useEditor({
    extensions,
    content: hydrated,
    editable: !disabled,
    editorProps: {
      attributes: {
        class: contentStyles.content,
        "aria-label": label,
        role: "textbox",
        "aria-multiline": "true",
        // The contenteditable itself fills the visible surface, not just its first line.
        style: `min-height: ${Math.max(48, (Number.isFinite(minHeight) ? minHeight : 120) - 32)}px; outline: none`,
      },
      handleDoubleClickOn(view, _pos, node, nodePos, _event, direct) {
        if (!direct || !view.editable || !["inlineMath", "blockMath"].includes(node.type.name)) return false;
        view.dispatch(view.state.tr.setSelection(NodeSelection.create(view.state.doc, nodePos)));
        openFormulaEditor();
        return true;
      },
      handleKeyDown(view, event) {
        const selection = view.state.selection;
        if (event.key !== "Enter" || !view.editable || !(selection instanceof NodeSelection)
          || !["inlineMath", "blockMath"].includes(selection.node.type.name)) return false;
        openFormulaEditor();
        return true;
      },
    },
    onUpdate: ({ editor: currentEditor }) => {
      const current = createTiptapEditorAdapter(currentEditor);
      onChange(current.document(), current.plainText());
      setToolbarVersion((value) => value + 1);
    },
    onSelectionUpdate: () => setToolbarVersion((value) => value + 1),
  });
  editorRef.current = editor;
  const adapter = useMemo(
    () => (editor ? createTiptapEditorAdapter(editor) : null),
    [editor],
  );
  const pairSide = editorSide(label);
  const pairId = pairSide && assetDraftId ? assetDraftId : null;
  void toolbarVersion;
  void pairVersion;

  useEffect(() => {
    if (disabled && formulaSession) {
      formulaSession.cancel();
      setFormulaSession(null);
    }
  }, [disabled, formulaSession]);

  useEffect(() => {
    if (!adapter) return;
    adapter.setEditable(!disabled);
    if (disabled) setLinkOpen(false);
  }, [adapter, disabled]);

  useEffect(() => {
    if (!adapter) return;
    if (JSON.stringify(adapter.document()) !== JSON.stringify(canonicalIncoming)) {
      adapter.setDocument(hydrated, false);
    }
  }, [adapter, canonicalIncoming, hydrated]);

  useEffect(() => {
    if (!adapter || !pairId || !pairSide) return;
    const unregister = registerCardEditorPairMember(pairId, pairSide, {
      adapter,
      apply(nextDocument, plainText) {
        adapter.setDocument(hydrateDocument(nextDocument, assets), false);
        onChange(nextDocument, plainText);
        setToolbarVersion((value) => value + 1);
      },
    });
    const unsubscribe = subscribeCardEditorPair(pairId, () => {
      setPairVersion((value) => value + 1);
    });
    return () => {
      unsubscribe();
      unregister();
    };
  }, [adapter, assets, onChange, pairId, pairSide]);

  async function addImage(): Promise<void> {
    if (!adapter || disabled || isImportingImage) return;
    setEditorError(null);
    setIsImportingImage(true);
    try {
      let asset;
      if (onRequestImage) {
        asset = await onRequestImage();
      } else {
        if (!assetDraftId) {
          setEditorError("The managed image draft is still opening. Try again in a moment.");
          return;
        }
        const selected = await selectExternalImport("image");
        if (!selected) return;
        asset = await importImage(selected.id, assetDraftId);
      }
      if (!asset) return;
      onAssetImported?.(asset);
      adapter.execute({
        command: "insert_image",
        value: {
          src: managedAssetUrl(asset.id),
          alt: asset.originalFilename,
          title: asset.originalFilename,
          assetId: asset.id,
        },
      });
    } catch (error: unknown) {
      setEditorError(getErrorMessage(error));
    } finally {
      setIsImportingImage(false);
    }
  }

  async function addMedia(): Promise<void> {
    const current = editorRef.current;
    if (!current || disabled || isImportingMedia) return;
    if (!assetDraftId) {
      setEditorError("The managed asset draft is still opening. Try again in a moment.");
      return;
    }
    setEditorError(null);
    setIsImportingMedia(true);
    try {
      const asset = await importMedia(assetDraftId);
      if (!asset) return;
      const type = asset.mimeType.startsWith("audio/")
        ? "audio"
        : asset.mimeType.startsWith("video/")
          ? "video"
          : null;
      if (!type) throw new Error("The selected managed asset is not audio or video.");
      onAssetImported?.(asset);
      current.chain().focus().insertContent({
        type,
        attrs: { assetId: asset.id, src: managedAssetUrl(asset.id) },
      }).run();
    } catch (error: unknown) {
      setEditorError(getErrorMessage(error));
    } finally {
      setIsImportingMedia(false);
    }
  }

  function openLinkEditor(): void {
    if (!adapter || disabled) return;
    const href = adapter.attributes("link").href;
    setLinkValue(typeof href === "string" ? href : "");
    setLinkOpen(true);
  }

  function saveLink(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    if (!adapter || disabled) return;
    const href = normalizeExternalLink(linkValue);
    if (href === "") {
      adapter.execute({ command: "unset_link" });
      setEditorError(null);
      setLinkOpen(false);
      return;
    }
    if (!isAllowedExternalLink(href)) {
      setEditorError("Enter a complete http, https, mailto, or tel address.");
      return;
    }
    adapter.execute({ command: "set_link", value: href });
    setEditorError(null);
    setLinkOpen(false);
  }

  function insertTable(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    if (!adapter) return;
    adapter.execute({
      command: "insert_table",
      value: { rows: tableRows, columns: tableColumns, headerRow: tableHeader },
    });
    setTableOpen(false);
  }

  function insertChecklist(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    if (!adapter) return;
    const items = checklistItems
      .map((item) => ({ ...item, text: item.text.trim() }))
      .filter((item) => item.text !== "");
    if (items.length < 2) {
      setEditorError("An answer checklist needs at least two non-empty options.");
      return;
    }
    adapter.execute({ command: "insert_answer_checklist", value: { items } });
    setChecklistOpen(false);
    setChecklistItems(defaultChecklistItems());
    setEditorError(null);
  }

  function copyChecklist(): void {
    if (!pairId) {
      setEditorError("The question and answer editors are still opening.");
      return;
    }
    setEditorError(copyCardEditorChecklistToAnswer(pairId));
  }

  if (!adapter) {
    return (
      <section className={styles.field}>
        <div className={styles.heading}>
          <div>
            <h3>{label}</h3>
            {description ? <p>{description}</p> : null}
          </div>
        </div>
        <div className={styles.loading} role="status">Loading editor…</div>
      </section>
    );
  }

  return (
    <section className={styles.field}>
      <div className={styles.heading}>
        <div>
          <h3>{label}</h3>
          {description ? <p>{description}</p> : null}
        </div>
        {pairSide === "question" && pairId && cardEditorPairCanCopy(pairId) ? (
          <Button type="button" variant="secondary" size="small" disabled={disabled} onClick={copyChecklist}>
            Copy checklist to answer
          </Button>
        ) : null}
      </div>

      <div className={styles.shell}>
        <EditorToolbar
          adapter={adapter}
          onAddImage={() => void addImage()}
          onAddMedia={() => void addMedia()}
          onOpenLink={openLinkEditor}
          onOpenFormula={openFormulaEditor}
          onOpenTable={() => setTableOpen(true)}
          onOpenChecklist={() => setChecklistOpen(true)}
          isImportingImage={isImportingImage}
          isImportingMedia={isImportingMedia}
          imageDraftReady={onRequestImage !== undefined || assetDraftId !== null}
          mediaDraftReady={assetDraftId !== null}
          disabled={disabled}
        />
        {linkOpen ? (
          <Dialog title="Edit link" width="small" onClose={() => setLinkOpen(false)}><form className={styles.insertForm} onSubmit={saveLink}>
            <label className="tw:grid tw:gap-2 tw:text-label">
              <span>Link address</span>
              <TextField
                autoFocus
                disabled={disabled}
                type="text"
                value={linkValue}
                onChange={(event) => setLinkValue(event.currentTarget.value)}
                placeholder="https://example.org"
              />
            </label>
            <div className={styles.linkActions}>
              <Button type="button" variant="text" disabled={disabled} onClick={() => setLinkOpen(false)}>
                Cancel
              </Button>
              {adapter.isActive("link") ? (
                <Button
                  type="button"
                  variant="dangerGhost"
                  disabled={disabled}
                  onClick={() => {
                    adapter.execute({ command: "unset_link" });
                    setEditorError(null);
                    setLinkOpen(false);
                  }}
                >
                  Remove link
                </Button>
              ) : null}
              <Button type="submit" variant="primary" size="small" disabled={disabled}>
                Apply
              </Button>
            </div>
            {editorError ? <InlineNotice kind="error">{editorError}</InlineNotice> : null}
          </form></Dialog>
        ) : null}
        <div className={styles.surface} style={{ minHeight }} onClick={(event) => {
          // Padding/empty wrapper clicks focus the document. Clicks within its
          // actual content retain native caret placement, selection and media controls.
          const current = editorRef.current;
          if (!disabled && current && !current.isDestroyed && event.target instanceof Node
            && !current.view.dom.contains(event.target)) current.commands.focus("end");
        }}>
          <EditorContent editor={editor} />
        </div>
      </div>
      {editorError && !linkOpen && !checklistOpen ? <InlineNotice kind="error">{editorError}</InlineNotice> : null}

      {formulaSession ? <FormulaDialog session={formulaSession} onClose={() => setFormulaSession(null)} /> : null}

      {tableOpen ? (
        <Dialog title="Insert table" width="small" onClose={() => setTableOpen(false)}>
          <form className={styles.insertForm} onSubmit={insertTable}>
            <div className={styles.insertGrid}>
              <label>
                <span>Rows</span>
                <TextField type="number" min={1} max={20} value={tableRows} onChange={(event) => setTableRows(Number(event.currentTarget.value))} />
              </label>
              <label>
                <span>Columns</span>
                <TextField type="number" min={1} max={12} value={tableColumns} onChange={(event) => setTableColumns(Number(event.currentTarget.value))} />
              </label>
            </div>
            <label className={styles.insertCheckbox}>
              <Checkbox checked={tableHeader} onCheckedChange={(checked) => setTableHeader(checked)} />
              Use the first row as a header
            </label>
            <div className={styles.insertActions}>
              <Button type="button" variant="secondary" onClick={() => setTableOpen(false)}>Cancel</Button>
              <Button type="submit" variant="primary">Insert table</Button>
            </div>
          </form>
        </Dialog>
      ) : null}

      {checklistOpen ? (
        <Dialog title="Insert answer checklist" width="medium" onClose={() => setChecklistOpen(false)}>
          <form className={styles.insertForm} onSubmit={insertChecklist}>
            <p className={styles.insertHelp}>
              Mark the correct options here. Copy the checklist to the answer before saving; the
              question then shows empty boxes and the answer reveals the marked boxes.
            </p>
            {editorError ? <InlineNotice kind="error">{editorError}</InlineNotice> : null}
            <div className={styles.checklistDraft}>
              {checklistItems.map((item, index) => (
                <div className={styles.checklistDraftRow} key={item.id}>
                  <label className={styles.correctToggle}>
                    <Checkbox checked={item.correct} aria-label={`Option ${index + 1} is correct`} onCheckedChange={(checked) => setChecklistItems((current) => current.map((entry) => entry.id === item.id ? { ...entry, correct: checked } : entry))} />
                    Correct
                  </label>
                  <TextField type="text" value={item.text} aria-label={`Option ${index + 1}`} placeholder={`Option ${index + 1}`} onChange={(event) => { const text = event.currentTarget.value; setChecklistItems((current) => current.map((entry) => entry.id === item.id ? { ...entry, text } : entry)); }} />
                  <Button type="button" variant="dangerGhost" size="small" disabled={checklistItems.length <= 2} onClick={() => setChecklistItems((current) => current.filter((entry) => entry.id !== item.id))}>Remove</Button>
                </div>
              ))}
            </div>
            <Button type="button" variant="secondary" size="small" disabled={checklistItems.length >= 12} onClick={() => setChecklistItems((current) => [...current, { id: createItemId(), text: "", correct: false }])}>Add option</Button>
            <div className={styles.insertActions}>
              <Button type="button" variant="secondary" onClick={() => setChecklistOpen(false)}>Cancel</Button>
              <Button type="submit" variant="primary">Insert checklist</Button>
            </div>
          </form>
        </Dialog>
      ) : null}
    </section>
  );
}

function defaultChecklistItems(): ChecklistDraftItem[] { return [{ id: createItemId(), text: "", correct: false }, { id: createItemId(), text: "", correct: false }, { id: createItemId(), text: "", correct: false }]; }
function createItemId(): string { return crypto.randomUUID().replaceAll("-", ""); }
function editorSide(label: string): CardEditorSide | null { const normalized = label.trim().toLowerCase(); return normalized.startsWith("question") ? "question" : normalized.startsWith("answer") ? "answer" : null; }
