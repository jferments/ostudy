import { Extension, Node } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { renderFormula } from "../../math/render";

const atomNames = new Set(["inlineMath", "blockMath", "audio", "video"]);
const assetIdPattern = /^[0-9a-f]{32}$/u;

function formulaText(value: unknown, inline: boolean): string {
  const source = typeof value === "string" ? value : "";
  return inline ? `\\(${source}\\)` : `\\[${source}\\]`;
}

function mathNode(name: "inlineMath" | "blockMath") {
  const inline = name === "inlineMath";
  const tag = inline ? "span" : "div";
  return Node.create({
    name,
    group: inline ? "inline" : "block",
    inline,
    atom: true,
    selectable: true,
    marks: "",
    addAttributes() {
      return {
        latex: {
          default: "",
          rendered: false,
          parseHTML: (element: HTMLElement) => element.getAttribute("data-latex") ?? "",
        },
      };
    },
    parseHTML() {
      return [{ tag: `${tag}[data-study-math="${name}"][data-latex]` }];
    },
    renderHTML({ node }) {
      const latex = typeof node.attrs.latex === "string" ? node.attrs.latex : "";
      // Portable clipboard HTML remains escaped source, not renderer markup.
      // The NodeView alone owns on-screen typesetting.
      return [tag, {
        "data-study-math": name,
        "data-latex": latex,
        "aria-label": "Formula source",
        contenteditable: "false",
        style: "white-space:pre-wrap;overflow-wrap:anywhere",
      }, formulaText(latex, inline)];
    },
    addNodeView() {
      return ({ node }) => {
        const dom = document.createElement(tag);
        dom.dataset.studyMath = name;
        dom.contentEditable = "false";
        dom.style.cssText = "max-width:100%;overflow-x:auto;white-space:pre-wrap;overflow-wrap:anywhere";
        const paint = (latex: unknown) => renderFormula(dom, typeof latex === "string" ? latex : "", !inline);
        paint(node.attrs.latex);
        return {
          dom,
          update(next) {
            if (next.type.name !== name) return false;
            paint(next.attrs.latex);
            return true;
          },
          // Selection still belongs to ProseMirror; typesetter DOM never does.
          ignoreMutation: (mutation) => mutation.type !== "selection",
        };
      };
    },
    renderText({ node }) {
      return formulaText(node.attrs.latex, inline);
    },
  });
}

function mediaNode(name: "audio" | "video") {
  return Node.create({
    name,
    group: "block",
    atom: true,
    selectable: true,
    marks: "",
    addAttributes() {
      return {
        assetId: {
          default: null,
          rendered: false,
          parseHTML: (element: HTMLElement) => element.getAttribute("data-study-asset"),
        },
        src: {
          default: "",
          rendered: false,
          // Clipboard markup may carry an identity, never a fetchable donor URL.
          parseHTML: (element: HTMLElement) => {
            const id = element.getAttribute("data-study-asset") ?? "";
            return assetIdPattern.test(id) ? `study-asset://${id}` : "";
          },
        },
        title: { default: null, rendered: false, parseHTML: () => null },
        ...(name === "video" ? {
          width: { default: null, rendered: false, parseHTML: () => null },
          height: { default: null, rendered: false, parseHTML: () => null },
        } : {}),
      };
    },
    parseHTML() {
      return [{
        tag: `div[data-study-media="${name}"][data-study-asset]`,
        getAttrs: (element: HTMLElement) =>
          assetIdPattern.test(element.getAttribute("data-study-asset") ?? "") ? null : false,
      }];
    },
    renderHTML({ node }) {
      const id = typeof node.attrs.assetId === "string" ? node.attrs.assetId : "";
      const label = name === "audio" ? "Audio" : "Video";
      // Do not mount players until the activation/lifecycle boundary is ready.
      // Neither hydration URLs nor potentially answer-revealing titles may leak
      // through generic HTML serialization, accessible labels, or text snapshots.
      return ["div", {
        "data-study-media": name,
        ...(assetIdPattern.test(id) ? { "data-study-asset": id } : {}),
        contenteditable: "false",
      }, `[${label}: playback unavailable]`];
    },
    renderText() {
      return name === "audio" ? "[Audio]" : "[Video]";
    },
  });
}

const RichAtomMarks = Extension.create({
  name: "studyRichAtomMarks",
  addProseMirrorPlugins() {
    return [new Plugin({
      key: new PluginKey("studyRichAtomMarks"),
      appendTransaction(transactions, _previous, state) {
        if (!transactions.some((transaction) => transaction.docChanged)) return null;
        const transaction = state.tr;
        state.doc.descendants((node, position) => {
          // NodeSpec.marks controls a node's children, not marks on the node
          // itself. Formatting across inline atoms must not create JSON that
          // native schema-3 admission rejects. Preserve surrounding text marks.
          if (atomNames.has(node.type.name) && node.marks.length > 0) {
            transaction.setNodeMarkup(position, undefined, node.attrs, []);
          }
        });
        return transaction.steps.length > 0 ? transaction : null;
      },
    })];
  },
});

export const richAtomExtensions = [
  mathNode("inlineMath"),
  mathNode("blockMath"),
  mediaNode("audio"),
  mediaNode("video"),
  RichAtomMarks,
];
