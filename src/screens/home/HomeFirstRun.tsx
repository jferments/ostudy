import { useArtwork } from "../../ui/artwork/Artwork";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { Card } from "../../ui/primitives/Card";

interface HomeFirstRunProps {
  /** Archived decks are kept in Library, so the library is not empty. */
  hasArchivedDecks: boolean;
  importing: boolean;
  onCreateSubject: () => void;
  onImportDocument: () => void;
  onOpenLibrary: () => void;
}

/**
 * Home for a library with nothing in it: the mascot welcomes (a plain icon
 * when mascot art is off), and creating a subject is the one main action. A
 * library whose decks are all archived has nothing to study but is not empty,
 * so it says where those decks are instead.
 */
export function HomeFirstRun({
  hasArchivedDecks,
  importing,
  onCreateSubject,
  onImportDocument,
  onOpenLibrary,
}: HomeFirstRunProps) {
  const { definition, enabled } = useArtwork("empty.library");
  const image = enabled ? definition.image : undefined;
  return (
    <Card
      as="section"
      aria-labelledby="home-first-run-heading"
      data-testid="home-first-run"
      className="tw:items-center tw:gap-5 tw:px-6 tw:py-10 tw:text-center tw:compact:px-4 tw:compact:py-8"
    >
      {image ? (
        <img
          src={image.src}
          srcSet={image.srcSet}
          alt=""
          draggable={false}
          data-artwork-slot="empty.library"
          className="tw:pointer-events-none tw:block tw:size-40 tw:select-none tw:compact:size-32"
        />
      ) : (
        <span
          aria-hidden="true"
          className="tw:grid tw:size-16 tw:place-items-center tw:rounded-full tw:bg-accent tw:text-accent-foreground"
        >
          <Icon name="library" size={28} />
        </span>
      )}
      <div className="tw:grid tw:max-w-[34rem] tw:gap-2">
        <h2
          id="home-first-run-heading"
          className="tw:m-0 tw:text-title tw:leading-tight tw:font-semibold tw:tracking-tight tw:compact:text-section"
        >
          {hasArchivedDecks ? "Nothing to study yet" : "Your library is empty"}
        </h2>
        <p className="tw:m-0 tw:text-body tw:text-muted-foreground">
          {hasArchivedDecks ? "Your archived decks are in Library. " : null}
          Start with a subject for each course. It keeps the course&apos;s documents, study guides
          and flashcard decks together.
        </p>
      </div>
      <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-center tw:gap-3">
        <Button variant="primary" size="large" onClick={onCreateSubject}>
          <Icon name="plus" size={18} /> Create a subject
        </Button>
        <Button size="large" loading={importing} onClick={onImportDocument}>
          <Icon name="upload" size={18} /> Import a document
        </Button>
        <Button variant="text" size="large" onClick={onOpenLibrary}>
          Open Library
        </Button>
      </div>
    </Card>
  );
}
