import { useArtwork } from "../../ui/artwork/Artwork";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { HomeGettingStarted } from "./HomeGettingStarted";

interface HomeFirstRunProps {
  /** Archived decks are kept in Library, so the library is not empty. */
  hasArchivedDecks: boolean;
  importing: boolean;
  onCreateSubject: () => void;
  onImportDocument: () => void;
  onImportDeck: () => void;
  onOpenLibrary: () => void;
  onSetUpAi: () => void;
  onTakeTour: () => void;
}

/**
 * Home for a library with nothing in it: the mascot welcomes (a plain icon
 * when mascot art is off), creating a subject is the one main action, and a
 * document can come in beside it. A library whose decks are all
 * archived has nothing to study but is not empty, so it offers Library, where
 * those decks are. Below it sit ways to begin: importing a deck, the tour,
 * and AI while none is set up.
 */
export function HomeFirstRun({
  hasArchivedDecks,
  importing,
  onCreateSubject,
  onImportDocument,
  onImportDeck,
  onOpenLibrary,
  onSetUpAi,
  onTakeTour,
}: HomeFirstRunProps) {
  const { definition, enabled } = useArtwork("empty.library");
  const image = enabled ? definition.image : undefined;
  return (
    <section
      aria-labelledby="home-first-run-heading"
      data-testid="home-first-run"
      className="tw:grid tw:min-h-[min(36rem,70dvh)] tw:content-center tw:justify-items-center tw:gap-5 tw:px-4 tw:py-10 tw:text-center tw:compact:py-8"
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
      <h2
        id="home-first-run-heading"
        className="tw:m-0 tw:text-title tw:leading-tight tw:font-semibold tw:tracking-tight tw:compact:text-section"
      >
        {hasArchivedDecks ? "Nothing to study yet" : "Your library is empty"}
      </h2>
      <div className="tw:grid tw:justify-items-center tw:gap-3">
        <Button variant="primary" size="large" className="tw:min-w-52" onClick={onCreateSubject}>
          <Icon name="plus" size={18} /> Create a subject
        </Button>
        <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-center tw:gap-3">
          <Button loading={importing} onClick={onImportDocument}>
            <Icon name="upload" size={18} /> Import a document
          </Button>
          {hasArchivedDecks ? (
            <Button variant="text" onClick={onOpenLibrary}>
              Open Library
            </Button>
          ) : null}
        </div>
      </div>
      <div className="tw:mt-4 tw:w-full tw:max-w-5xl">
        <HomeGettingStarted onSetUpAi={onSetUpAi} onImportDeck={onImportDeck} onTakeTour={onTakeTour} />
      </div>
    </section>
  );
}
