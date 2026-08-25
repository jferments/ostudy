import { useEffect, useMemo, useState } from "react";
import type { FormEvent, MouseEvent, ReactNode } from "react";
import type { Editor } from "@tiptap/core";
import { EditorContent, useEditor } from "@tiptap/react";

import {
  importImage,
  type Asset,
} from "../ipc/assets";
import {
  getErrorMessage,
  type RichDocument,
} from "../ipc/common";
import { managedAssetUrl } from "../platform/assets";
import { selectExternalImport } from "../platform/files";
import { Button } from "../ui/primitives/Button";
import contentStyles from "./RichContent.module.css";
import styles from "./RichTextEditor.module.css";
import { createEditorExtensions } from "./extensions";
import { isAllowedExternalLink, normalizeExternalLink } from "./links";
import { canonicalizeDocument, hydrateDocument } from "./richText";

interface RichTextEditorProps {
  label: string;
  description?: string;
  document: RichDocument;
  assets: Asset[];
  assetDraftId?: string | null;
  onChange: (document: RichDocument, plainText: string) => void;
  onAssetImported: (asset: Asset) => void;
  minHeight?: number;
  disabled?: boolean;
}

const FONT_SIZES = [8, 10, 12, 14, 16, 18, 20, 24, 30, 36, 48, 60, 72, 84, 96];
const SYMBOLS = ["±", "→", "←", "↔", "↑", "↓", "≤", "≥", "≠", "≈", "∞", "°", "µ", "α", "β", "γ", "δ", "Δ", "λ", "π", "Σ", "√", "×", "÷", "•", "✓", "✕"];

export function RichTextEditor({
  label,
  description,
  document,
  assets,
  assetDraftId = null,
  onChange,
  onAssetImported,
  minHeight = 180,
  disabled = false,
}: RichTextEditorProps) {
  const extensions = useMemo(() => createEditorExtensions(), []);
  const [toolbarVersion, setToolbarVersion] = useState(0);
  const [linkOpen, setLinkOpen] = useState(false);
  const [linkValue, setLinkValue] = useState("");
  const [editorError, setEditorError] = useState<string | null>(null);
  const [isImportingImage, setIsImportingImage] = useState(false);

  const editor = useEditor({
    extensions,
    content: hydrateDocument(document, assets),
    editable: !disabled,
    editorProps: {
      attributes: {
        class: contentStyles.content,
        "aria-label": label,
      },
    },
    onUpdate: ({ editor: currentEditor }) => {
      onChange(
        canonicalizeDocument(currentEditor.getJSON() as RichDocument),
        currentEditor.getText({ blockSeparator: "\n" }),
      );
      setToolbarVersion((value) => value + 1);
    },
    onSelectionUpdate: () => {
      setToolbarVersion((value) => value + 1);
    },
  });

  void toolbarVersion;

  useEffect(() => {
    if (!editor) return;
    editor.setEditable(!disabled, false);
    if (disabled) setLinkOpen(false);
  }, [disabled, editor]);

  async function addImage(): Promise<void> {
    if (!editor || disabled || isImportingImage) return;
    if (!assetDraftId) {
      setEditorError("The managed image draft is still opening. Try again in a moment.");
      return;
    }

    setEditorError(null);
    setIsImportingImage(true);
    try {
      const selected = await selectExternalImport("image");
      if (!selected) return;

      const asset = await importImage(selected.id, assetDraftId);
      onAssetImported(asset);
      editor
        .chain()
        .focus()
        .setImage({
          src: managedAssetUrl(asset.id),
          alt: asset.originalFilename,
          title: asset.originalFilename,
          assetId: asset.id,
        } as never)
        .run();
      setEditorError(null);
    } catch (error: unknown) {
      setEditorError(getErrorMessage(error));
    } finally {
      setIsImportingImage(false);
    }
  }

  function openLinkEditor(): void {
    if (!editor || disabled) return;
    const href = editor.getAttributes("link").href;
    setLinkValue(typeof href === "string" ? href : "");
    setLinkOpen(true);
  }

  function saveLink(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    if (!editor || disabled) return;
    const href = normalizeExternalLink(linkValue);
    if (href === "") {
      editor.chain().focus().extendMarkRange("link").unsetLink().run();
      setEditorError(null);
      setLinkOpen(false);
      return;
    }
    if (!isAllowedExternalLink(href)) {
      setEditorError("Enter a complete http, https, mailto, or tel address.");
      return;
    }
    editor.chain().focus().extendMarkRange("link").setLink({ href }).run();
    setEditorError(null);
    setLinkOpen(false);
  }

  if (!editor) {
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
      </div>

      <div className={styles.shell}>
        <EditorToolbar
          editor={editor}
          onAddImage={() => void addImage()}
          onOpenLink={openLinkEditor}
          isImportingImage={isImportingImage}
          imageDraftReady={assetDraftId !== null}
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
              {editor.isActive("link") ? (
                <Button
                  type="button"
                  variant="dangerGhost"
                  disabled={disabled}
                  onClick={() => {
                    editor.chain().focus().extendMarkRange("link").unsetLink().run();
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
    </section>
  );
}

interface EditorToolbarProps {
  editor: Editor;
  onAddImage: () => void;
  onOpenLink: () => void;
  isImportingImage: boolean;
  imageDraftReady: boolean;
  disabled: boolean;
}

function EditorToolbar({
  editor,
  onAddImage,
  onOpenLink,
  isImportingImage,
  imageDraftReady,
  disabled,
}: EditorToolbarProps) {
  const blockType = editor.isActive("heading", { level: 1 })
    ? "h1"
    : editor.isActive("heading", { level: 2 })
      ? "h2"
      : editor.isActive("heading", { level: 3 })
        ? "h3"
        : editor.isActive("heading", { level: 4 })
          ? "h4"
          : "paragraph";
  const textStyle = editor.getAttributes("textStyle");

  function preserveSelection(event: MouseEvent<HTMLButtonElement>) {
    event.preventDefault();
  }

  function changeBlock(value: string): void {
    if (value === "paragraph") {
      editor.chain().focus().setParagraph().run();
      return;
    }
    const level = Number(value.slice(1)) as 1 | 2 | 3 | 4;
    editor.chain().focus().toggleHeading({ level }).run();
  }

  function indent(delta: number): void {
    if (editor.isActive("bulletList") || editor.isActive("orderedList")) {
      if (delta > 0) {
        editor.chain().focus().sinkListItem("listItem").run();
      } else {
        editor.chain().focus().liftListItem("listItem").run();
      }
      return;
    }
    const nodeType = editor.isActive("heading") ? "heading" : "paragraph";
    const current = Number(editor.getAttributes(nodeType).indent ?? 0);
    editor
      .chain()
      .focus()
      .updateAttributes(nodeType, {
        indent: Math.min(6, Math.max(0, current + delta)),
      })
      .run();
  }

  return (
    <fieldset className={styles.toolbar} role="toolbar" aria-label="Text formatting" disabled={disabled}>
      <div className={styles.group}>
        <select
          aria-label="Paragraph style"
          value={blockType}
          onChange={(event) => changeBlock(event.currentTarget.value)}
        >
          <option value="paragraph">Normal</option>
          <option value="h1">Heading 1</option>
          <option value="h2">Heading 2</option>
          <option value="h3">Heading 3</option>
          <option value="h4">Heading 4</option>
        </select>
        <select
          aria-label="Font family"
          value={typeof textStyle.fontFamily === "string" ? textStyle.fontFamily : ""}
          onChange={(event) => {
            const value = event.currentTarget.value;
            if (value === "") {
              editor.chain().focus().unsetFontFamily().run();
            } else {
              editor.chain().focus().setFontFamily(value).run();
            }
          }}
        >
          <option value="">Default font</option>
          <option value="Arial, sans-serif">Arial</option>
          <option value="Georgia, serif">Georgia</option>
          <option value="'Times New Roman', serif">Times New Roman</option>
          <option value="'Courier New', monospace">Courier New</option>
          <option value="monospace">Monospace</option>
        </select>
        <select
          aria-label="Text size"
          value={typeof textStyle.fontSize === "string" ? textStyle.fontSize : ""}
          onChange={(event) => {
            const value = event.currentTarget.value;
            if (value === "") {
              editor.chain().focus().unsetFontSize().run();
            } else {
              editor.chain().focus().setFontSize(value).run();
            }
          }}
        >
          <option value="">Size</option>
          {FONT_SIZES.map((size) => (
            <option key={size} value={`${size}px`}>
              {size}
            </option>
          ))}
        </select>
      </div>

      <div className={styles.group}>
        <ToolbarButton
          label="Bold"
          active={editor.isActive("bold")}
          onMouseDown={preserveSelection}
          onClick={() => editor.chain().focus().toggleBold().run()}
        >
          <strong>B</strong>
        </ToolbarButton>
        <ToolbarButton
          label="Italic"
          active={editor.isActive("italic")}
          onMouseDown={preserveSelection}
          onClick={() => editor.chain().focus().toggleItalic().run()}
        >
          <em>I</em>
        </ToolbarButton>
        <ToolbarButton
          label="Underline"
          active={editor.isActive("underline")}
          onMouseDown={preserveSelection}
          onClick={() => editor.chain().focus().toggleUnderline().run()}
        >
          <u>U</u>
        </ToolbarButton>
        <ToolbarButton
          label="Strikethrough"
          active={editor.isActive("strike")}
          onMouseDown={preserveSelection}
          onClick={() => editor.chain().focus().toggleStrike().run()}
        >
          <s>S</s>
        </ToolbarButton>
        <ToolbarButton
          label="Inline code"
          active={editor.isActive("code")}
          onMouseDown={preserveSelection}
          onClick={() => editor.chain().focus().toggleCode().run()}
        >
          {"</>"}
        </ToolbarButton>
      </div>

      <div className={`${styles.group} ${styles.colorTools}`}>
        <label title="Text color">
          <span>A</span>
          <input
            type="color"
            aria-label="Text color"
            value={normalizeColor(textStyle.color, "#d9e2f2")}
            onChange={(event) => editor.chain().focus().setColor(event.currentTarget.value).run()}
          />
        </label>
        <label title="Highlight color">
          <span>▰</span>
          <input
            type="color"
            aria-label="Highlight color"
            value="#ffd166"
            onChange={(event) =>
              editor.chain().focus().setHighlight({ color: event.currentTarget.value }).run()
            }
          />
        </label>
        <ToolbarButton
          label="Remove text color and highlight"
          onMouseDown={preserveSelection}
          onClick={() =>
            editor.chain().focus().unsetColor().unsetHighlight().run()
          }
        >
          Reset color
        </ToolbarButton>
      </div>

      <div className={styles.group}>
        <ToolbarButton
          label="Bullet list"
          active={editor.isActive("bulletList")}
          onMouseDown={preserveSelection}
          onClick={() => editor.chain().focus().toggleBulletList().run()}
        >
          • List
        </ToolbarButton>
        <ToolbarButton
          label="Numbered list"
          active={editor.isActive("orderedList")}
          onMouseDown={preserveSelection}
          onClick={() => editor.chain().focus().toggleOrderedList().run()}
        >
          1. List
        </ToolbarButton>
        <ToolbarButton
          label="Block quote"
          active={editor.isActive("blockquote")}
          onMouseDown={preserveSelection}
          onClick={() => editor.chain().focus().toggleBlockquote().run()}
        >
          “ ”
        </ToolbarButton>
        <ToolbarButton
          label="Decrease indentation"
          onMouseDown={preserveSelection}
          onClick={() => indent(-1)}
        >
          ⇤
        </ToolbarButton>
        <ToolbarButton
          label="Increase indentation"
          onMouseDown={preserveSelection}
          onClick={() => indent(1)}
        >
          ⇥
        </ToolbarButton>
      </div>

      <div className={styles.group}>
        {(["left", "center", "right"] as const).map((alignment) => (
          <ToolbarButton
            key={alignment}
            label={`Align ${alignment}`}
            active={editor.isActive({ textAlign: alignment })}
            onMouseDown={preserveSelection}
            onClick={() => editor.chain().focus().setTextAlign(alignment).run()}
          >
            {alignment === "left" ? "≡←" : alignment === "center" ? "≡" : "→≡"}
          </ToolbarButton>
        ))}
      </div>

      <div className={styles.group}>
        <ToolbarButton
          label="Add or edit link"
          active={editor.isActive("link")}
          onMouseDown={preserveSelection}
          onClick={onOpenLink}
        >
          Link
        </ToolbarButton>
        <ToolbarButton
          label="Insert image"
          onMouseDown={preserveSelection}
          onClick={onAddImage}
          disabled={isImportingImage || !imageDraftReady}
        >
          {isImportingImage ? "Adding…" : "Image"}
        </ToolbarButton>
        <select
          aria-label="Insert symbol"
          defaultValue=""
          onChange={(event) => {
            const symbol = event.currentTarget.value;
            if (symbol !== "") {
              editor.chain().focus().insertContent(symbol).run();
              event.currentTarget.value = "";
            }
          }}
        >
          <option value="">Symbol</option>
          {SYMBOLS.map((symbol) => (
            <option key={symbol} value={symbol}>
              {symbol}
            </option>
          ))}
        </select>
        <ToolbarButton
          label="Horizontal divider"
          onMouseDown={preserveSelection}
          onClick={() => editor.chain().focus().setHorizontalRule().run()}
        >
          ―
        </ToolbarButton>
      </div>

      <div className={`${styles.group} ${styles.toolbarHistory}`}>
        <ToolbarButton
          label="Undo"
          onMouseDown={preserveSelection}
          onClick={() => editor.chain().focus().undo().run()}
          disabled={!editor.can().chain().focus().undo().run()}
        >
          ↶
        </ToolbarButton>
        <ToolbarButton
          label="Redo"
          onMouseDown={preserveSelection}
          onClick={() => editor.chain().focus().redo().run()}
          disabled={!editor.can().chain().focus().redo().run()}
        >
          ↷
        </ToolbarButton>
        <ToolbarButton
          label="Clear formatting"
          onMouseDown={preserveSelection}
          onClick={() => editor.chain().focus().unsetAllMarks().clearNodes().run()}
        >
          Clear
        </ToolbarButton>
      </div>
    </fieldset>
  );
}

interface ToolbarButtonProps {
  label: string;
  active?: boolean;
  disabled?: boolean;
  children: ReactNode;
  onClick: () => void;
  onMouseDown?: (event: MouseEvent<HTMLButtonElement>) => void;
}

function ToolbarButton({
  label,
  active = false,
  disabled = false,
  children,
  onClick,
  onMouseDown,
}: ToolbarButtonProps) {
  return (
    <button
      type="button"
      className={`${styles.toolbarButton} ${active ? styles.toolbarButtonActive : ""}`.trim()}
      aria-label={label}
      title={label}
      aria-pressed={active}
      disabled={disabled}
      onMouseDown={onMouseDown}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function normalizeColor(value: unknown, fallback: string): string {
  return typeof value === "string" && /^#[0-9a-f]{6}$/iu.test(value)
    ? value
    : fallback;
}
