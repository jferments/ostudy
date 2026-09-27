import { useEffect, useMemo, type MouseEvent } from "react";
import { EditorContent, useEditor } from "@tiptap/react";

import { getErrorMessage } from "../../ipc/common";
import type { RichDocument } from "../../ipc/common";
import { openExternalUrl } from "../../platform/externalLinks";
import contentStyles from "../RichContent.module.css";
import type { RichTextViewProps } from "../RichTextView";
import { createEditorExtensions } from "../extensions";
import { isAllowedExternalLink } from "../links";
import { hydrateDocument, prepareDocumentForPresentation } from "../richText";

export function TiptapRichTextView({
  document,
  assets,
  answerKeySide = null,
  mediaPlaybackActive = true,
  className = "",
  onLinkError,
}: RichTextViewProps) {
  const extensions = useMemo(() => createEditorExtensions(), []);
  const presented = useMemo(
    () => prepareDocumentForPresentation(document, answerKeySide),
    [answerKeySide, document],
  );
  const hydrated = useMemo(
    () => gateMediaPlayback(hydrateDocument(presented, assets), mediaPlaybackActive),
    [assets, mediaPlaybackActive, presented],
  );
  const editor = useEditor({
    extensions,
    content: hydrated,
    editable: false,
    editorProps: {
      attributes: {
        class: contentStyles.content ?? "",
        tabindex: "0",
      },
    },
  });

  useEffect(() => {
    if (editor) editor.commands.setContent(hydrated, { emitUpdate: false });
  }, [editor, hydrated]);

  function handleClick(event: MouseEvent<HTMLDivElement>): void {
    const target = event.target;
    if (!(target instanceof Element)) return;

    const anchor = target.closest("a");
    if (!anchor || !event.currentTarget.contains(anchor)) return;

    const href = anchor.getAttribute("href") ?? "";
    if (!isAllowedExternalLink(href)) return;

    event.preventDefault();
    void openExternalUrl(href).catch((error: unknown) => {
      const message = getErrorMessage(error);
      if (onLinkError) onLinkError(message);
      else console.error("Unable to open card link:", message);
    });
  }

  return (
    <div className={`${contentStyles.view} ${className}`.trim()} onClick={handleClick}>
      <EditorContent editor={editor} />
    </div>
  );
}

function gateMediaPlayback(document: RichDocument, active: boolean): RichDocument {
  if (active) return document;
  const copy = structuredClone(document);
  const stack: unknown[] = [copy];
  while (stack.length > 0) {
    const value = stack.pop();
    if (Array.isArray(value)) {
      stack.push(...value);
      continue;
    }
    if (typeof value !== "object" || value === null) continue;
    const record = value as Record<string, unknown>;
    if ((record.type === "audio" || record.type === "video")
      && typeof record.attrs === "object" && record.attrs !== null && !Array.isArray(record.attrs)) {
      (record.attrs as Record<string, unknown>).src = "";
    }
    stack.push(...Object.values(record));
  }
  return copy;
}
