import { useCallback, useEffect, useRef, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import type { LessonSummary } from "../../ipc/lessons";
import { deletePodcast, listSubjectPodcasts, type PodcastSummary } from "../../ipc/podcasts";
import { Icon } from "../../ui/icons/Icon";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { Card } from "../../ui/primitives/Card";
import { EmptyState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { ListRow, ListRowMedia, ListRows } from "../../ui/primitives/ListRow";
import { SubjectMaterialBeingMade, useMaterialProgress } from "../documents/MaterialsBeingMade";
import { MakePodcastDialog } from "./MakePodcastDialog";

interface PodcastsSectionProps {
  headingId: string;
  subjectId: string;
  /** The subject's written lessons, in course order. */
  lessons: LessonSummary[];
  onOpenPodcast: (podcastId: string) => void;
}

/** The subject's podcasts, and making one from its lessons. */
export function PodcastsSection({ headingId, subjectId, lessons, onOpenPodcast }: PodcastsSectionProps) {
  const lane = useRequestLane();
  const confirm = useConfirm();
  const [podcasts, setPodcasts] = useState<PodcastSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [making, setMaking] = useState(false);

  const refresh = useCallback(() => {
    void latestResult(lane, () => listSubjectPodcasts(subjectId)).then(
      (value) => {
        if (value === undefined) return;
        setPodcasts(value);
        setError(null);
      },
      (failure: unknown) => setError(`Podcasts could not load: ${getErrorMessage(failure)}`),
    );
  }, [lane, subjectId]);
  useEffect(refresh, [refresh]);

  const remove = async (podcast: PodcastSummary) => {
    const approved = await confirm({
      title: "Delete this podcast?",
      description: `“${podcast.title}” is deleted with its audio. The lessons it was made from stay.`,
      confirmLabel: "Delete podcast",
      destructive: true,
    });
    if (!approved) return;
    deletePodcast(podcast.id).then(refresh, (failure: unknown) =>
      setError(`The podcast was not deleted: ${getErrorMessage(failure)}`),
    );
  };

  const beingMade = useMaterialProgress(subjectId, "podcast") !== null;
  const wasBeingMade = useRef(false);
  useEffect(() => {
    // The finished podcast joins the list.
    if (wasBeingMade.current && !beingMade) refresh();
    wasBeingMade.current = beingMade;
  }, [beingMade, refresh]);
  if (lessons.length === 0 && !podcasts?.length && !beingMade) {
    return podcasts ? (
      <EmptyState icon="headphones" title="No podcasts yet" description="A podcast is made from the subject's lessons, once it has some." />
    ) : null;
  }
  return (
    <section aria-labelledby={headingId} data-testid="subject-podcasts" className="tw:flex tw:flex-col tw:gap-3">
      <div className="tw:flex tw:items-center tw:justify-between tw:gap-3">
        <h2 id={headingId} className="tw:m-0 tw:text-body-large tw:font-semibold">
          Podcasts
        </h2>
        {lessons.length > 0 ? (
          <Button size="small" className="tw:rounded-full" onClick={() => setMaking(true)}>
            <Icon name="headphones" size={16} /> Make a podcast
          </Button>
        ) : null}
      </div>
      {error ? <InlineNotice kind="error">{error}</InlineNotice> : null}
      <SubjectMaterialBeingMade subjectId={subjectId} kind="podcast" />
      {podcasts && podcasts.length > 0 ? (
        <Card as="div" className="tw:gap-0 tw:py-0">
          <ListRows>
            {podcasts.map((podcast) => (
              <ListRow
                key={podcast.id}
                data-testid="subject-podcast"
                leading={<ListRowMedia icon="headphones" />}
                title={podcast.title}
                meta={`${podcast.minutes} min · ${podcast.lessonTitles.join(", ")}`}
                trailing={
                  <div className="tw:flex tw:items-center tw:gap-1">
                    <Button
                      variant="secondary"
                      size="small"
                      aria-label={`Play ${podcast.title}`}
                      onClick={() => onOpenPodcast(podcast.id)}
                    >
                      <Icon name="play" size={14} /> Play
                    </Button>
                    <OverflowMenu
                      label={`More for ${podcast.title}`}
                      items={[
                        { id: "delete", label: "Delete", destructive: true, onSelect: () => void remove(podcast) },
                      ]}
                    />
                  </div>
                }
              />
            ))}
          </ListRows>
        </Card>
      ) : podcasts && !beingMade ? (
        <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
          Two hosts talk through the lessons you pick, to listen to anywhere.
        </p>
      ) : null}
      {making ? (
        <MakePodcastDialog
          subjectId={subjectId}
          lessons={lessons}
          onClose={() => setMaking(false)}
          onMade={(podcast) => {
            setMaking(false);
            refresh();
            onOpenPodcast(podcast.id);
          }}
        />
      ) : null}
    </section>
  );
}
