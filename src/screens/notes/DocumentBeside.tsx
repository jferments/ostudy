import { useEffect, useState, type MutableRefObject } from "react";

import type { ShowToast } from "../../components/Toast";
import type { SourcePassage } from "../../components/sourcePassage";
import { getErrorMessage } from "../../ipc/common";
import { listDocuments, type DocumentSummary } from "../../ipc/documents";
import { Icon } from "../../ui/icons/Icon";
import { IconButton } from "../../ui/icons/IconButton";
import { Dialog } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { FormField } from "../../ui/primitives/FormField";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { AnnotatorBody, SaveStatus, useAnnotatorLeave } from "../documents/annotator/DocumentAnnotator";
import { useDocumentAnnotator } from "../documents/annotator/useDocumentAnnotator";

/**
 * A document open beside the notebook on a wide screen: its pages with every
 * annotation tool, saved as the learner works. `flushRef` lets the notebook
 * save the document's marks before it closes.
 */
export function DocumentBeside({ passage, showToast, flushRef, onClose }: {
  passage: SourcePassage;
  showToast: ShowToast;
  flushRef: MutableRefObject<(() => Promise<boolean>) | null>;
  onClose: () => void;
}) {
  const controller = useDocumentAnnotator(passage.documentId, showToast);
  const { leave, closing } = useAnnotatorLeave(controller);
  const { flush } = controller;
  useEffect(() => {
    flushRef.current = flush;
    return () => {
      if (flushRef.current === flush) flushRef.current = null;
    };
  }, [flush, flushRef]);
  return (
    <section aria-label={`${passage.title}, beside your notes`}
      className="tw:flex tw:min-w-0 tw:flex-1 tw:basis-0 tw:flex-col tw:border-0 tw:border-e tw:border-solid tw:border-border">
      <div className="tw:flex tw:min-w-0 tw:items-center tw:gap-2 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-background tw:px-3 tw:py-1.5">
        <Icon name="document" className="tw:shrink-0 tw:text-muted-foreground" />
        <h2 className="tw:m-0 tw:min-w-0 tw:flex-1 tw:truncate tw:text-label tw:font-medium">{passage.title}</h2>
        <SaveStatus state={controller.saveState} onRetry={controller.retrySave} />
        <IconButton label="Close document" icon="close" disabled={closing} onClick={() => void leave(onClose)} />
      </div>
      <div className="tw:flex tw:min-h-0 tw:min-w-0 tw:flex-1 tw:flex-col">
        <AnnotatorBody
          documentId={passage.documentId}
          title={passage.title}
          controller={controller}
          showToast={showToast}
          focusPage={passage.pageNumber}
        />
      </div>
    </section>
  );
}

/** Chooses a document with pages to open beside the notebook. */
export function ChooseDocumentDialog({ onClose, onChoose }: {
  onClose: () => void;
  onChoose: (document: DocumentSummary) => void;
}) {
  const [documents, setDocuments] = useState<DocumentSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [chosen, setChosen] = useState("");
  useEffect(() => {
    let current = true;
    listDocuments().then((all) => {
      if (current) setDocuments(all.filter((document) => document.sourceUnitKind === "page" && document.sourceUnitCount > 0));
    }, (failure: unknown) => {
      if (current) setError(getErrorMessage(failure));
    });
    return () => { current = false; };
  }, []);
  const document = documents?.find((item) => item.id === chosen) ?? null;
  return (
    <Dialog
      title="Open a document beside your notes"
      description="Read and mark a PDF while you write. Its marks are saved with the document."
      width="small"
      onClose={onClose}
      onSubmit={(event) => {
        event.preventDefault();
        if (document) onChoose(document);
      }}
      actions={<>
        <Button variant="secondary" onClick={onClose}>Cancel</Button>
        <Button variant="primary" type="submit" disabled={!document}>Open</Button>
      </>}
    >
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      {documents && documents.length === 0 ? (
        <InlineNotice kind="info">Import a PDF in Library's Documents page to read it here.</InlineNotice>
      ) : (
        <FormField label="Document">
          <NativeSelect value={chosen} disabled={!documents} onChange={(event) => setChosen(event.target.value)}>
            <option value="">{documents ? "Choose…" : "Loading…"}</option>
            {documents?.map((item) => <option key={item.id} value={item.id}>{item.title}</option>)}
          </NativeSelect>
        </FormField>
      )}
    </Dialog>
  );
}
