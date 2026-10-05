// Practice-specific presentation; review grades remain domain values.
export const practiceStyles = {
  practiceSideLabel: "tw:mb-3 tw:block tw:font-sans tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground",
  practiceAnswerLabel: "tw:text-accent-foreground",
  // A revealed animated card keeps its turn-over button on the label's row, clear of the content.
  practiceSideLabelBesideToggle: "tw:flex tw:min-h-7 tw:items-center tw:coarse:min-h-touch",
  practiceAnswerDivider: "tw:my-5 tw:h-px tw:bg-border",
  // The page fills the main area as a column: a top bar that stays put, the card, then the actions.
  page: "tw:flex tw:min-h-full tw:flex-col",
  // The top bar follows the chapter reader's: the deck as a one-line title over a muted line with
  // the kind of practice and the counts, quiet actions on the right, and progress as its bottom edge.
  topBar: "tw:sticky tw:top-0 tw:z-(--ostudy-layer-sticky) tw:shrink-0 tw:bg-background",
  topBarRow: "tw:flex tw:min-h-14 tw:min-w-0 tw:items-center tw:gap-2 tw:py-1.5 tw:px-page tw:compact:gap-1 tw:compact:pr-2 tw:compact:pl-4",
  topBarTitle: "tw:min-w-0 tw:flex-1",
  title: "tw:m-0 tw:truncate tw:text-body tw:font-semibold",
  subtitle: "tw:m-0 tw:truncate tw:text-label tw:tabular-nums tw:text-muted-foreground",
  topBarActions: "tw:flex tw:shrink-0 tw:items-center tw:gap-1",
  quietIconButton: "tw:border-transparent tw:bg-transparent",
  endAction: "tw:ml-1 tw:whitespace-nowrap",
  // The card column: as wide as comfortable reading, centered under the top bar. On a phone it
  // takes the free height, which keeps the actions at the bottom of the screen.
  body: "tw:px-page tw:pt-6 tw:pb-6 tw:compact:flex-1 tw:compact:px-4 tw:compact:pt-4",
  column: "tw:mx-auto tw:grid tw:w-full tw:max-w-[45rem] tw:min-w-0 tw:gap-4",
  // The reveal button and the ratings keep one place at the bottom of the view: a long card scrolls
  // under them, behind a short fade, and on a phone they sit just above the bottom navigation.
  actionArea: "tw:sticky tw:bottom-0 tw:z-(--ostudy-layer-sticky) tw:shrink-0 tw:bg-background tw:px-page tw:pt-3 tw:pb-[max(1.5rem,var(--safe-bottom))] tw:compact:px-4 tw:compact:pb-3 tw:before:pointer-events-none tw:before:absolute tw:before:inset-x-0 tw:before:bottom-full tw:before:h-6 tw:before:bg-linear-to-t tw:before:from-background tw:before:to-transparent",
  primaryAction: "tw:h-12 tw:min-w-56 tw:justify-self-center tw:text-body tw:compact:w-full tw:compact:justify-self-stretch",
  actionSlot: "tw:grid tw:min-w-0",
  // The four ratings are one group: its question, then one row of four that a phone stacks inside
  // each button so the names keep their full width.
  ratingGroup: "tw:grid tw:min-w-0 tw:gap-3 tw:rounded-lg tw:outline-none tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:compact:gap-2",
  ratingHeading: "tw:m-0 tw:text-center tw:text-small tw:font-medium tw:text-muted-foreground",
  ratingNote: "tw:m-0 tw:text-center tw:text-small tw:text-muted-foreground",
  ratingRetry: "tw:justify-self-center",
  ratingButtons: "tw:grid tw:min-w-0 tw:grid-cols-4 tw:gap-2",
  ratingButton: "tw:h-12 tw:min-w-0 tw:gap-2 tw:border-(--confidence-tone)/40 tw:bg-(--confidence-tone)/10 tw:px-2 tw:text-small tw:text-foreground tw:enabled:hover:bg-(--confidence-tone)/20 tw:compact:h-auto tw:compact:min-h-14 tw:compact:flex-col tw:compact:gap-1 tw:compact:rounded-lg tw:compact:py-2",
  ratingKey: "tw:grid tw:size-6 tw:shrink-0 tw:place-items-center tw:rounded-full tw:bg-(--confidence-tone) tw:text-caption tw:font-semibold tw:tabular-nums tw:text-(--ostudy-color-confidence-on)",
  ratingName: "tw:min-w-0 tw:truncate",
  // The flashcard: the card font, sized to its content above a modest minimum. An animated card
  // draws its frame on each face so the frame turns over with it.
  cardSurface: "tw:min-h-40 tw:min-w-0 tw:text-card-foreground tw:font-(family-name:--card-font-family) tw:text-(length:--card-base-size) tw:compact:min-h-32",
  cardFrame: "tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:shadow-sm tw:compact:shadow-none",
  cardFace: "tw:px-8 tw:py-7 tw:compact:p-5",
  // Question cards share the flashcard's frame and spacing, a little tighter on a phone so their
  // tiles keep room.
  questionCard: "tw:px-8 tw:py-7 tw:compact:p-4",
  cardSideToggle: "tw:absolute tw:top-7 tw:right-5 tw:z-(--ostudy-layer-raised) tw:text-muted-foreground tw:enabled:hover:text-foreground tw:compact:top-5 tw:compact:right-2.5",
} as const;

export const scoreTones = {
  0: "tw:[--confidence-tone:var(--ostudy-color-confidence-0)]",
  1: "tw:[--confidence-tone:var(--ostudy-color-confidence-1)]",
  2: "tw:[--confidence-tone:var(--ostudy-color-confidence-2)]",
  3: "tw:[--confidence-tone:var(--ostudy-color-confidence-3)]",
} as const;
