import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { SUBJECT_COLORS, type SubjectTile } from "./homeModel";

const SIZES = {
  small: { box: "tw:size-8 tw:rounded-sm tw:text-label", icon: 16 },
  medium: { box: "tw:size-10 tw:rounded-sm tw:text-body", icon: 18 },
  large: { box: "tw:size-13 tw:rounded-md tw:text-title-small", icon: 22 },
} as const;

interface SubjectBadgeProps {
  subject: Pick<SubjectTile, "subjectId" | "initials" | "colorIndex">;
  size: keyof typeof SIZES;
  className?: string;
}

/**
 * A subject's initials on its color. No subject shows a folder on the inset
 * surface instead. Decorative: the subject's name is always written beside it.
 */
export function SubjectBadge({ subject, size, className }: SubjectBadgeProps) {
  const color = subject.colorIndex === null ? null : SUBJECT_COLORS[subject.colorIndex];
  const { box, icon } = SIZES[size];
  return (
    <span
      aria-hidden="true"
      className={cn(
        "tw:grid tw:shrink-0 tw:place-items-center tw:font-heading tw:leading-none tw:font-semibold",
        box,
        color
          ? cn(color.badge, "tw:text-primary-foreground")
          : "tw:bg-(--ostudy-color-canvas-inset) tw:text-muted-foreground",
        className,
      )}
    >
      {subject.subjectId === null ? <Icon name="folder" size={icon} /> : subject.initials}
    </span>
  );
}
