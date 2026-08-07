import { convertFileSrc } from "@tauri-apps/api/core";
import Highlight from "@tiptap/extension-highlight";
import Image from "@tiptap/extension-image";
import Link from "@tiptap/extension-link";
import { TextStyle } from "@tiptap/extension-text-style";
import Underline from "@tiptap/extension-underline";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { useEffect, useMemo } from "react";

import {
  type Asset,
} from "../ipc/assets";
import {
  type RichDocument,
} from "../ipc/common";

const ManagedImage = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      assetId: {
        default: null,
        parseHTML: (element) => element.getAttribute("data-asset-id"),
        renderHTML: (attributes) =>
          attributes.assetId ? { "data-asset-id": attributes.assetId } : {},
      },
    };
  },
});

interface StudyRichEditorProps {
  value: RichDocument;
  assets: Asset[];
  disabled?: boolean;
  label: string;
  onChange: (document: RichDocument, text: string) => void;
  onRequestImage?: () => Promise<Asset | null>;
}

export function StudyRichEditor({
  value,
  assets,
  disabled = false,
  label,
  onChange,
  onRequestImage,
}: StudyRichEditorProps) {
  const assetById = useMemo(() => new Map(assets.map((asset) => [asset.id, asset])), [assets]);
  const editorValue = useMemo(() => documentForEditor(value, assetById), [assetById, value]);
  const editor = useEditor({
    extensions: [
      StarterKit,
      Underline,
      Highlight,
      TextStyle,
      Link.configure({ openOnClick: false, autolink: false }),
      ManagedImage.configure({ inline: false, allowBase64: false }),
    ],
    content: editorValue,
    editable: !disabled,
    onUpdate({ editor: current }) {
      const canonical = canonicalDocument(current.getJSON());
      onChange(canonical, extractText(canonical));
    },
  });

  useEffect(() => {
    editor?.setEditable(!disabled);
  }, [disabled, editor]);

  useEffect(() => {
    if (!editor) return;
    const current = JSON.stringify(canonicalDocument(editor.getJSON()));
    const incoming = JSON.stringify(canonicalDocument(value));
    if (current !== incoming) editor.commands.setContent(editorValue, { emitUpdate: false });
  }, [editor, editorValue, value]);

  if (!editor) return <div className="inline-progress">Opening editor…</div>;

  async function addImage(): Promise<void> {
    if (!onRequestImage) return;
    const asset = await onRequestImage();
    if (!asset) return;
    editor
      .chain()
      .focus()
      .setImage({
        src: convertFileSrc(asset.filePath),
        alt: asset.originalFilename,
        assetId: asset.id,
      } as never)
      .run();
  }

  return (
    <div className="study-rich-editor" aria-label={label}>
      <div className="rich-editor-toolbar" role="toolbar" aria-label={`${label} formatting`}>
        <button type="button" disabled={disabled} aria-pressed={editor.isActive("bold")} onClick={() => editor.chain().focus().toggleBold().run()}><strong>B</strong></button>
        <button type="button" disabled={disabled} aria-pressed={editor.isActive("italic")} onClick={() => editor.chain().focus().toggleItalic().run()}><em>I</em></button>
        <button type="button" disabled={disabled} aria-pressed={editor.isActive("underline")} onClick={() => editor.chain().focus().toggleUnderline().run()}><u>U</u></button>
        <button type="button" disabled={disabled} aria-pressed={editor.isActive("bulletList")} onClick={() => editor.chain().focus().toggleBulletList().run()}>• List</button>
        <button type="button" disabled={disabled} aria-pressed={editor.isActive("orderedList")} onClick={() => editor.chain().focus().toggleOrderedList().run()}>1. List</button>
        <button type="button" disabled={disabled} aria-pressed={editor.isActive("blockquote")} onClick={() => editor.chain().focus().toggleBlockquote().run()}>Quote</button>
        <button type="button" disabled={disabled} aria-pressed={editor.isActive("codeBlock")} onClick={() => editor.chain().focus().toggleCodeBlock().run()}>Code</button>
        <button type="button" disabled={disabled} aria-pressed={editor.isActive("heading", { level: 2 })} onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}>Heading</button>
        {onRequestImage ? <button type="button" disabled={disabled} onClick={() => void addImage()}>Image</button> : null}
      </div>
      <EditorContent editor={editor} />
    </div>
  );
}

export function emptyRichDocument(): RichDocument {
  return { type: "doc", content: [{ type: "paragraph" }] };
}

export function plainTextDocument(value: string): RichDocument {
  return {
    type: "doc",
    content: value.split("\n").map((line) => ({
      type: "paragraph",
      content: line ? [{ type: "text", text: line }] : undefined,
    })),
  };
}

export function extractText(value: RichDocument): string {
  const parts: string[] = [];
  visit(value, (node) => {
    if (node.type === "text" && typeof node.text === "string") parts.push(node.text);
    else if (node.type === "hardBreak") parts.push("\n");
    else if (
      ["paragraph", "heading", "blockquote", "listItem", "codeBlock"].includes(
        typeof node.type === "string" ? node.type : "",
      )
    ) {
      if (parts.length > 0 && parts[parts.length - 1] !== "\n") parts.push("\n");
    }
  });
  return parts.join("").replace(/\n{3,}/gu, "\n\n").trim();
}

function documentForEditor(value: RichDocument, assets: Map<string, Asset>): RichDocument {
  return mapDocument(value, (node) => {
    if (node.type !== "image") return node;
    const attrs = asRecord(node.attrs);
    const assetId = typeof attrs.assetId === "string" ? attrs.assetId : null;
    const asset = assetId ? assets.get(assetId) : undefined;
    return {
      ...node,
      attrs: {
        ...attrs,
        src: asset ? convertFileSrc(asset.filePath) : attrs.src,
        alt: attrs.alt ?? asset?.originalFilename ?? "Image",
      },
    };
  });
}

function canonicalDocument(value: RichDocument): RichDocument {
  return mapDocument(value, (node) => {
    if (node.type !== "image") return node;
    const attrs = asRecord(node.attrs);
    const { src: _src, ...rest } = attrs;
    return { ...node, attrs: rest };
  });
}

function mapDocument(
  value: RichDocument,
  transform: (node: Record<string, unknown>) => Record<string, unknown>,
): RichDocument {
  const mapNode = (unknownNode: unknown): unknown => {
    if (!unknownNode || typeof unknownNode !== "object" || Array.isArray(unknownNode)) {
      return unknownNode;
    }
    const node = unknownNode as Record<string, unknown>;
    const content = Array.isArray(node.content) ? node.content.map(mapNode) : node.content;
    return transform(content === undefined ? { ...node } : { ...node, content });
  };
  return mapNode(value) as RichDocument;
}

function visit(value: unknown, visitor: (node: Record<string, unknown>) => void): void {
  if (!value || typeof value !== "object" || Array.isArray(value)) return;
  const node = value as Record<string, unknown>;
  visitor(node);
  if (Array.isArray(node.content)) for (const child of node.content) visit(child, visitor);
}

function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}
