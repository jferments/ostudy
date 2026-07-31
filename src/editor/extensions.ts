import { Extension } from "@tiptap/core";
import Highlight from "@tiptap/extension-highlight";
import Image from "@tiptap/extension-image";
import Link from "@tiptap/extension-link";
import TextAlign from "@tiptap/extension-text-align";
import { TextStyleKit } from "@tiptap/extension-text-style";
import Underline from "@tiptap/extension-underline";
import StarterKit from "@tiptap/starter-kit";

import { normalizeImageAlignment, normalizeImageSize } from "./imageAttributes";

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
      size: {
        default: "medium",
        parseHTML: (element: HTMLElement) =>
          normalizeImageSize(element.getAttribute("data-image-size")),
        renderHTML: (attributes: Record<string, unknown>) => ({
          "data-image-size": normalizeImageSize(attributes.size),
        }),
      },
      alignment: {
        default: "center",
        parseHTML: (element: HTMLElement) =>
          normalizeImageAlignment(element.getAttribute("data-image-alignment")),
        renderHTML: (attributes: Record<string, unknown>) => ({
          "data-image-alignment": normalizeImageAlignment(attributes.alignment),
        }),
      },
    };
  },
});

export function createEditorExtensions() {
  return [
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
      HTMLAttributes: { class: "study-card-image" },
    }),
    Indent,
  ];
}
