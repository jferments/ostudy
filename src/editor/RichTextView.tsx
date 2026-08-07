import { useEffect, useMemo } from "react";
import type { MouseEvent } from "react";
import { EditorContent, useEditor } from "@tiptap/react";
import { openUrl } from "@tauri-apps/plugin-opener";

import {
  type Asset,
} from "../ipc/assets";
import {
  getErrorMessage,
  type RichDocument,
} from "../ipc/common";
import { createEditorExtensions } from "./extensions";
import { isAllowedExternalLink } from "./links";
import { hydrateDocument } from "./richText";

interface RichTextViewProps {
  document: RichDocument;
  assets: Asset[];
  className?: string;
  onLinkError?: (message: string) => void;
}

export function RichTextView({
  document,
  assets,
  className = "",
  onLinkError,
}: RichTextViewProps) {
  const extensions = useMemo(() => createEditorExtensions(), []);
  const hydrated = useMemo(
    () => hydrateDocument(document, assets),
    [document, assets],
  );
  const editor = useEditor({
    extensions,
    content: hydrated,
    editable: false,
    editorProps: {
      attributes: {
        class: "rich-card-content",
        tabindex: "0",
      },
    },
  });

  useEffect(() => {
    if (editor) {
      editor.commands.setContent(hydrated, { emitUpdate: false });
    }
  }, [editor, hydrated]);

  function handleClick(event: MouseEvent<HTMLDivElement>): void {
    const target = event.target;
    if (!(target instanceof Element)) return;

    const anchor = target.closest("a");
    if (!anchor || !event.currentTarget.contains(anchor)) return;

    const href = anchor.getAttribute("href") ?? "";
    if (!isAllowedExternalLink(href)) return;

    event.preventDefault();
    void openUrl(href).catch((error: unknown) => {
      const message = getErrorMessage(error);
      if (onLinkError) {
        onLinkError(message);
      } else {
        console.error("Unable to open card link:", message);
      }
    });
  }

  return (
    <div className={`rich-card-view ${className}`.trim()} onClick={handleClick}>
      <EditorContent editor={editor} />
    </div>
  );
}
