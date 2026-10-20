import { useId } from "react";

import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { ItemTrigger } from "../../ui/primitives/Item";
import { countLabel, SUBJECT_COLORS, type SubjectTile } from "./homeModel";
import { SubjectBadge } from "./SubjectBadge";

interface SubjectTileCardProps {
  tile: SubjectTile;
  compact: boolean;
  /** The subject's remembered rate over the last 30 days; `null` before any review, undefined while unknown. */
  remembered?: number | null;
  onOpen: () => void;
}

/**
 * One subject on Home: a row with its badge, its name, what it holds or the
 * document it is preparing, how much of it is remembered, and its due count. The name is the row's button
 * and stretches over the whole row; the rest describes it.
 */
export function SubjectTileCard({ tile, compact, remembered, onOpen }: SubjectTileCardProps) {
  const id = useId();
  const pillId = `${id}-due`;
  const metaId = `${id}-meta`;
  const rememberedId = `${id}-remembered`;
  const percent = remembered === undefined || remembered === null ? null : Math.round(remembered * 100);
  const color = tile.colorIndex === null ? null : SUBJECT_COLORS[tile.colorIndex];
  const pill = tilePill(tile);
  const status = tileStatus(tile);
  // Phones fold the rate into the row's line; wider rows draw it as a bar.
  const meta = status ? `${status.text}` : compact && percent !== null ? `${tileMeta(tile)} · ${percent}% remembered` : tileMeta(tile);
  const showBar = !compact && percent !== null;
  const describedBy = [pill ? pillId : null, metaId, showBar ? rememberedId : null].filter(Boolean).join(" ");

  return (
    <article
      data-testid="subject-tile"
      data-subject-id={tile.subjectId ?? "none"}
      className="tw:relative tw:flex tw:min-h-14 tw:items-center tw:gap-3 tw:rounded-md tw:py-2 tw:has-[button:focus-visible]:ring-3 tw:has-[button:focus-visible]:ring-ring/50"
    >
      <SubjectBadge subject={tile} size={compact ? "small" : "medium"} />
      <div className="tw:flex tw:min-w-0 tw:flex-1 tw:flex-col">
        <h3 className="tw:m-0 tw:min-w-0 tw:text-body tw:leading-tight tw:font-semibold tw:wrap-anywhere">
          {/* The name's button stretches over the whole row, so the row is the touch target. */}
          <ItemTrigger
            aria-describedby={describedBy}
            onClick={onOpen}
            className="tw:inline tw:outline-none tw:coarse:min-h-0 tw:after:absolute tw:after:inset-0 tw:after:content-['']"
          >
            {tile.name}
          </ItemTrigger>
        </h3>
        <p id={metaId} className="tw:m-0 tw:flex tw:min-w-0 tw:items-center tw:gap-1.5 tw:text-small tw:text-muted-foreground">
          {status ? <Icon name={status.icon} size={14} className="tw:shrink-0" /> : null}
          <span className="tw:min-w-0 tw:truncate">{meta}</span>
        </p>
      </div>
      {showBar ? (
        <span id={rememberedId} className="tw:grid tw:w-36 tw:shrink-0 tw:gap-1">
          <span aria-hidden="true" className="tw:h-1.5 tw:overflow-hidden tw:rounded-full tw:bg-progress-track">
            <span className="tw:block tw:h-full tw:rounded-full tw:bg-primary" style={{ width: `${percent}%` }} />
          </span>
          <span className="tw:text-label tw:text-muted-foreground tw:tabular-nums">{percent}% remembered</span>
        </span>
      ) : null}
      {pill ? (
        <span
          id={pillId}
          className={cn(
            "tw:inline-flex tw:shrink-0 tw:items-center tw:gap-1 tw:rounded-full tw:px-2.5 tw:py-0.5 tw:text-label tw:font-semibold tw:whitespace-nowrap",
            pill.kind === "due"
              ? color
                ? color.soft
                : "tw:bg-accent tw:text-accent-foreground"
              : pill.kind === "done"
                ? "tw:bg-success-soft tw:text-success"
                : "tw:bg-(--ostudy-color-canvas-inset) tw:text-muted-foreground",
          )}
        >
          {pill.kind === "done" ? <Icon name="check" size={13} /> : null}
          {pill.text}
        </span>
      ) : null}
    </article>
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
