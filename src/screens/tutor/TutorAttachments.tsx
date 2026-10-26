import type { TutorAttachment } from "../../ipc/tutor";
import { managedAssetUrl } from "../../platform/assets";
import { Icon } from "../../ui/icons/Icon";
import { IconButton } from "../../ui/icons/IconButton";
import { cn } from "../../ui/internal/cn";
import type { ComposerAttachment } from "./useTutorAttachments";

const chip = "tw:flex tw:h-12 tw:min-w-0 tw:max-w-60 tw:items-center tw:gap-2 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-muted/40 tw:py-1 tw:pr-0.5 tw:pl-1";
const thumb = "tw:size-10 tw:shrink-0 tw:rounded-lg tw:object-cover";
const fileMark = "tw:grid tw:size-10 tw:shrink-0 tw:place-items-center tw:rounded-lg tw:bg-muted tw:text-muted-foreground";
const spinner = "tw:size-4 tw:animate-spin tw:rounded-full tw:border-2 tw:border-solid tw:border-border tw:border-t-primary tw:reduce-motion:animate-none";

/** What waits in the composer to go with the next question. */
export function ComposerAttachments({ items, disabled, onRemove }: {
  items: readonly ComposerAttachment[];
  disabled: boolean;
  onRemove: (key: string) => void;
}) {
  if (!items.length) return null;
  return (
    <ul aria-label="Attached" className="tw:m-0 tw:flex tw:w-full tw:min-w-0 tw:list-none tw:flex-wrap tw:gap-2 tw:p-0 tw:pt-1">
      {items.map((item) => (
        <li key={item.key} className={chip}>
          {item.kind === "image" && item.assetId
            ? <img src={managedAssetUrl(item.assetId)} alt="" className={thumb} />
            : (
              <span className={fileMark} aria-hidden="true">
                {item.ready ? <Icon name={item.kind === "image" ? "image" : "document"} size={20} /> : <span className={spinner} />}
              </span>
            )}
          <span className="tw:min-w-0 tw:flex-1 tw:truncate tw:text-small" title={item.name}>
            {item.name}
            {item.ready ? null : <span className="tw:sr-only">, adding</span>}
          </span>
          <IconButton icon="close" iconSize={16} label={`Remove ${item.name}`} disabled={disabled} onClick={() => onRemove(item.key)} />
        </li>
      ))}
    </ul>
  );
}

/** The pictures and files a sent message carried, above its text. */
export function SentAttachments({ attachments, className }: {
  attachments: readonly TutorAttachment[];
  className?: string;
}) {
  if (!attachments.length) return null;
  const pictures = attachments.filter((attachment) => attachment.kind === "image");
  const files = attachments.filter((attachment) => attachment.kind === "text");
  return (
    <div className={cn("tw:ml-auto tw:flex tw:max-w-[min(85%,36rem)] tw:min-w-0 tw:flex-wrap tw:justify-end tw:gap-2", className)}>
      {pictures.map((picture) => (
        <img
          key={picture.assetId}
          src={managedAssetUrl(picture.assetId)}
          alt={picture.name}
          className="tw:max-h-48 tw:max-w-full tw:rounded-xl tw:border tw:border-solid tw:border-border tw:object-contain tw:compact:max-h-36"
        />
      ))}
      {files.map((file, index) => (
        <span key={`${file.name}-${index}`} className={chip}>
          <span className={fileMark} aria-hidden="true"><Icon name="document" size={20} /></span>
          <span className="tw:min-w-0 tw:truncate tw:pr-2 tw:text-small" title={file.name}>{file.name}</span>
        </span>
      ))}
    </div>
  );
}
