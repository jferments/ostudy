import { Extension, Mark, Node } from "@tiptap/core";
import Highlight from "@tiptap/extension-highlight";
import Image from "@tiptap/extension-image";
import Link from "@tiptap/extension-link";
import TextAlign from "@tiptap/extension-text-align";
import { TextStyleKit } from "@tiptap/extension-text-style";
import Underline from "@tiptap/extension-underline";
import StarterKit from "@tiptap/starter-kit";

import { RICH_DOCUMENT_SCHEMA_VERSION } from "./adapter";
import contentStyles from "./RichContent.module.css";

export const TEMPLATE_FIELD_INLINE = "templateFieldInline";
export const TEMPLATE_FIELD_BLOCK = "templateFieldBlock";
export const ANSWER_KEY_MODE = "answer_key";
export const ORDINARY_TASK_MODE = "ordinary";

const DocumentVersion = Extension.create({
  name: "studyDocumentVersion",
  addGlobalAttributes() {
    return [
      {
        types: ["doc"],
        attributes: {
          schemaVersion: {
            default: RICH_DOCUMENT_SCHEMA_VERSION,
            parseHTML: (element: HTMLElement) =>
              Number(element.getAttribute("data-rich-schema-version")) ||
              RICH_DOCUMENT_SCHEMA_VERSION,
            renderHTML: (attributes: Record<string, unknown>) => ({
              "data-rich-schema-version": String(
                Number(attributes.schemaVersion) || RICH_DOCUMENT_SCHEMA_VERSION,
              ),
            }),
          },
        },
      },
    ];
  },
});

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

const StudySubscript = Mark.create({
  name: "subscript",
  excludes: "superscript",
  parseHTML() {
    return [{ tag: "sub" }];
  },
  renderHTML() {
    return ["sub", 0];
  },
  addKeyboardShortcuts() {
    return {
      "Mod-,": () => this.editor.commands.toggleMark(this.name),
    };
  },
});

const StudySuperscript = Mark.create({
  name: "superscript",
  excludes: "subscript",
  parseHTML() {
    return [{ tag: "sup" }];
  },
  renderHTML() {
    return ["sup", 0];
  },
  addKeyboardShortcuts() {
    return {
      "Mod-.": () => this.editor.commands.toggleMark(this.name),
    };
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

const StudyTable = Node.create({
  name: "table",
  group: "block",
  content: "tableRow+",
  isolating: true,
  parseHTML() {
    return [{ tag: "table[data-study-table]" }, { tag: "table" }];
  },
  renderHTML({ HTMLAttributes }) {
    return [
      "table",
      { ...HTMLAttributes, "data-study-table": "true" },
      ["tbody", 0],
    ];
  },
});

const StudyTableRow = Node.create({
  name: "tableRow",
  content: "(tableHeader|tableCell)+",
  parseHTML() {
    return [{ tag: "tr" }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["tr", HTMLAttributes, 0];
  },
});

function createTableCell(name: "tableHeader" | "tableCell", tag: "th" | "td") {
  return Node.create({
    name,
    content: "block+",
    isolating: true,
    addAttributes() {
      return {
        colspan: {
          default: 1,
          parseHTML: (element: HTMLElement) => Number(element.getAttribute("colspan") ?? 1),
          renderHTML: (attributes: Record<string, unknown>) =>
            Number(attributes.colspan ?? 1) > 1
              ? { colspan: String(attributes.colspan) }
              : {},
        },
        rowspan: {
          default: 1,
          parseHTML: (element: HTMLElement) => Number(element.getAttribute("rowspan") ?? 1),
          renderHTML: (attributes: Record<string, unknown>) =>
            Number(attributes.rowspan ?? 1) > 1
              ? { rowspan: String(attributes.rowspan) }
              : {},
        },
        colwidth: {
          default: null,
          parseHTML: (element: HTMLElement) => {
            const value = element.getAttribute("data-colwidth");
            return value
              ? value
                  .split(",")
                  .map(Number)
                  .filter((entry) => Number.isFinite(entry) && entry > 0)
              : null;
          },
          renderHTML: (attributes: Record<string, unknown>) =>
            Array.isArray(attributes.colwidth) && attributes.colwidth.length > 0
              ? { "data-colwidth": attributes.colwidth.join(",") }
              : {},
        },
      };
    },
    parseHTML() {
      return [{ tag }];
    },
    renderHTML({ HTMLAttributes }) {
      return [tag, HTMLAttributes, 0];
    },
  });
}

const StudyTableHeader = createTableCell("tableHeader", "th");
const StudyTableCell = createTableCell("tableCell", "td");

const StudyTaskList = Node.create({
  name: "taskList",
  group: "block",
  content: "taskItem+",
  addAttributes() {
    return {
      mode: {
        default: ORDINARY_TASK_MODE,
        parseHTML: (element: HTMLElement) =>
          element.getAttribute("data-checklist-mode") === ANSWER_KEY_MODE
            ? ANSWER_KEY_MODE
            : ORDINARY_TASK_MODE,
        renderHTML: (attributes: Record<string, unknown>) => ({
          "data-checklist-mode":
            attributes.mode === ANSWER_KEY_MODE ? ANSWER_KEY_MODE : ORDINARY_TASK_MODE,
        }),
      },
    };
  },
  parseHTML() {
    return [{ tag: "ul[data-task-list]" }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["ul", { ...HTMLAttributes, "data-task-list": "true" }, 0];
  },
});

const StudyTaskItem = Node.create({
  name: "taskItem",
  content: "paragraph block*",
  defining: true,
  addAttributes() {
    return {
      checked: {
        default: false,
        parseHTML: (element: HTMLElement) => element.getAttribute("data-checked") === "true",
        renderHTML: (attributes: Record<string, unknown>) => ({
          "data-checked": attributes.checked === true ? "true" : "false",
        }),
      },
      itemId: {
        default: null,
        parseHTML: (element: HTMLElement) => element.getAttribute("data-checklist-item-id"),
        renderHTML: (attributes: Record<string, unknown>) =>
          typeof attributes.itemId === "string" && attributes.itemId !== ""
            ? { "data-checklist-item-id": attributes.itemId }
            : {},
      },
    };
  },
  parseHTML() {
    return [{ tag: "li[data-task-item]" }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["li", { ...HTMLAttributes, "data-task-item": "true" }, 0];
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
    DocumentVersion,
    Indent,
    StudySubscript,
    StudySuperscript,
    StudyTable,
    StudyTableRow,
    StudyTableHeader,
    StudyTableCell,
    StudyTaskList,
    StudyTaskItem,
  ];
  if (options.templatePlaceholders !== false) {
    extensions.push(TemplateFieldInline, TemplateFieldBlock);
  }
  return extensions;
}
