import { useEffect, useMemo, useState } from "react";
import type { FormEvent, MouseEvent, ReactNode } from "react";
import { EditorContent, useEditor } from "@tiptap/react";

import { importImage } from "../../ipc/assets";
import { getErrorMessage } from "../../ipc/common";
import { managedAssetUrl } from "../../platform/assets";
import { selectExternalImport } from "../../platform/files";
import { Button } from "../../ui/primitives/Button";
import { Dialog } from "../../ui/overlays/Dialog";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import contentStyles from "../RichContent.module.css";
import styles from "../RichTextEditor.module.css";
import type { RichTextEditorProps } from "../RichTextEditor";
import {
  editorToolbarGroups,
  type EditorAdapter,
  type EditorBlockStyle,
  type EditorCommandRequest,
  type EditorTextAlignment,
} from "../adapter";
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

const FONT_SIZES = [8, 10, 12, 14, 16, 18, 20, 24, 30, 36, 48, 60, 72, 84, 96];
const SYMBOLS = ["±", "→", "←", "↔", "↑", "↓", "≤", "≥", "≠", "≈", "∞", "°", "µ", "α", "β", "γ", "δ", "Δ", "λ", "π", "Σ", "√", "×", "÷", "•", "✓", "✕"];

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
  minHeight = 180,
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

  const editor = useEditor({
    extensions,
    content: hydrated,
    editable: !disabled,
    editorProps: {
      attributes: {
        class: contentStyles.content,
        "aria-label": label,
      },
    },
    onUpdate: ({ editor: currentEditor }) => {
      const current = createTiptapEditorAdapter(currentEditor);
      onChange(current.document(), current.plainText());
      setToolbarVersion((value) => value + 1);
    },
    onSelectionUpdate: () => setToolbarVersion((value) => value + 1),
  });
  const adapter = useMemo(
    () => (editor ? createTiptapEditorAdapter(editor) : null),
    [editor],
  );
  const pairSide = editorSide(label);
  const pairId = pairSide && assetDraftId ? assetDraftId : null;
  void toolbarVersion;
  void pairVersion;

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
          onOpenLink={openLinkEditor}
          onOpenTable={() => setTableOpen(true)}
          onOpenChecklist={() => setChecklistOpen(true)}
          isImportingImage={isImportingImage}
          imageDraftReady={onRequestImage !== undefined || assetDraftId !== null}
          disabled={disabled}
        />
        {linkOpen ? (
          <form className={styles.linkPopover} onSubmit={saveLink}>
            <label>
              <span>Link address</span>
              <input
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
          </form>
        ) : null}
        <div className={styles.surface} style={{ minHeight }}>
          <EditorContent editor={editor} />
        </div>
      </div>
      {editorError ? <p className={styles.error}>{editorError}</p> : null}

      {tableOpen ? (
        <Dialog title="Insert table" width="small" onClose={() => setTableOpen(false)}>
          <form className={styles.insertForm} onSubmit={insertTable}>
            <div className={styles.insertGrid}>
              <label>
                <span>Rows</span>
                <input type="number" min={1} max={20} value={tableRows} onChange={(event) => setTableRows(Number(event.currentTarget.value))} />
              </label>
              <label>
                <span>Columns</span>
                <input type="number" min={1} max={12} value={tableColumns} onChange={(event) => setTableColumns(Number(event.currentTarget.value))} />
              </label>
            </div>
            <label className={styles.insertCheckbox}>
              <input type="checkbox" checked={tableHeader} onChange={(event) => setTableHeader(event.currentTarget.checked)} />
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
            <div className={styles.checklistDraft}>
              {checklistItems.map((item, index) => (
                <div className={styles.checklistDraftRow} key={item.id}>
                  <label className={styles.correctToggle}>
                    <input type="checkbox" checked={item.correct} aria-label={`Option ${index + 1} is correct`} onChange={(event) => setChecklistItems((current) => current.map((entry) => entry.id === item.id ? { ...entry, correct: event.currentTarget.checked } : entry))} />
                    Correct
                  </label>
                  <input type="text" value={item.text} aria-label={`Option ${index + 1}`} placeholder={`Option ${index + 1}`} onChange={(event) => setChecklistItems((current) => current.map((entry) => entry.id === item.id ? { ...entry, text: event.currentTarget.value } : entry))} />
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

interface EditorToolbarProps {
  adapter: EditorAdapter;
  onAddImage: () => void;
  onOpenLink: () => void;
  onOpenTable: () => void;
  onOpenChecklist: () => void;
  isImportingImage: boolean;
  imageDraftReady: boolean;
  disabled: boolean;
}

function EditorToolbar({ adapter, onAddImage, onOpenLink, onOpenTable, onOpenChecklist, isImportingImage, imageDraftReady, disabled }: EditorToolbarProps) {
  const selection = adapter.selection();
  const textStyle = adapter.attributes("textStyle");
  const historyGroup = editorToolbarGroups(adapter.capabilities).find((group) => group.id === "history");

  function preserveSelection(event: MouseEvent<HTMLButtonElement>) { event.preventDefault(); }
  function run(request: EditorCommandRequest): void { adapter.execute(request); }

  const advancedItems = (historyGroup?.items ?? []).map((item) => ({
    id: item.id,
    label: item.label,
    disabled: !adapter.canExecute({ command: item.command } as EditorCommandRequest),
    onSelect: () => run({ command: item.command } as EditorCommandRequest),
  }));

  return (
    <fieldset className={styles.toolbar} role="toolbar" aria-label="Text formatting" disabled={disabled}>
      <div className={styles.group}>
        <select aria-label="Paragraph style" value={selection.blockStyle} onChange={(event) => run({ command: "set_block", value: event.currentTarget.value as EditorBlockStyle })}>
          <option value="paragraph">Normal</option><option value="h1">Heading 1</option><option value="h2">Heading 2</option><option value="h3">Heading 3</option><option value="h4">Heading 4</option>
        </select>
        <select aria-label="Font family" value={typeof textStyle.fontFamily === "string" ? textStyle.fontFamily : ""} onChange={(event) => run({ command: "set_font_family", value: event.currentTarget.value || null })}>
          <option value="">Default font</option><option value="Arial, sans-serif">Arial</option><option value="Georgia, serif">Georgia</option><option value="'Times New Roman', serif">Times New Roman</option><option value="'Courier New', monospace">Courier New</option><option value="monospace">Monospace</option>
        </select>
        <select aria-label="Text size" value={typeof textStyle.fontSize === "string" ? textStyle.fontSize : ""} onChange={(event) => run({ command: "set_font_size", value: event.currentTarget.value || null })}>
          <option value="">Size</option>{FONT_SIZES.map((size) => <option key={size} value={`${size}px`}>{size}</option>)}
        </select>
      </div>
      <div className={styles.group}>
        <ToolbarButton label="Bold" active={adapter.isActive("bold")} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_bold" })}><strong>B</strong></ToolbarButton>
        <ToolbarButton label="Italic" active={adapter.isActive("italic")} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_italic" })}><em>I</em></ToolbarButton>
        <ToolbarButton label="Underline" active={adapter.isActive("underline")} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_underline" })}><u>U</u></ToolbarButton>
        <ToolbarButton label="Subscript" active={adapter.isActive("subscript")} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_subscript" })}>X<sub>2</sub></ToolbarButton>
        <ToolbarButton label="Superscript" active={adapter.isActive("superscript")} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_superscript" })}>X<sup>2</sup></ToolbarButton>
      </div>
      <div className={styles.group}>
        <ToolbarButton label="Bullet list" active={adapter.isActive("bulletList")} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_bullet_list" })}>• List</ToolbarButton>
        <ToolbarButton label="Numbered list" active={adapter.isActive("orderedList")} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_numbered_list" })}>1. List</ToolbarButton>
        <ToolbarButton label="Task list" active={adapter.isActive("taskList")} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_task_list" })}>☐ Tasks</ToolbarButton>
        {selection.inTaskList ? <ToolbarButton label={selection.taskChecked ? "Mark task incomplete" : "Mark task complete"} active={selection.taskChecked === true} onMouseDown={preserveSelection} onClick={() => run({ command: "toggle_task_checked" })}>✓</ToolbarButton> : null}
        <ToolbarButton label="Decrease indentation" onMouseDown={preserveSelection} onClick={() => run({ command: "indent", value: -1 })}>⇤</ToolbarButton>
        <ToolbarButton label="Increase indentation" onMouseDown={preserveSelection} onClick={() => run({ command: "indent", value: 1 })}>⇥</ToolbarButton>
      </div>
      <div className={styles.group}>
        {(["left", "center", "right"] as const).map((alignment) => <ToolbarButton key={alignment} label={`Align ${alignment}`} active={selection.textAlignment === alignment} onMouseDown={preserveSelection} onClick={() => run({ command: "set_text_alignment", value: alignment as EditorTextAlignment })}>{alignment === "left" ? "≡←" : alignment === "center" ? "≡" : "→≡"}</ToolbarButton>)}
      </div>
      <div className={`${styles.group} ${styles.colorTools}`}>
        <label title="Text color"><span>A</span><input type="color" aria-label="Text color" value={normalizeColor(textStyle.color, "#d9e2f2")} onChange={(event) => run({ command: "set_text_color", value: event.currentTarget.value })} /></label>
        <label title="Highlight color"><span>▰</span><input type="color" aria-label="Highlight color" value="#ffd166" onChange={(event) => run({ command: "set_highlight", value: event.currentTarget.value })} /></label>
        <ToolbarButton label="Remove text color and highlight" onMouseDown={preserveSelection} onClick={() => run({ command: "reset_colors" })}>Reset color</ToolbarButton>
      </div>
      <div className={styles.group}>
        <ToolbarButton label="Add or edit link" active={adapter.isActive("link")} onMouseDown={preserveSelection} onClick={onOpenLink}>Link</ToolbarButton>
        <ToolbarButton label="Insert image" onMouseDown={preserveSelection} onClick={onAddImage} disabled={isImportingImage || !imageDraftReady}>{isImportingImage ? "Adding…" : "Image"}</ToolbarButton>
        <select aria-label="Insert symbol" defaultValue="" onChange={(event) => { const symbol = event.currentTarget.value; if (symbol !== "") { run({ command: "insert_text", value: symbol }); event.currentTarget.value = ""; } }}><option value="">Symbol</option>{SYMBOLS.map((symbol) => <option key={symbol} value={symbol}>{symbol}</option>)}</select>
        <OverflowMenu label="Insert content" triggerPresentation="text" items={[{ id: "table", label: "Insert table", onSelect: onOpenTable }, { id: "answer-checklist", label: "Insert answer checklist", onSelect: onOpenChecklist }, { id: "divider", label: "Horizontal divider", onSelect: () => run({ command: "insert_horizontal_rule" }) }]} />
        <OverflowMenu label="Advanced formatting" triggerPresentation="text" items={advancedItems} />
      </div>
      {selection.inTable ? <div className={`${styles.group} ${styles.tableTools}`} aria-label="Table editing"><ToolbarButton label="Add table row" onClick={() => run({ command: "add_table_row" })}>+ Row</ToolbarButton><ToolbarButton label="Delete table row" onClick={() => run({ command: "delete_table_row" })}>− Row</ToolbarButton><ToolbarButton label="Add table column" onClick={() => run({ command: "add_table_column" })}>+ Column</ToolbarButton><ToolbarButton label="Delete table column" onClick={() => run({ command: "delete_table_column" })}>− Column</ToolbarButton><ToolbarButton label="Toggle table header row" onClick={() => run({ command: "toggle_table_header_row" })}>Header</ToolbarButton><ToolbarButton label="Delete table" onClick={() => run({ command: "delete_table" })}>Delete table</ToolbarButton></div> : null}
    </fieldset>
  );
}

interface ToolbarButtonProps { label: string; active?: boolean; disabled?: boolean; children: ReactNode; onClick: () => void; onMouseDown?: (event: MouseEvent<HTMLButtonElement>) => void; }
function ToolbarButton({ label, active = false, disabled = false, children, onClick, onMouseDown }: ToolbarButtonProps) { return <button type="button" className={`${styles.toolbarButton} ${active ? styles.toolbarButtonActive : ""}`.trim()} aria-label={label} title={label} aria-pressed={active} disabled={disabled} onMouseDown={onMouseDown} onClick={onClick}>{children}</button>; }
function normalizeColor(value: unknown, fallback: string): string { return typeof value === "string" && /^#[0-9a-f]{6}$/iu.test(value) ? value : fallback; }
function defaultChecklistItems(): ChecklistDraftItem[] { return [{ id: createItemId(), text: "", correct: false }, { id: createItemId(), text: "", correct: false }, { id: createItemId(), text: "", correct: false }]; }
function createItemId(): string { return crypto.randomUUID().replaceAll("-", ""); }
function editorSide(label: string): CardEditorSide | null { const normalized = label.trim().toLowerCase(); return normalized.startsWith("question") ? "question" : normalized.startsWith("answer") ? "answer" : null; }
