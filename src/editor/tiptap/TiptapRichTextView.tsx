import { useEffect, useMemo, type MouseEvent } from "react";
import { EditorContent, useEditor } from "@tiptap/react";

import { getErrorMessage } from "../../ipc/common";
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
  className = "",
  onLinkError,
}: RichTextViewProps) {
  const extensions = useMemo(() => createEditorExtensions(), []);
  const presented = useMemo(
    () => prepareDocumentForPresentation(document, answerKeySide),
    [answerKeySide, document],
  );
  const hydrated = useMemo(
    () => hydrateDocument(presented, assets),
    [assets, presented],
  );
  const editor = useEditor({
    extensions,
    content: hydrated,
    editable: false,
    editorProps: {
      attributes: {
        class: contentStyles.content,
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
