import { Extension, Node } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { renderFormula } from "../../math/render";
import { managedAssetUrl } from "../../platform/assets";

const atomNames = new Set(["inlineMath", "blockMath", "audio", "video"]);
const assetIdPattern = /^[0-9a-f]{32}$/u;
const managedPlayers = new Set<HTMLMediaElement>();

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
          ignoreMutation: (mutation) => mutation.type !== "selection",
        };
      };
    },
    renderText({ node }) {
      return formulaText(node.attrs.latex, inline);
    },
  });
}

function releasePlayer(player: HTMLMediaElement | null): void {
  if (!player) return;
  managedPlayers.delete(player);
  try { player.pause(); } catch { /* jsdom and old WebViews may not implement pause(). */ }
  player.removeAttribute("src");
  try { player.load(); } catch { /* Decoder release is best-effort on teardown. */ }
}

function pauseOtherPlayers(current: HTMLMediaElement): void {
  for (const player of managedPlayers) {
    if (player === current || player.paused) continue;
    try { player.pause(); } catch { /* A failed pause must not affect the active player. */ }
  }
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
      return ["div", {
        "data-study-media": name,
        ...(assetIdPattern.test(id) ? { "data-study-asset": id } : {}),
        contenteditable: "false",
      }, `[${label}]`];
    },
    addNodeView() {
      return ({ node }) => {
        const dom = document.createElement("div");
        dom.dataset.studyMedia = name;
        dom.contentEditable = "false";
        dom.style.cssText = "display:block;max-width:100%;margin:0.75em 0";
        let player: HTMLMediaElement | null = null;
        let signature = "";

        const paint = (attrs: Record<string, unknown>) => {
          const id = typeof attrs.assetId === "string" ? attrs.assetId : "";
          const src = typeof attrs.src === "string" ? attrs.src : "";
          const width = name === "video" && typeof attrs.width === "number" ? attrs.width : null;
          const height = name === "video" && typeof attrs.height === "number" ? attrs.height : null;
          const expected = assetIdPattern.test(id) ? managedAssetUrl(id) : "";
          const playable = expected !== "" && src === expected;
          const nextSignature = `${id}\0${src}\0${width ?? ""}\0${height ?? ""}`;
          if (signature === nextSignature) return;
          signature = nextSignature;
          releasePlayer(player);
          player = null;
          dom.replaceChildren();
          if (assetIdPattern.test(id)) dom.dataset.studyAsset = id;
          else delete dom.dataset.studyAsset;

          if (!playable) {
            const placeholder = document.createElement("span");
            placeholder.dataset.studyMediaPlaceholder = name;
            placeholder.textContent = name === "audio" ? "[Audio]" : "[Video]";
            placeholder.setAttribute("aria-label", `${name === "audio" ? "Audio" : "Video"} attachment`);
            dom.appendChild(placeholder);
            return;
          }

          player = document.createElement(name);
          player.dataset.studyManagedMedia = name;
          player.controls = true;
          player.autoplay = false;
          player.preload = "metadata";
          player.src = src;
          player.style.maxWidth = "100%";
          if (name === "audio") player.style.width = "min(100%, 36rem)";
          player.setAttribute("aria-label", name === "audio" ? "Audio player" : "Video player");
          player.setAttribute("controlslist", "nodownload noremoteplayback");
          if (name === "video") {
            const video = player as HTMLVideoElement;
            video.playsInline = true;
            video.disablePictureInPicture = true;
            if (width !== null && Number.isFinite(width)) video.width = Math.trunc(width);
            if (height !== null && Number.isFinite(height)) video.height = Math.trunc(height);
          }
          player.addEventListener("play", () => pauseOtherPlayers(player!));
          managedPlayers.add(player);
          dom.appendChild(player);
        };

        paint(node.attrs as Record<string, unknown>);
        return {
          dom,
          update(next) {
            if (next.type.name !== name) return false;
            paint(next.attrs as Record<string, unknown>);
            return true;
          },
          stopEvent: (event) => event.target instanceof globalThis.Node && dom.contains(event.target),
          ignoreMutation: (mutation) => mutation.type !== "selection",
          destroy() {
            releasePlayer(player);
            player = null;
          },
        };
      };
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
