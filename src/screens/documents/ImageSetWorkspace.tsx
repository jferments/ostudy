import type { ShowToast } from "../../components/Toast";
import type { ImageSetInclusionCrop } from "../../ipc/imageSets";
import { Badge } from "../../ui/primitives/Badge";
import { Button } from "../../ui/primitives/Button";
import { Switch } from "../../ui/primitives/FormControls";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { TextField } from "../../ui/primitives/TextField";
import { InlineWorkspace } from "../../ui/overlays/Dialog";
import {
  type ExistingImageDraft,
  type PendingImageDraft,
  useImageSetWorkspaceController,
} from "./useImageSetWorkspaceController";

interface ImageSetWorkspaceProps {
  documentId: string | null;
  showToast: ShowToast;
  onImported: (documentId: string) => void;
  onChanged: () => Promise<unknown>;
  onClose: () => void;
}

const fullCrop: ImageSetInclusionCrop = { left: 0, top: 0, right: 1, bottom: 1 };

function CropEditor({
  label,
  value,
  disabled,
  onChange,
}: {
  label: string;
  value: ImageSetInclusionCrop | null;
  disabled: boolean;
  onChange: (value: ImageSetInclusionCrop | null) => void;
}) {
  const crop = value ?? fullCrop;
  function setField(field: keyof ImageSetInclusionCrop, text: string): void {
    const number = Number(text);
    if (!Number.isFinite(number)) return;
    onChange({ ...crop, [field]: number });
  }
  return (
    <fieldset className="tw:m-0 tw:grid tw:min-w-0 tw:grid-cols-2 tw:gap-2 tw:border-0 tw:p-0">
      <legend className="tw:col-span-2 tw:mb-1 tw:text-xs tw:font-medium tw:text-muted-foreground">
        {label}
      </legend>
      {(["left", "top", "right", "bottom"] as const).map((field) => (
        <label key={field} className="tw:grid tw:min-w-0 tw:gap-1 tw:text-xs tw:text-muted-foreground">
          <span className="tw:capitalize">{field}</span>
          <TextField
            type="number"
            min={0}
            max={1}
            step={0.01}
            value={crop[field]}
            disabled={disabled}
            onChange={(event) => setField(field, event.currentTarget.value)}
          />
        </label>
      ))}
      <Button
        size="small"
        variant="ghost"
        className="tw:col-span-2"
        disabled={disabled || value === null}
        onClick={() => onChange(null)}
      >
        Use whole image
      </Button>
    </fieldset>
  );
}

function PendingRow({
  member,
  index,
  total,
  disabled,
  allowMove = true,
  onMove,
  onRotate,
  onCrop,
  onRetry,
  onRemove,
}: {
  member: PendingImageDraft;
  index: number;
  total: number;
  disabled: boolean;
  allowMove?: boolean;
  onMove: (destination: number) => void;
  onRotate: (delta: number) => void;
  onCrop: (crop: ImageSetInclusionCrop | null) => void;
  onRetry: () => void;
  onRemove: () => void;
}) {
  const image = member.inspection.image;
  return (
    <article className="tw:grid tw:min-w-0 tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-3">
      <div className="tw:min-w-0">
        <strong className="tw:block tw:break-all tw:text-sm">{member.staged.displayName}</strong>
        {image ? (
          <small className="tw:text-muted-foreground">
            {image.width} × {image.height} · {(image.byteSize / 1024).toFixed(1)} KB
          </small>
        ) : (
          <small className="tw:text-destructive">{member.inspection.error?.message ?? "Inspection failed."}</small>
        )}
      </div>
      <div className="tw:flex tw:flex-wrap tw:gap-2">
        <Button size="small" variant="ghost" disabled={disabled || !allowMove || index === 0} onClick={() => onMove(index - 1)}>
          Up
        </Button>
        <Button size="small" variant="ghost" disabled={disabled || !allowMove || index === total - 1} onClick={() => onMove(index + 1)}>
          Down
        </Button>
        <Button size="small" variant="ghost" disabled={disabled} onClick={() => onRotate(-1)}>
          Rotate left
        </Button>
        <Button size="small" variant="ghost" disabled={disabled} onClick={() => onRotate(1)}>
          Rotate right
        </Button>
        {!image ? (
          <Button size="small" variant="secondary" disabled={disabled} onClick={onRetry}>
            Retry inspection
          </Button>
        ) : null}
        <Button size="small" variant="dangerGhost" disabled={disabled} onClick={onRemove}>
          Remove
        </Button>
      </div>
      <div className="tw:text-xs tw:text-muted-foreground">
        Orientation: {member.quarterTurns * 90}°
      </div>
      <CropEditor label="Source inclusion (0–1, original upright coordinates)" value={member.inclusion} disabled={disabled} onChange={onCrop} />
    </article>
  );
}

function ExistingRow({
  member,
  index,
  total,
  selected,
  disabled,
  onSelect,
  onMove,
  onRotate,
  onCrop,
}: {
  member: ExistingImageDraft;
  index: number;
  total: number;
  selected: boolean;
  disabled: boolean;
  onSelect: () => void;
  onMove: (destination: number) => void;
  onRotate: (delta: number) => void;
  onCrop: (crop: ImageSetInclusionCrop | null) => void;
}) {
  return (
    <article className="tw:grid tw:min-w-0 tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-3">
      <Button
        variant={selected ? "primary" : "ghost"}
        className="tw:h-auto tw:min-w-0 tw:justify-start tw:whitespace-normal tw:text-left"
        disabled={disabled}
        onClick={onSelect}
      >
        <span className="tw:min-w-0 tw:break-all">{member.originalFilename}</span>
      </Button>
      <div className="tw:flex tw:flex-wrap tw:gap-2">
        <Button size="small" variant="ghost" disabled={disabled || index === 0} onClick={() => onMove(index - 1)}>
          Up
        </Button>
        <Button size="small" variant="ghost" disabled={disabled || index === total - 1} onClick={() => onMove(index + 1)}>
          Down
        </Button>
        <Button size="small" variant="ghost" disabled={disabled} onClick={() => onRotate(-1)}>
          Rotate left
        </Button>
        <Button size="small" variant="ghost" disabled={disabled} onClick={() => onRotate(1)}>
          Rotate right
        </Button>
      </div>
      <div className="tw:text-xs tw:text-muted-foreground">
        Orientation: {member.quarterTurns * 90}°
      </div>
      <CropEditor label="Source inclusion (0–1, original upright coordinates)" value={member.inclusion} disabled={disabled} onChange={onCrop} />
    </article>
  );
}

export function ImageSetWorkspace(props: ImageSetWorkspaceProps) {
  const controller = useImageSetWorkspaceController(props);
  const busy = controller.busyAction !== null;
  const isNew = props.documentId === null;

  if (controller.isLoading) {
    return (
      <InlineWorkspace title="Image source" description="Loading the retained image set." onClose={() => void controller.close()} busy>
        <div className="tw:p-5"><InlineProgress>Loading image source…</InlineProgress></div>
      </InlineWorkspace>
    );
  }

  return (
    <InlineWorkspace
      title={isNew ? "New image set" : controller.details?.title ?? "Image source"}
      description={isNew
        ? "Build one ordered study source from screenshots or photos. Originals remain separate and unchanged."
        : "Inspect originals, edit source order/orientation/inclusion, and review reversible Content filters."}
      onClose={() => void controller.close()}
      busy={busy}
      closeLabel="Close image workspace"
    >
      <div className="tw:grid tw:min-w-0 tw:gap-5 tw:p-4 tw:sm:p-5">
        {controller.errorMessage ? <InlineNotice kind="error">{controller.errorMessage}</InlineNotice> : null}

        {isNew ? (
          <>
            <label className="tw:grid tw:max-w-xl tw:gap-1.5 tw:text-sm tw:font-medium">
              Image-set name
              <TextField value={controller.title} disabled={busy} onChange={(event) => controller.setTitle(event.currentTarget.value)} />
            </label>
            <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2">
              <Button variant="secondary" disabled={busy} onClick={() => void controller.addImage()}>
                Add image
              </Button>
              <Button
                variant="primary"
                loading={controller.busyAction === "import"}
                disabled={busy || controller.pending.length === 0 || controller.pending.some((member) => member.inspection.image === null)}
                onClick={() => void controller.importCurrent()}
              >
                Import image set
              </Button>
              {controller.maximumCount ? (
                <span className="tw:text-sm tw:text-muted-foreground">
                  {controller.pending.length}/{controller.maximumCount} images
                </span>
              ) : null}
            </div>
            {controller.pending.length === 0 ? (
              <InlineNotice>
                Add images in reading order. Failed selections stay visible so you can retry or remove them before import.
              </InlineNotice>
            ) : (
              <div className="tw:grid tw:gap-3 tw:lg:grid-cols-2">
                {controller.pending.map((member, index) => (
                  <PendingRow
                    key={member.id}
                    member={member}
                    index={index}
                    total={controller.pending.length}
                    disabled={busy}
                    onMove={(destination) => controller.movePending(member.id, destination)}
                    onRotate={(delta) => controller.rotatePending(member.id, delta)}
                    onCrop={(crop) => controller.setPendingInclusion(member.id, crop)}
                    onRetry={() => void controller.retryPending(member.id)}
                    onRemove={() => void controller.removePending(member.id)}
                  />
                ))}
              </div>
            )}
          </>
        ) : controller.details ? (
          <>
            <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2">
              <Badge tone={controller.details.prepared ? "success" : "warning"}>
                {controller.details.prepared ? "Prepared" : "Needs local preparation"}
              </Badge>
              <span className="tw:text-sm tw:text-muted-foreground">
                Revision {controller.details.revision} · {controller.details.members.length} image{controller.details.members.length === 1 ? "" : "s"}
              </span>
              <Button variant="secondary" disabled={busy} onClick={() => void controller.addImage()}>
                Append image
              </Button>
              <Button
                variant="primary"
                loading={controller.busyAction === "source"}
                disabled={busy}
                onClick={() => void controller.saveSource()}
              >
                Save source changes
              </Button>
              <Button
                variant="ghost"
                loading={controller.busyAction === "prepare"}
                disabled={busy}
                onClick={() => void controller.retryPreparation()}
              >
                Retry local OCR
              </Button>
            </div>

            <div className="tw:grid tw:min-w-0 tw:items-start tw:gap-5 tw:lg:grid-cols-[minmax(17rem,23rem)_minmax(0,1fr)]">
              <section className="tw:grid tw:min-w-0 tw:gap-3" aria-label="Image order and source inclusion">
                {controller.members.map((member, index) => (
                  <ExistingRow
                    key={member.id}
                    member={member}
                    index={index}
                    total={controller.members.length}
                    selected={controller.selectedMemberId === member.id}
                    disabled={busy}
                    onSelect={() => controller.setSelectedMemberId(member.id)}
                    onMove={(destination) => controller.moveExisting(member.id, destination)}
                    onRotate={(delta) => controller.rotateExisting(member.id, delta)}
                    onCrop={(crop) => controller.setExistingInclusion(member.id, crop)}
                  />
                ))}
                {controller.pending.map((member, index) => (
                  <PendingRow
                    key={member.id}
                    member={member}
                    index={controller.members.length + index}
                    total={controller.members.length + controller.pending.length}
                    disabled={busy}
                    allowMove={false}
                    onMove={() => undefined}
                    onRotate={(delta) => controller.rotatePending(member.id, delta)}
                    onCrop={(crop) => controller.setPendingInclusion(member.id, crop)}
                    onRetry={() => void controller.retryPending(member.id)}
                    onRemove={() => void controller.removePending(member.id)}
                  />
                ))}
              </section>

              <div className="tw:grid tw:min-w-0 tw:gap-5">
                <section className="tw:grid tw:min-w-0 tw:gap-3 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:p-4" aria-label="Original source inspection">
                  <div>
                    <h3 className="tw:m-0 tw:text-base tw:font-medium">Original source inspection</h3>
                    <p className="tw:mt-1 tw:mb-0 tw:text-sm tw:text-muted-foreground">
                      This view comes from the retained original. Content filtering never rewrites these pixels.
                    </p>
                  </div>
                  {controller.preview ? (
                    <img
                      src={controller.preview.dataUrl}
                      alt={controller.selectedMember?.originalFilename ?? "Retained source image"}
                      className="tw:max-h-[32rem] tw:max-w-full tw:justify-self-center tw:rounded-lg tw:object-contain tw:ring-1 tw:ring-border"
                    />
                  ) : (
                    <InlineProgress>Loading original image…</InlineProgress>
                  )}
                </section>

                <section className="tw:grid tw:min-w-0 tw:gap-4 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:p-4" aria-label="Content filter">
                  <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-start tw:justify-between tw:gap-3">
                    <div className="tw:min-w-0">
                      <h3 className="tw:m-0 tw:text-base tw:font-medium">Content filter</h3>
                      <p className="tw:mt-1 tw:mb-0 tw:text-sm tw:text-muted-foreground">
                        Automatic exclusions are conservative. Restore educational interface text when the interface itself is the lesson.
                      </p>
                    </div>
                    <label className="tw:flex tw:min-h-11 tw:items-center tw:gap-2 tw:text-sm">
                      <Switch
                        checked={controller.includeInterface}
                        disabled={busy || !controller.details?.prepared}
                        onCheckedChange={controller.setIncludeInterface}
                      />
                      Include interface content
                    </label>
                  </div>

                  {!controller.details.prepared ? (
                    <InlineNotice kind="warning">
                      Prepare the current source before changing region filters. The previous processing receipt remains inspectable while edits are pending.
                    </InlineNotice>
                  ) : null}

                  {controller.selectedMember?.processing?.regions.length ? (
                    <div className="tw:grid tw:min-w-0 tw:gap-2">
                      {controller.selectedMember.processing.regions.map((region) => (
                        <article key={region.id} className="tw:grid tw:min-w-0 tw:gap-2 tw:rounded-lg tw:bg-muted/35 tw:p-3">
                          <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-start tw:justify-between tw:gap-2">
                            <p className="tw:m-0 tw:min-w-0 tw:flex-1 tw:whitespace-pre-wrap tw:break-words tw:text-sm tw:leading-relaxed">
                              {region.text || "[No recognized text]"}
                            </p>
                            <NativeSelect
                              aria-label={`Content filter for ${region.text || "unreadable region"}`}
                              value={controller.regionActions[region.id] ?? "automatic"}
                              disabled={busy || !controller.details.prepared}
                              onChange={(event) =>
                                controller.setRegionAction(
                                  region.id,
                                  event.currentTarget.value === "automatic"
                                    ? null
                                    : event.currentTarget.value as "include" | "exclude" | "metadata",
                                )
                              }
                            >
                              <option value="automatic">Automatic</option>
                              <option value="include">Include for learning</option>
                              <option value="exclude">Exclude</option>
                              <option value="metadata">Metadata only</option>
                            </NativeSelect>
                          </div>
                          <small className="tw:text-muted-foreground">
                            Automatic: {region.automaticRole.replaceAll("_", " ")} · Current: {region.role.replaceAll("_", " ")}. {region.reason}
                          </small>
                        </article>
                      ))}
                    </div>
                  ) : (
                    <InlineNotice>
                      {controller.selectedMember?.processing?.ocrAvailable === false
                        ? "Local OCR could not read this image. Retry OCR or inspect the retained original; oStudy will not invent obscured text."
                        : "No inspected text regions are available for this image yet."}
                    </InlineNotice>
                  )}
                  <div className="tw:flex tw:justify-end">
                    <Button
                      variant="primary"
                      loading={controller.busyAction === "filters"}
                      disabled={busy || !controller.details.prepared}
                      onClick={() => void controller.saveFilters()}
                    >
                      Save Content filter
                    </Button>
                  </div>
                </section>
              </div>
            </div>
          </>
        ) : (
          <InlineNotice kind="error">This image set is no longer available.</InlineNotice>
        )}
      </div>
    </InlineWorkspace>
  );
}
