import { useEffect, useRef, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import { getPodcast, type Podcast } from "../../ipc/podcasts";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { ChipButton } from "../../ui/primitives/Chip";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { ItemTrigger } from "../../ui/primitives/Item";
import { Progress } from "../../ui/primitives/Progress";
import { PODCAST_SKIP_SECONDS, usePodcastPlayer } from "./usePodcastPlayer";

interface PodcastPlayerProps {
  podcastId: string;
  subjectName: string;
  onClose: () => void;
}

/**
 * A podcast: its chapters, the transcript with the turn being spoken marked,
 * and play, pause, skip and speed. Choosing a turn or a chapter plays from
 * there.
 */
export function PodcastPlayer({ podcastId, subjectName, onClose }: PodcastPlayerProps) {
  const lane = useRequestLane();
  const [podcast, setPodcast] = useState<Podcast | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const player = usePodcastPlayer(podcast);
  const current = useRef<HTMLLIElement>(null);

  useEffect(() => {
    void latestResult(lane, () => getPodcast(podcastId)).then(
      (value) => {
        if (value !== undefined) setPodcast(value);
      },
      (error: unknown) => setReadError(getErrorMessage(error)),
    );
  }, [lane, podcastId]);

  // The transcript follows the voice.
  useEffect(() => {
    if (player.playback !== "idle") current.current?.scrollIntoView?.({ block: "nearest", behavior: "smooth" });
  }, [player.index, player.playback]);

  const back = (
    <nav aria-label="Breadcrumb">
      <Button size="small" variant="text" className="tw:ps-0" onClick={onClose}>
        <Icon name="chevron-left" size={16} /> {subjectName}
      </Button>
    </nav>
  );
  if (!podcast) {
    return (
      <div data-testid="podcast-player" className="tw:flex tw:flex-col tw:gap-4">
        {back}
        {readError ? (
          <InlineNotice kind="error">The podcast could not load: {readError}</InlineNotice>
        ) : (
          <LoadingState label="Loading the podcast…" />
        )}
      </div>
    );
  }

  const playing = player.turns[player.index];
  const chapterStarts = podcast.chapters.map((_, chapter) =>
    player.turns.findIndex((turn) => turn.chapter === chapter),
  );
  const hosts = podcast.hosts.map((host) => host.name).join(" and ");
  const detail = `${podcast.chapters.length === 1 ? "One lesson" : `${podcast.chapters.length} lessons`} · ${hosts} · ${podcast.minutes} min`;

  return (
    <div data-testid="podcast-player" className="tw:flex tw:flex-col tw:gap-4">
      {back}
      <header className="tw:flex tw:min-w-0 tw:flex-col tw:gap-1">
        <p className="tw:m-0 tw:text-small tw:text-muted-foreground">{detail}</p>
        <h1 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight tw:break-words">{podcast.title}</h1>
      </header>
      <div role="group" aria-label="Chapters" className="tw:flex tw:flex-wrap tw:gap-2">
        {podcast.chapters.map((chapter, position) => (
          <ChipButton
            key={chapter.lessonId}
            selected={playing?.chapter === position}
            aria-pressed={playing?.chapter === position}
            onClick={() => player.playTurn(Math.max(0, chapterStarts[position] ?? 0))}
          >
            {position + 1} · {chapter.title}
          </ChipButton>
        ))}
      </div>
      {player.error ? <InlineNotice kind="error">The podcast could not be played: {player.error}</InlineNotice> : null}
      <ol aria-label="Transcript" className="tw:m-0 tw:flex tw:list-none tw:flex-col tw:gap-1 tw:p-0">
        {player.turns.map((turn, position) => {
          const host = podcast.hosts[turn.host];
          const now = position === player.index && player.playback !== "idle";
          return (
            <li key={turn.key} ref={position === player.index ? current : undefined}>
              {chapterStarts.includes(position) ? (
                <h2 className="tw:m-0 tw:pt-3 tw:pb-1 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground">
                  {podcast.chapters[turn.chapter]?.title}
                </h2>
              ) : null}
              <ItemTrigger
                aria-current={now ? "true" : undefined}
                aria-label={`${host?.name ?? "Host"}: ${turn.text}`}
                onClick={() => player.playTurn(position)}
                className={cn(
                  "tw:w-full tw:items-start tw:gap-3 tw:rounded-md tw:px-3 tw:py-2 tw:text-start tw:outline-none tw:hover:bg-accent/40 tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50",
                  now && "tw:bg-accent/60",
                )}
              >
                <span
                  aria-hidden="true"
                  className={cn(
                    "tw:grid tw:size-8 tw:shrink-0 tw:place-items-center tw:rounded-full tw:text-label tw:font-semibold",
                    turn.host === 0
                      ? "tw:bg-primary tw:text-primary-foreground"
                      : "tw:bg-(--ostudy-color-canvas-inset) tw:text-foreground",
                  )}
                >
                  {host?.name.charAt(0) ?? "?"}
                </span>
                <span className="tw:flex tw:min-w-0 tw:flex-1 tw:flex-col tw:gap-0.5">
                  <span className="tw:text-small tw:font-semibold">{host?.name}</span>
                  <span className={cn("tw:break-words", !now && "tw:text-muted-foreground")}>{turn.text}</span>
                </span>
              </ItemTrigger>
            </li>
          );
        })}
      </ol>
      <div className="tw:sticky tw:bottom-0 tw:flex tw:flex-col tw:gap-2 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:bg-background tw:py-3 tw:pb-[max(0.75rem,env(safe-area-inset-bottom))]">
        <Progress
          value={player.index + (player.playback === "idle" ? 0 : 1)}
          max={Math.max(1, player.turns.length)}
          label="Podcast progress"
          valueText={`Part ${player.index + 1} of ${player.turns.length}`}
          size="small"
        />
        <div className="tw:flex tw:items-center tw:justify-center tw:gap-3">
          <Button
            variant="ghost"
            className="tw:rounded-full"
            aria-label={`Back ${PODCAST_SKIP_SECONDS} seconds`}
            onClick={() => player.skip(-PODCAST_SKIP_SECONDS)}
          >
            <Icon name="rewind" size={20} />
          </Button>
          <Button
            variant="primary"
            className="tw:size-14 tw:rounded-full tw:shadow-glow"
            aria-label={
              player.playback === "playing" ? "Pause" : player.playback === "preparing" ? "Preparing" : "Play"
            }
            disabled={player.playback === "preparing" || player.turns.length === 0}
            onClick={player.toggle}
          >
            <Icon name={player.playback === "playing" ? "pause" : "play"} size={22} />
          </Button>
          <Button
            variant="ghost"
            className="tw:rounded-full"
            aria-label={`Ahead ${PODCAST_SKIP_SECONDS} seconds`}
            onClick={() => player.skip(PODCAST_SKIP_SECONDS)}
          >
            <Icon name="skip-ahead" size={20} />
          </Button>
          <Button
            variant="ghost"
            className="tw:min-w-14 tw:rounded-full tw:tabular-nums"
            aria-label={`Speed ${player.rate} times`}
            onClick={player.changeSpeed}
          >
            {player.rate}×
          </Button>
        </div>
        {player.playback === "preparing" ? (
          <p role="status" className="tw:m-0 tw:text-center tw:text-small tw:text-muted-foreground">
            The hosts are getting ready…
          </p>
        ) : null}
      </div>
      <audio ref={player.audioRef} className="tw:hidden" />
    </div>
  );
}
