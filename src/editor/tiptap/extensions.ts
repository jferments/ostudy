import { Extension, Mark, Node, mergeAttributes } from "@tiptap/core";
import Highlight from "@tiptap/extension-highlight";
import Image from "@tiptap/extension-image";
import Link from "@tiptap/extension-link";
import TextAlign from "@tiptap/extension-text-align";
import { TextStyleKit } from "@tiptap/extension-text-style";
import Underline from "@tiptap/extension-underline";
import StarterKit from "@tiptap/starter-kit";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { tableEditing } from "@tiptap/pm/tables";

import contentStyles from "../RichContent.module.css";

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
              const parsed = Number.parseInt(element.getAttribute("data-indent") ?? "0", 10);
              return Number.isFinite(parsed) ? Math.min(6, Math.max(0, parsed)) : 0;
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

const Subscript = Mark.create({
  name: "subscript",
  excludes: "superscript",
  parseHTML() {
    return [{ tag: "sub" }, { style: "vertical-align=sub" }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["sub", HTMLAttributes, 0];
  },
  addKeyboardShortcuts() {
    return {
      "Mod-,": () => this.editor.commands.toggleMark(this.name),
    };
  },
});

const Superscript = Mark.create({
  name: "superscript",
  excludes: "subscript",
  parseHTML() {
    return [{ tag: "sup" }, { style: "vertical-align=super" }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["sup", HTMLAttributes, 0];
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
        parseHTML: (element: HTMLElement) => element.getAttribute("data-study-asset"),
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

const Table = Node.create({
  name: "table",
  group: "block",
  content: "tableRow+",
  tableRole: "table",
  isolating: true,
  parseHTML() {
    return [{ tag: "table" }];
  },
  renderHTML({ HTMLAttributes }) {
    return [
      "div",
      { class: contentStyles.tableScroller, "data-table-scroller": "true" },
      ["table", mergeAttributes(HTMLAttributes), ["tbody", 0]],
    ];
  },
  addProseMirrorPlugins() {
    return [tableEditing({ allowTableNodeSelection: true })];
  },
});

const TableRow = Node.create({
  name: "tableRow",
  content: "(tableCell | tableHeader)+",
  tableRole: "row",
  parseHTML() {
    return [{ tag: "tr" }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["tr", HTMLAttributes, 0];
  },
});

function tableCellAttributes() {
  return {
    colspan: {
      default: 1,
      parseHTML: (element: HTMLElement) => positiveInteger(element.getAttribute("colspan"), 1),
      renderHTML: (attributes: Record<string, unknown>) =>
        Number(attributes.colspan ?? 1) === 1 ? {} : { colspan: String(attributes.colspan) },
    },
    rowspan: {
      default: 1,
      parseHTML: (element: HTMLElement) => positiveInteger(element.getAttribute("rowspan"), 1),
      renderHTML: (attributes: Record<string, unknown>) =>
        Number(attributes.rowspan ?? 1) === 1 ? {} : { rowspan: String(attributes.rowspan) },
    },
    colwidth: {
      default: null,
      parseHTML: (element: HTMLElement) => parseColumnWidths(element.getAttribute("data-colwidth")),
      renderHTML: (attributes: Record<string, unknown>) => {
        const values = Array.isArray(attributes.colwidth)
          ? attributes.colwidth.filter((value): value is number => Number.isInteger(value) && value > 0)
          : [];
        return values.length > 0 ? { "data-colwidth": values.join(",") } : {};
      },
    },
  };
}

const TableCell = Node.create({
  name: "tableCell",
  content: "block+",
  tableRole: "cell",
  isolating: true,
  addAttributes: tableCellAttributes,
  parseHTML() {
    return [{ tag: "td" }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["td", HTMLAttributes, 0];
  },
});

const TableHeader = Node.create({
  name: "tableHeader",
  content: "block+",
  tableRole: "header_cell",
  isolating: true,
  addAttributes: tableCellAttributes,
  parseHTML() {
    return [{ tag: "th" }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["th", HTMLAttributes, 0];
  },
});

const TaskList = Node.create({
  name: "taskList",
  group: "block list",
  content: "taskItem+",
  addAttributes() {
    return {
      mode: {
        default: "ordinary",
        parseHTML: (element: HTMLElement) =>
          element.getAttribute("data-mode") === "answer_key" ? "answer_key" : "ordinary",
        renderHTML: (attributes: Record<string, unknown>) => ({
          "data-mode": attributes.mode === "answer_key" ? "answer_key" : "ordinary",
        }),
      },
    };
  },
  parseHTML() {
    return [{ tag: 'ul[data-type="taskList"]' }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["ul", mergeAttributes(HTMLAttributes, { "data-type": "taskList" }), 0];
  },
  addKeyboardShortcuts() {
    return {
      "Mod-Shift-9": () => this.editor.commands.toggleList(this.name, "taskItem"),
    };
  },
});

const TaskItem = Node.create({
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
      correct: {
        default: false,
        parseHTML: (element: HTMLElement) => element.getAttribute("data-correct") === "true",
        renderHTML: (attributes: Record<string, unknown>) => ({
          "data-correct": attributes.correct === true ? "true" : "false",
        }),
      },
      itemId: {
        default: null,
        parseHTML: (element: HTMLElement) => element.getAttribute("data-item-id"),
        renderHTML: (attributes: Record<string, unknown>) =>
          typeof attributes.itemId === "string" && attributes.itemId !== ""
            ? { "data-item-id": attributes.itemId }
            : {},
      },
    };
  },
  parseHTML() {
    return [{ tag: 'li[data-type="taskItem"]' }];
  },
  renderHTML({ node, HTMLAttributes }) {
    return [
      "li",
      mergeAttributes(HTMLAttributes, { "data-type": "taskItem" }),
      [
        "label",
        { contenteditable: "false" },
        [
          "input",
          {
            type: "checkbox",
            checked: node.attrs.checked === true ? "checked" : undefined,
            disabled: "disabled",
            tabindex: "-1",
          },
        ],
        ["span", { "aria-hidden": "true" }],
      ],
      ["div", 0],
    ];
  },
  addKeyboardShortcuts() {
    return {
      Enter: () => this.editor.commands.splitListItem(this.name),
      "Shift-Tab": () => this.editor.commands.liftListItem(this.name),
      Tab: () => this.editor.commands.sinkListItem(this.name),
    };
  },
  addNodeView() {
    return ({ editor, getPos, node }) => {
      let currentNode = node;
      const dom = document.createElement("li");
      dom.dataset.type = "taskItem";
      const label = document.createElement("label");
      label.contentEditable = "false";
      const checkbox = document.createElement("input");
      checkbox.type = "checkbox";
      checkbox.tabIndex = -1;
      const indicator = document.createElement("span");
      indicator.setAttribute("aria-hidden", "true");
      const contentDOM = document.createElement("div");
      label.append(checkbox, indicator);
      dom.append(label, contentDOM);

      const render = () => {
        const attrs = currentNode.attrs as Record<string, unknown>;
        checkbox.checked = attrs.checked === true;
        checkbox.disabled = !editor.isEditable;
        dom.dataset.checked = checkbox.checked ? "true" : "false";
        dom.dataset.correct = attrs.correct === true ? "true" : "false";
        const itemId = typeof attrs.itemId === "string" ? attrs.itemId : "";
        if (itemId) dom.dataset.itemId = itemId;
        else delete dom.dataset.itemId;
      };
      const onChange = () => {
        if (!editor.isEditable || typeof getPos !== "function") return;
        const position = getPos();
        if (typeof position !== "number") return;
        editor.view.dispatch(
          editor.view.state.tr.setNodeMarkup(position, undefined, {
            ...currentNode.attrs,
            checked: checkbox.checked,
          }),
        );
      };
      checkbox.addEventListener("change", onChange);
      render();
      return {
        dom,
        contentDOM,
        update(updatedNode) {
          if (updatedNode.type.name !== "taskItem") return false;
          currentNode = updatedNode;
          render();
          return true;
        },
        destroy() {
          checkbox.removeEventListener("change", onChange);
        },
      };
    };
  },
});

const StableTaskItemIds = Extension.create({
  name: "stableTaskItemIds",
  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey("stableTaskItemIds"),
        appendTransaction(transactions, _oldState, newState) {
          if (!transactions.some((transaction) => transaction.docChanged)) return null;
          const transaction = newState.tr;
          let changed = false;
          newState.doc.descendants((node, position) => {
            if (node.type.name !== "taskItem") return;
            const itemId = typeof node.attrs.itemId === "string" ? node.attrs.itemId : "";
            if (/^[0-9a-f]{32}$/u.test(itemId)) return;
            transaction.setNodeMarkup(position, undefined, {
              ...node.attrs,
              itemId: createStableTaskItemId(),
            });
            changed = true;
          });
          return changed ? transaction : null;
        },
      }),
    ];
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
    const label = typeof HTMLAttributes.label === "string" ? HTMLAttributes.label : "Field";
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
    const label = typeof HTMLAttributes.label === "string" ? HTMLAttributes.label : "Field";
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
      HTMLAttributes: { rel: "noopener noreferrer" },
    }),
    Underline,
    Subscript,
    Superscript,
    Highlight.configure({ multicolor: true }),
    TextStyleKit,
    TextAlign.configure({ types: ["heading", "paragraph"] }),
    StudyImage.configure({
      allowBase64: false,
      HTMLAttributes: { class: contentStyles.image },
    }),
    Table,
    TableRow,
    TableHeader,
    TableCell,
    TaskList,
    TaskItem,
    StableTaskItemIds,
    Indent,
  ];
  if (options.templatePlaceholders !== false) {
    extensions.push(TemplateFieldInline, TemplateFieldBlock);
  }
  return extensions;
}

function positiveInteger(value: string | null, fallback: number): number {
  const parsed = Number.parseInt(value ?? "", 10);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback;
}

function parseColumnWidths(value: string | null): number[] | null {
  if (!value) return null;
  const widths = value.split(",").map((entry) => Number.parseInt(entry, 10));
  return widths.length > 0 && widths.every((entry) => Number.isInteger(entry) && entry > 0)
    ? widths
    : null;
}

function createStableTaskItemId(): string {
  return globalThis.crypto.randomUUID().replaceAll("-", "");
}
