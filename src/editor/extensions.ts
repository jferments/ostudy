import { Extension, Node } from "@tiptap/core";
import Highlight from "@tiptap/extension-highlight";
import Image from "@tiptap/extension-image";
import Link from "@tiptap/extension-link";
import TextAlign from "@tiptap/extension-text-align";
import { TextStyleKit } from "@tiptap/extension-text-style";
import Underline from "@tiptap/extension-underline";
import StarterKit from "@tiptap/starter-kit";

import contentStyles from "./RichContent.module.css";

export const TEMPLATE_FIELD_INLINE = "templateFieldInline";
export const TEMPLATE_FIELD_BLOCK = "templateFieldBlock";

const Indent = Extension.create({
  name: "studyIndent",
  addGlobalAttributes() {
    return [
      {
        types: ["paragraph", "heading"],
        attributes: {
          indent: {
            default: 0,
            parseHTML: (element: HTMLElement) => {
              const parsed = Number.parseInt(
                element.getAttribute("data-indent") ?? "0",
                10,
              );
              return Number.isFinite(parsed)
                ? Math.min(6, Math.max(0, parsed))
                : 0;
            },
            renderHTML: (attributes: Record<string, unknown>) => {
              const value = Number(attributes.indent ?? 0);
              return value > 0 ? { "data-indent": String(value) } : {};
            },
          },
        },
      },
    ];
  },
});

export const StudyImage = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      assetId: {
        default: null,
        parseHTML: (element: HTMLElement) =>
          element.getAttribute("data-study-asset"),
        renderHTML: (attributes: Record<string, unknown>) => {
          const assetId = attributes.assetId;
          return typeof assetId === "string" && assetId !== ""
            ? { "data-study-asset": assetId }
            : {};
        },
      },
    };
  },
});

const TemplateFieldInline = Node.create({
  name: TEMPLATE_FIELD_INLINE,
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,
  addAttributes() {
    return {
      fieldId: { default: null },
      label: { default: "Field" },
    };
  },
  parseHTML() {
    return [{ tag: "span[data-template-field-inline]" }];
  },
  renderHTML({ HTMLAttributes }) {
    const label =
      typeof HTMLAttributes.label === "string" ? HTMLAttributes.label : "Field";
    return [
      "span",
      {
        class: `${contentStyles.templateFieldPlaceholder} ${contentStyles.inlinePlaceholder}`,
        "data-template-field-inline": "true",
        "data-field-id": HTMLAttributes.fieldId,
        contenteditable: "false",
      },
      `{{${label}}}`,
    ];
  },
});

const TemplateFieldBlock = Node.create({
  name: TEMPLATE_FIELD_BLOCK,
  group: "block",
  atom: true,
  selectable: true,
  addAttributes() {
    return {
      fieldId: { default: null },
      label: { default: "Field" },
    };
  },
  parseHTML() {
    return [{ tag: "div[data-template-field-block]" }];
  },
  renderHTML({ HTMLAttributes }) {
    const label =
      typeof HTMLAttributes.label === "string" ? HTMLAttributes.label : "Field";
    return [
      "div",
      {
        class: `${contentStyles.templateFieldPlaceholder} ${contentStyles.blockPlaceholder}`,
        "data-template-field-block": "true",
        "data-field-id": HTMLAttributes.fieldId,
        contenteditable: "false",
      },
      `{{${label}}}`,
    ];
  },
});

interface EditorExtensionOptions {
  templatePlaceholders?: boolean;
}

export function createEditorExtensions(
  options: EditorExtensionOptions = { templatePlaceholders: true },
) {
  const extensions = [
    StarterKit.configure({
      heading: { levels: [1, 2, 3, 4] },
      link: false,
      underline: false,
    }),
    Link.configure({
      openOnClick: false,
      autolink: true,
      defaultProtocol: "https",
      HTMLAttributes: {
        rel: "noopener noreferrer",
      },
    }),
    Underline,
    Highlight.configure({ multicolor: true }),
    TextStyleKit,
    TextAlign.configure({ types: ["heading", "paragraph"] }),
    StudyImage.configure({
      allowBase64: false,
      HTMLAttributes: { class: contentStyles.image },
    }),
    Indent,
  ];
  if (options.templatePlaceholders !== false) {
    extensions.push(TemplateFieldInline, TemplateFieldBlock);
  }
  return extensions;
}
