import { useEffect, useState } from "react";

import { getErrorMessage } from "../../../ipc/common";
import { createNotebook, listNotebooks, type DocumentClipRegion, type NotebookSummary } from "../../../ipc/notebooks";
import { Dialog } from "../../../ui/overlays/Dialog";
import { Button } from "../../../ui/primitives/Button";
import { LoadingState } from "../../../ui/primitives/EmptyState";
import { FormField } from "../../../ui/primitives/FormField";
import { InlineNotice } from "../../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../../ui/primitives/NativeSelect";
import { TextField } from "../../../ui/primitives/TextField";
import { clipToNotebook, type PlacedClip } from "../../notes/clipToNotebook";

/** The longest notebook name; the native side checks it too. */
const MAX_TITLE_CHARS = 200;
const NEW_NOTEBOOK = "new";

/** The notebook clipped into last, offered first next time. */
let lastNotebookId: string | null = null;

/**
 * Chooses the notebook a clip goes into, or names a new one filed with the
 * document, then clips the region into it.
 */
export function ClipToNotesDialog({ region, documentTitle, subjectId, onClose, onClipped }: {
  region: DocumentClipRegion;
  documentTitle: string;
  /** The document's subject, where a new notebook is filed and whose notebooks come first. */
  subjectId: string | null;
  onClose: () => void;
  onClipped: (placed: PlacedClip, notebookTitle: string) => void;
}) {
  const [notebooks, setNotebooks] = useState<NotebookSummary[] | null>(null);
  const [choice, setChoice] = useState<string>(NEW_NOTEBOOK);
  const [title, setTitle] = useState(() => `${documentTitle} notes`.slice(0, MAX_TITLE_CHARS));
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    listNotebooks().then((all) => {
      if (!current) return;
      const ordered = orderForClip(all, subjectId);
      setNotebooks(ordered);
      const remembered = ordered.find((item) => item.notebook.id === lastNotebookId);
      setChoice((remembered ?? ordered.find((item) => item.notebook.subjectId === subjectId))?.notebook.id ?? NEW_NOTEBOOK);
    }, (failure: unknown) => {
      if (!current) return;
      // Without the list a new notebook can still be made.
      setNotebooks([]);
      setError(getErrorMessage(failure));
    });
    return () => { current = false; };
  }, [subjectId]);

  const trimmed = title.trim();
  const creating = choice === NEW_NOTEBOOK;

  async function clip() {
    if (saving || (creating && !trimmed)) return;
    setSaving(true);
    setError(null);
    try {
      let notebookId = choice;
      let notebookTitle = notebooks?.find((item) => item.notebook.id === choice)?.notebook.title ?? "";
      if (creating) {
        const created = await createNotebook({ title: trimmed, subjectId, paper: "blank", pageSize: "letter" });
        notebookId = created.notebook.id;
        notebookTitle = created.notebook.title;
        // A second try goes into the notebook just made, not another new one.
        const summary: NotebookSummary = {
          notebook: created.notebook, pageCount: created.pages.length, linkCount: 0, linkLabels: [], folderId: null, pinnedAt: null, firstPage: created.pages[0] ?? null, preview: [],
        };
        setNotebooks((current) => [summary, ...(current ?? [])]);
        setChoice(notebookId);
      }
      const placed = await clipToNotebook(notebookId, region);
      lastNotebookId = notebookId;
      onClipped(placed, notebookTitle);
    } catch (failure) {
      setError(getErrorMessage(failure));
      setSaving(false);
    }
  }

  return (
    <Dialog
      title="Clip to notes"
      description={`Page ${region.pageNumber} of ${documentTitle}`}
      width="small"
      busy={saving}
      onClose={onClose}
      onSubmit={(event) => {
        event.preventDefault();
        void clip();
      }}
      actions={<>
        <Button variant="secondary" onClick={onClose}>Cancel</Button>
        <Button variant="primary" type="submit" loading={saving} disabled={notebooks === null || saving || (creating && !trimmed)}>
          Clip
        </Button>
      </>}
    >
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      {notebooks === null ? <LoadingState label="Loading notebooks…" /> : (
        <>
          <FormField label="Notebook">
            <NativeSelect value={choice} disabled={saving} onChange={(event) => setChoice(event.target.value)}>
              {notebooks.map((item) => (
                <option key={item.notebook.id} value={item.notebook.id}>{item.notebook.title}</option>
              ))}
              <option value={NEW_NOTEBOOK}>New notebook…</option>
            </NativeSelect>
          </FormField>
          {creating ? (
            <FormField label="Name" required>
              <TextField value={title} maxLength={MAX_TITLE_CHARS} autoFocus disabled={saving}
                onChange={(event) => setTitle(event.target.value)} />
            </FormField>
          ) : null}
        </>
      )}
    </Dialog>
  );
}

/** The document's subject's notebooks first, then the rest; each most recently changed first. */
export function orderForClip(notebooks: readonly NotebookSummary[], subjectId: string | null): NotebookSummary[] {
  const rank = (item: NotebookSummary) => (item.notebook.subjectId === subjectId ? 0 : 1);
  return [...notebooks].sort((a, b) => rank(a) - rank(b) || b.notebook.updatedAt - a.notebook.updatedAt);
}
