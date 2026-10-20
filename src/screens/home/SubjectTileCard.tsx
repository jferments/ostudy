import { useId } from "react";

import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Card } from "../../ui/primitives/Card";
import { ItemTrigger } from "../../ui/primitives/Item";
import { countLabel, SUBJECT_COLORS, type SubjectTile } from "./homeModel";
import { SubjectBadge } from "./SubjectBadge";

interface SubjectTileCardProps {
  tile: SubjectTile;
  compact: boolean;
  onOpen: () => void;
}

/**
 * One subject on Home: its badge and due count, its name, what it holds, and a
 * status line from real state. The name is the tile's button; the rest
 * describes it.
 */
export function SubjectTileCard({ tile, compact, onOpen }: SubjectTileCardProps) {
  const id = useId();
  const pillId = `${id}-due`;
  const metaId = `${id}-meta`;
  const statusId = `${id}-status`;
  const color = tile.colorIndex === null ? null : SUBJECT_COLORS[tile.colorIndex];
  const pill = tilePill(tile);
  const meta = tileMeta(tile);
  const status = compact ? null : tileStatus(tile);
  const guide = compact ? null : guideLine(tile);
  const describedBy = [pill ? pillId : null, metaId, status ? statusId : null]
    .filter(Boolean)
    .join(" ");

  return (
    <Card
      as="article"
      data-testid="subject-tile"
      data-subject-id={tile.subjectId ?? "none"}
      className={cn(
        "tw:relative tw:h-full tw:gap-3.5 tw:p-5 tw:transition-shadow tw:duration-(--ostudy-motion-duration-fast) tw:reduce-motion:transition-none tw:hover:shadow-md tw:has-[button:focus-visible]:ring-3 tw:has-[button:focus-visible]:ring-ring/50",
        compact && "tw:gap-2.5 tw:rounded-md tw:p-3.5",
      )}
    >
      <div className="tw:flex tw:items-start tw:justify-between tw:gap-2">
        <SubjectBadge subject={tile} size={compact ? "medium" : "large"} />
        {pill ? (
          <span
            id={pillId}
            className={cn(
              "tw:inline-flex tw:min-w-0 tw:items-center tw:gap-1.5 tw:rounded-full tw:font-semibold tw:whitespace-nowrap",
              compact ? "tw:px-2.5 tw:py-0.5 tw:text-label" : "tw:px-3 tw:py-0.5 tw:text-small",
              pill.kind === "due"
                ? color
                  ? color.soft
                  : "tw:bg-accent tw:text-accent-foreground"
                : pill.kind === "done"
                  ? "tw:bg-success-soft tw:text-success"
                  : "tw:bg-(--ostudy-color-canvas-inset) tw:text-muted-foreground",
            )}
          >
            {pill.kind === "done" && !compact ? <Icon name="check" size={14} /> : null}
            {pill.kind === "preparing" && !compact ? <Icon name="sparkles" size={14} /> : null}
            {pill.text}
          </span>
        ) : null}
      </div>
      <div className="tw:flex tw:min-w-0 tw:flex-col tw:gap-0.5">
        <h3
          className={cn(
            "tw:m-0 tw:min-w-0 tw:leading-tight tw:font-semibold tw:wrap-anywhere",
            compact ? "tw:text-body" : "tw:text-title-small",
          )}
        >
          {/* The name's button stretches over the whole card, so the card is the touch target. */}
          <ItemTrigger
            aria-describedby={describedBy}
            onClick={onOpen}
            className="tw:inline tw:outline-none tw:coarse:min-h-0 tw:after:absolute tw:after:inset-0 tw:after:content-['']"
          >
            {tile.name}
          </ItemTrigger>
        </h3>
        <p id={metaId} className={cn("tw:m-0 tw:text-muted-foreground", compact ? "tw:text-label" : "tw:text-small")}>
          {meta}
        </p>
      </div>
      {status ? (
        <p id={statusId} className="tw:m-0 tw:flex tw:min-w-0 tw:items-start tw:gap-1.5 tw:text-small tw:text-muted-foreground">
          <Icon name={status.icon} size={16} className="tw:mt-0.5 tw:shrink-0" />
          <span className="tw:min-w-0">{status.text}</span>
        </p>
      ) : null}
      {guide ? (
        <p className="tw:mt-auto tw:mb-0 tw:flex tw:items-center tw:gap-1.5 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:pt-3 tw:text-label tw:text-muted-foreground">
          <Icon name="study-guide" size={15} className="tw:shrink-0" />
          {guide}
        </p>
      ) : null}
    </Card>
  );
}

type TilePill = { kind: "due" | "done" | "preparing"; text: string };

function tilePill(tile: SubjectTile): TilePill | null {
  if (tile.preparing) return { kind: "preparing", text: "Preparing" };
  if (tile.dueCount > 0) return { kind: "due", text: `${tile.dueCount.toLocaleString()} due` };
  if (tile.summary.cardCount > 0) return { kind: "done", text: "All done" };
  return null;
}

function tileMeta(tile: SubjectTile): string {
  const { summary } = tile;
  const cards = countLabel(summary.cardCount, "card");
  if (summary.sourceCount === 0) return summary.cardCount > 0 ? `Cards only · ${cards}` : "No cards yet";
  return `${countLabel(summary.sourceCount, "source")} · ${cards}`;
}

function tileStatus(tile: SubjectTile): { icon: "sparkles" | "practice"; text: string } | null {
  if (tile.preparing) {
    return { icon: "sparkles", text: `${tile.preparing.step}: ${tile.preparing.documentTitle}` };
  }
  return null;
}

/** The guide line: what Home can back with data until reading progress exists. */
function guideLine(tile: SubjectTile): string | null {
  if (tile.subjectId === null) return null;
  const { summary } = tile;
  if (summary.hasOverviewGuide) return "Overview guide ready";
  if (summary.sourceCount > 0) return "No overview guide yet";
  return "No sources yet";
}
