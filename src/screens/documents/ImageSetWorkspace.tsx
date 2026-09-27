import { useEffect, useRef, useState } from "react";
import { useDocumentJobs } from "../../controllers/DocumentJobsProvider";
import { createOperationId, getErrorMessage } from "../../ipc/common";
import { getImageSet, getImageSetPreview, type ImageSetDetails, type ImageSetPreview } from "../../ipc/imageSets";
import { dataTransferImageFiles, IMAGE_FILE_ACCEPT } from "../../platform/imageInput";
import { ImagePasteSession } from "../../platform/imagePaste";
import { InlineWorkspace } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { TextField } from "../../ui/primitives/TextField";
import { InlineNotice } from "../../ui/primitives/InlineNotice";

interface ImageSetWorkspaceProps {
  documentId: string | null;
  onClose: () => void;
}

const phaseLabels = {
  idle: "Add images to start.",
  uploading: "Adding images…",
  cards: "Creating flashcards…",
  guide: "Creating study guide…",
  ready: "Study materials are ready in Flashcards and Study Guides.",
  failed: "Processing needs attention.",
  stopped: "Processing stopped. Saved images and study materials are retained.",
};

/** Acquisition only. Source relevance, reading relationships, and generation
 * belong to the native document pipeline, not to a preparation wizard. */
export function ImageSetWorkspace({ documentId, onClose }: ImageSetWorkspaceProps) {
  const documentJobs = useDocumentJobs();
  const collections = documentJobs?.imageCollections;
  const [draftKey] = useState(createOperationId);
  const key = documentId
    ? Object.keys(collections?.jobs ?? {}).find((id) => collections?.jobs[id]?.documentId === documentId) ?? documentId
    : draftKey;
  const job = collections?.jobs[key];
  const savedId = job?.documentId ?? documentId;
  const [details, setDetails] = useState<ImageSetDetails | null>(null);
  const [previewId, setPreviewId] = useState<string | null>(null);
  const [preview, setPreview] = useState<ImageSetPreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const picker = useRef<HTMLInputElement>(null);
  const savedCount = job?.items.filter((item) => item.status === "saved").length ?? 0;
  const phase = job?.phase;
  const addImages = collections?.addImages;

  useEffect(() => {
    let current = true;
    if (!savedId) { setDetails(null); return; }
    void getImageSet(savedId).then((value) => {
      if (current) { setDetails(value); setError(null); }
    }).catch((reason: unknown) => {
      if (current) setError(`Images are saved; refreshing their list failed: ${getErrorMessage(reason)}`);
    });
    return () => { current = false; };
  }, [savedId, savedCount, phase]);

  const pasteSession = useRef<ImagePasteSession | null>(null);
  useEffect(() => {
    if (!addImages) return;
    const session = pasteSession.current ?? new ImagePasteSession();
    pasteSession.current = session;
    return session.bind((files) => addImages(key, savedId, files));
  }, [addImages, key, savedId]);

  useEffect(() => {
    let current = true;
    setPreview(null);
    setPreviewError(null);
    if (!savedId || !details || details.documentId !== savedId || !previewId) {
      setPreviewLoading(false);
      return () => { current = false; };
    }
    setPreviewLoading(true);
    void getImageSetPreview({ documentId: savedId, memberId: previewId,
      expectedRevision: details.revision, viewport: null }).then((value) => {
      if (current) setPreview(value);
    }).catch((reason: unknown) => {
      if (current) setPreviewError(getErrorMessage(reason));
    }).finally(() => {
      if (current) setPreviewLoading(false);
    });
    return () => { current = false; };
  }, [savedId, details, previewId]);

  const pending = job?.items.filter((item) => !details || item.status !== "saved") ?? [];
  const total = (details?.members.length ?? 0) + pending.length;
  const retryable = phase === "failed" || phase === "stopped";
  const nativeDocument = documentJobs?.documents.find((item) => item.id === savedId);
  const working = phase === "cards" || phase === "guide" || phase === "uploading";

  return (
    <InlineWorkspace title={details?.title ?? "Upload images"}
      description="Add screenshots or photos. Flashcards and a study guide are created automatically."
      onClose={onClose} closeLabel="Back to Documents">
      <div className="tw:grid tw:min-w-0 tw:gap-4 tw:p-4 tw:sm:p-5"
        onDragOverCapture={(event) => {
          event.preventDefault();
          event.dataTransfer.dropEffect = "copy";
        }}
        onDropCapture={(event) => {
          const files = dataTransferImageFiles(event.dataTransfer);
          if (files.length === 0) return;
          event.preventDefault();
          event.stopPropagation();
          addImages?.(key, savedId, files);
        }}>
        <TextField ref={picker} type="file" multiple accept={IMAGE_FILE_ACCEPT}
          aria-label="Choose images" className="tw:sr-only"
          onChange={(event) => {
            const files = Array.from(event.currentTarget.files ?? []);
            event.currentTarget.value = "";
            addImages?.(key, savedId, files);
          }} />
        <div className="tw:flex tw:items-center tw:justify-between tw:gap-3">
          <span className="tw:text-sm tw:text-muted-foreground">{total} image{total === 1 ? "" : "s"}</span>
          <Button variant="primary" size={total > 0 ? "icon" : "medium"}
            aria-label={total > 0 ? "Add more images" : "Select images"}
            disabled={!collections} onClick={() => picker.current?.click()}>
            {total > 0 ? <span aria-hidden="true">+</span> : "Select images"}
          </Button>
        </div>
        {total > 0 ? <ul aria-label="Images in collection" className="tw:m-0 tw:grid tw:list-none tw:gap-1 tw:p-0">
          {details?.members.map((member) => {
            const selected = previewId === member.id;
            return <li key={member.id}
              className="tw:grid tw:min-w-0 tw:gap-2 tw:border-x-0 tw:border-t-0 tw:border-b tw:border-solid tw:border-border tw:py-2">
              <div className="tw:flex tw:min-w-0 tw:items-center tw:justify-between tw:gap-3">
                <span className="tw:min-w-0 tw:break-all tw:text-sm">{member.originalFilename}</span>
                <Button size="small" variant="ghost"
                  aria-label={`${selected ? "Hide" : "View"} ${member.originalFilename}`}
                  onClick={() => setPreviewId((current) => current === member.id ? null : member.id)}>
                  {selected ? "Hide" : "View"}
                </Button>
              </div>
              {selected && previewLoading ? <p role="status"
                className="tw:m-0 tw:text-sm tw:text-muted-foreground">Loading retained original…</p> : null}
              {selected && previewError ? <InlineNotice kind="error">{previewError}</InlineNotice> : null}
              {selected && preview ? <img src={preview.dataUrl} alt="Retained original image"
                className="tw:mx-auto tw:max-h-[70vh] tw:max-w-full tw:object-contain" /> : null}
            </li>;
          })}
          {pending.map((item) => <li key={item.id} className="tw:grid tw:min-w-0 tw:gap-1 tw:border-x-0 tw:border-t-0 tw:border-b tw:border-solid tw:border-border tw:py-2">
            <div className="tw:flex tw:min-w-0 tw:items-center tw:justify-between tw:gap-3">
              <span className="tw:min-w-0 tw:break-all tw:text-sm">{item.name}</span>
              {item.status === "failed" ? <Button size="small" variant="ghost"
                aria-label={`Remove ${item.name}`} onClick={() => collections?.removeFailedImage(key, item.id)}>Remove</Button>
                : <span className="tw:text-xs tw:text-muted-foreground">{item.status === "saved" ? "Saved" : "Adding…"}</span>}
            </div>
            {item.error ? <span className="tw:text-sm tw:text-destructive">{item.error}</span> : null}
          </li>)}
        </ul> : null}
        <Button variant="ghost" aria-label="Drop images here or select files"
          className="tw:h-auto tw:w-full tw:min-h-32 tw:gap-1 tw:rounded-lg tw:border tw:border-dashed tw:border-border tw:bg-transparent tw:p-5 tw:text-center tw:text-sm tw:text-muted-foreground tw:focus-visible:outline tw:focus-visible:outline-ring"
          onClick={() => picker.current?.click()}>
          <span className="tw:grid tw:gap-1"><span>Drop images here</span>
            <span>or paste images from your clipboard</span></span>
        </Button>
        {job ? <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-3">
          <p role="status" className="tw:m-0 tw:text-sm tw:text-muted-foreground">{phaseLabels[job.phase]}</p>
          {retryable ? <Button size="small" onClick={() => collections?.retryImages(key)}>Retry processing</Button> : null}
          {working && nativeDocument ? <Button size="small" variant="ghost" onClick={() => {
            if (phase === "guide") void documentJobs?.stopStudyGuide(nativeDocument);
            else void documentJobs?.stopGeneration(nativeDocument);
          }}>Stop</Button> : null}
        </div> : null}
        {job?.error ? <InlineNotice kind="error">{savedId ? "Your images are saved. " : ""}{job.error}</InlineNotice> : null}
        {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      </div>
    </InlineWorkspace>
  );
}
