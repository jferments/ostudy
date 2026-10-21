import { useEffect, useMemo, useState } from "react";

import { getErrorMessage } from "../../ipc/common";
import { listDecks, type DeckSummary } from "../../ipc/decks";
import type { LinkTarget, NoteLink } from "../../ipc/documentAnnotations";
import { listDocuments, type DocumentSummary } from "../../ipc/documents";
import { getNotebook, listNotebooks, type NotebookSummary } from "../../ipc/notebooks";
import { listStudyGuidePage, type StudyGuidePageItem } from "../../ipc/studyGuides";
import { Dialog } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { FormField } from "../../ui/primitives/FormField";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { SegmentedControl } from "../../ui/primitives/SegmentedControl";
import { TextField } from "../../ui/primitives/TextField";
import { LabeledChoice } from "./LabeledChoice";

type LinkKind = LinkTarget["kind"];

const KIND_OPTIONS = [
  { value: "documentPage", label: "Document page" },
  { value: "studyGuide", label: "Study guide" },
  { value: "deck", label: "Flashcards" },
  { value: "notebookPage", label: "Notes page" },
] as const;

/** The longest label a link may have; the native side checks it too. */
export const MAX_LINK_LABEL_CHARS = 200;

interface Choices {
  documents: DocumentSummary[];
  guides: StudyGuidePageItem[];
  decks: DeckSummary[];
  notebooks: NotebookSummary[];
}

export interface LinkChoice {
  link: NoteLink;
  label: string;
}

interface LinkDialogProps {
  /** The link being changed; none adds a new one. */
  initial: LinkChoice | null;
  /** The notebook the link sits in, and its pages, for links within it. */
  notebookId: string;
  onClose: () => void;
  onSave: (choice: LinkChoice) => void;
}

/**
 * "Add a link": a document page, a document's study guide, a deck, or a page
 * of any notebook, shown as a small link or as a button. A deck's button
 * starts Practice on its cards.
 */
export function LinkDialog({ initial, notebookId, onClose, onSave }: LinkDialogProps) {
  const [choices, setChoices] = useState<Choices | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const start = initial?.link.target;
  const [kind, setKind] = useState<LinkKind>(start?.kind ?? "documentPage");
  const [documentId, setDocumentId] = useState(start && "documentId" in start ? start.documentId : "");
  const [pageNumber, setPageNumber] = useState(String(start?.kind === "documentPage" ? start.pageNumber : 1));
  const [deckId, setDeckId] = useState(start?.kind === "deck" ? start.deckId : "");
  const [targetNotebookId, setTargetNotebookId] = useState(start?.kind === "notebookPage" ? start.notebookId : notebookId);
  const [pageId, setPageId] = useState(start?.kind === "notebookPage" ? start.pageId : "");
  const [label, setLabel] = useState(initial?.label ?? "");
  const [labelEdited, setLabelEdited] = useState(initial !== null);
  const [button, setButton] = useState(initial ? initial.link.button === true : false);
  const [buttonChosen, setButtonChosen] = useState(initial !== null);

  useEffect(() => {
    let current = true;
    Promise.all([
      listDocuments(),
      listStudyGuidePage({ limit: 100 }),
      listDecks(),
      listNotebooks(),
    ]).then(([documents, guides, decks, notebooks]) => {
      if (!current) return;
      setChoices({
        documents: documents.filter((document) => document.sourceUnitKind === "page" && document.sourceUnitCount > 0),
        guides: guides.items.filter((guide) => guide.targetKind === "document"),
        decks: decks.filter((deck) => deck.archivedAt === null),
        notebooks,
      });
    }, (error: unknown) => {
      if (current) setLoadError(getErrorMessage(error));
    });
    return () => { current = false; };
  }, []);

  const document = choices?.documents.find((item) => item.id === documentId) ?? null;
  const guide = choices?.guides.find((item) => item.targetId === documentId) ?? null;
  const deck = choices?.decks.find((item) => item.id === deckId) ?? null;
  const notebook = choices?.notebooks.find((item) => item.notebook.id === targetNotebookId) ?? null;
  const [notebookPages, setNotebookPages] = useState<readonly string[]>([]);
  useEffect(() => {
    if (!notebook) return;
    let current = true;
    getNotebook(notebook.notebook.id).then((contents) => {
      if (!current) return;
      const ids = contents.pages.map((page) => page.id);
      setNotebookPages(ids);
      setPageId((chosen) => (ids.includes(chosen) ? chosen : ids[0] ?? ""));
    }, () => undefined);
    return () => { current = false; };
  }, [notebook]);

  const page = Number.parseInt(pageNumber, 10);
  const target = useMemo((): LinkTarget | null => {
    switch (kind) {
      case "documentPage":
        return document && page >= 1 && page <= document.sourceUnitCount
          ? { kind, documentId: document.id, pageNumber: page }
          : null;
      case "studyGuide":
        return guide ? { kind, documentId: guide.targetId } : null;
      case "deck":
        return deck ? { kind, deckId: deck.id } : null;
      case "notebookPage":
        return notebook && pageId ? { kind, notebookId: notebook.notebook.id, pageId } : null;
    }
  }, [deck, document, guide, kind, notebook, page, pageId]);

  // A label follows the chosen target until the learner types one.
  const suggested = kind === "documentPage" && document ? `${document.title} · p. ${Number.isFinite(page) ? page : 1}`
    : kind === "studyGuide" && guide ? `${guide.title} study guide`
      : kind === "deck" && deck ? `Study ${deck.name}`
        : kind === "notebookPage" && notebook ? `${notebook.notebook.title} · p. ${notebookPages.indexOf(pageId) + 1}`
          : "";
  const shownLabel = labelEdited ? label : suggested;
  // A deck reads best as a button that starts studying; the rest as links.
  const shownButton = buttonChosen ? button : kind === "deck";
  const trimmed = shownLabel.trim();
  const canSave = target !== null && trimmed.length > 0 && trimmed.length <= MAX_LINK_LABEL_CHARS;

  function save() {
    if (!target || !canSave) return;
    onSave({ link: shownButton ? { target, button: true } : { target }, label: trimmed });
  }

  return (
    <Dialog
      title={initial ? "Edit link" : "Add a link"}
      description="Link to a page of a document, a study guide, flashcards or another page of notes."
      width="medium"
      onClose={onClose}
      onSubmit={(event) => {
        event.preventDefault();
        save();
      }}
      actions={<>
        <Button variant="secondary" onClick={onClose}>Cancel</Button>
        <Button variant="primary" type="submit" disabled={!canSave}>{initial ? "Save link" : "Add link"}</Button>
      </>}
    >
      {loadError ? <InlineNotice kind="error">{loadError}</InlineNotice> : null}
      <FormField label="Link to">
        <NativeSelect value={kind} onChange={(event) => setKind(event.target.value as LinkKind)}>
          {KIND_OPTIONS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
        </NativeSelect>
      </FormField>
      {kind === "documentPage" || kind === "studyGuide" ? (
        <FormField label={kind === "documentPage" ? "Document" : "Study guide"}>
          <NativeSelect value={documentId} disabled={!choices} onChange={(event) => setDocumentId(event.target.value)}>
            <option value="">{choices ? "Choose…" : "Loading…"}</option>
            {kind === "documentPage"
              ? choices?.documents.map((item) => <option key={item.id} value={item.id}>{item.title}</option>)
              : choices?.guides.map((item) => <option key={item.guideId} value={item.targetId}>{item.title}</option>)}
          </NativeSelect>
        </FormField>
      ) : null}
      {kind === "documentPage" ? (
        <FormField label="Page" description={document ? `1 to ${document.sourceUnitCount}` : undefined}>
          <TextField type="number" inputMode="numeric" min={1} max={document?.sourceUnitCount} value={pageNumber}
            onChange={(event) => setPageNumber(event.target.value)} />
        </FormField>
      ) : null}
      {kind === "deck" ? (
        <FormField label="Deck">
          <NativeSelect value={deckId} disabled={!choices} onChange={(event) => setDeckId(event.target.value)}>
            <option value="">{choices ? "Choose…" : "Loading…"}</option>
            {choices?.decks.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
          </NativeSelect>
        </FormField>
      ) : null}
      {kind === "notebookPage" ? (
        <>
          <FormField label="Notebook">
            <NativeSelect value={targetNotebookId} disabled={!choices} onChange={(event) => setTargetNotebookId(event.target.value)}>
              {choices?.notebooks.map((item) => <option key={item.notebook.id} value={item.notebook.id}>{item.notebook.title}</option>)}
            </NativeSelect>
          </FormField>
          <FormField label="Page">
            <NativeSelect value={pageId} disabled={notebookPages.length === 0} onChange={(event) => setPageId(event.target.value)}>
              {notebookPages.map((id, index) => <option key={id} value={id}>{`Page ${index + 1}`}</option>)}
            </NativeSelect>
          </FormField>
        </>
      ) : null}
      <FormField label="Label">
        <TextField value={shownLabel} maxLength={MAX_LINK_LABEL_CHARS}
          onChange={(event) => {
            setLabelEdited(true);
            setLabel(event.target.value);
          }} />
      </FormField>
      <LabeledChoice label="Show as">
        <SegmentedControl
          label="Show as"
          value={shownButton ? "button" : "link"}
          options={[{ value: "link", label: "Link" }, { value: "button", label: "Button" }]}
          onChange={(value) => {
            setButtonChosen(true);
            setButton(value === "button");
          }}
        />
      </LabeledChoice>
    </Dialog>
  );
}
