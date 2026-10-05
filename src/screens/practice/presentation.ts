// Practice-specific presentation; review grades remain domain values.
export const practiceStyles = {
  practiceSideLabel: "tw:mb-3 tw:block tw:font-sans tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground",
  practiceAnswerLabel: "tw:text-accent-foreground",
  // A revealed animated card keeps its turn-over button on the label's row, clear of the content.
  practiceSideLabelBesideToggle: "tw:flex tw:min-h-7 tw:items-center tw:coarse:min-h-touch",
  practiceAnswerDivider: "tw:my-5 tw:h-px tw:bg-border",
  practiceSessionHeader: "tw:mb-3 tw:flex tw:min-w-0 tw:shrink-0 tw:flex-wrap tw:items-start tw:justify-between tw:gap-3",
  practiceSessionTitle: "tw:min-w-0 tw:flex-1 tw:[&_h1]:mt-0.5 tw:[&_h1]:mb-0 tw:[&_h1]:break-words tw:[&_h1]:text-section tw:[&_h1]:font-semibold tw:[&_h1]:tracking-tight",
  eyebrow: "tw:text-caption tw:font-medium tw:uppercase tw:tracking-wide tw:text-muted-foreground",
  practiceHeaderActions: "tw:flex tw:min-w-0 tw:shrink-0 tw:flex-wrap tw:items-center tw:justify-end tw:gap-1.5 tw:compact:w-full tw:compact:justify-start",
  practiceHeaderButton: "tw:whitespace-nowrap",
  practiceProgressTrack: "tw:mb-3 tw:shrink-0",
  practicePrimaryAction: "tw:flex tw:shrink-0 tw:justify-center tw:pt-3 tw:[&>button]:min-w-40",
  reviewRatingPanelConfidenceRatingPanel: "tw:mt-3 tw:min-w-0 tw:shrink-0 tw:rounded-lg tw:bg-muted/30 tw:p-3 tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50",
  confidenceRatingHeading: "tw:text-center tw:[&_h2]:m-0 tw:[&_h2]:text-small tw:[&_h2]:font-medium tw:[&_p]:mt-1 tw:[&_p]:mb-0 tw:[&_p]:text-small tw:[&_p]:text-muted-foreground",
  confidenceScoreActions: "tw:mt-3 tw:grid tw:min-w-0 tw:grid-cols-4 tw:gap-2 tw:max-[600px]:grid-cols-2",
  confidenceScoreNumber: "tw:grid tw:size-7 tw:shrink-0 tw:place-items-center tw:rounded-md tw:bg-foreground/10 tw:text-small tw:font-medium",
  confidenceScoreCopy: "tw:min-w-0 tw:text-small tw:font-medium",
  // The flashcard: the card font, sized to its content above a modest minimum. An animated card
  // draws its frame on each face so the frame turns over with it.
  cardSurface: "tw:min-h-40 tw:min-w-0 tw:text-card-foreground tw:font-(family-name:--card-font-family) tw:text-(length:--card-base-size) tw:compact:min-h-32",
  cardFrame: "tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:shadow-sm tw:compact:shadow-none",
  cardFace: "tw:px-8 tw:py-7 tw:compact:p-5",
  cardSideToggle: "tw:absolute tw:top-7 tw:right-5 tw:z-(--ostudy-layer-raised) tw:text-muted-foreground tw:enabled:hover:text-foreground tw:compact:top-5 tw:compact:right-2.5",
  confidenceButton: "tw:h-auto tw:min-h-12 tw:min-w-0 tw:rounded-lg tw:border-transparent tw:px-3 tw:py-2.5 tw:text-(--ostudy-color-confidence-on) tw:enabled:hover:text-(--ostudy-color-confidence-on) tw:enabled:hover:brightness-105 tw:coarse:min-h-touch",
  confidenceButtonContent: "tw:flex tw:w-full tw:items-center tw:justify-center tw:gap-2.5",
} as const;

export const completionStyles = {
  screen: "tw:grid tw:content-start tw:gap-5",
  hero: "tw:grid tw:min-w-0 tw:grid-cols-1 tw:items-center tw:gap-5 tw:p-5 tw:lg:grid-cols-[minmax(0,1fr)_minmax(0,2fr)] tw:lg:p-6",
  artworkWrap: "tw:grid tw:min-w-0 tw:place-items-center tw:[&_[data-artwork-slot]]:[--ostudy-artwork-max:13rem]",
  heroCopy: "tw:grid tw:min-w-0 tw:justify-items-start tw:gap-2 tw:[&_h1]:m-0 tw:[&_h1]:text-title tw:[&_h1]:font-semibold tw:[&_h1]:tracking-tight tw:[&_p]:m-0 tw:[&_p]:max-w-prose tw:[&_p]:leading-relaxed tw:[&_p]:text-muted-foreground",
  completionMark: "tw:grid tw:size-9 tw:place-items-center tw:rounded-lg tw:bg-success-soft tw:text-success",
  eyebrow: "tw:text-caption tw:font-medium tw:uppercase tw:tracking-wide tw:text-muted-foreground",
  analytics: "tw:grid tw:min-w-0 tw:gap-3",
  distributionCard: "tw:grid tw:min-w-0 tw:items-center tw:gap-4 tw:p-4 tw:lg:grid-cols-[minmax(0,1fr)_minmax(0,2fr)]",
  actions: "tw:flex tw:flex-wrap tw:items-center tw:justify-center tw:gap-2 tw:max-sm:[&>button]:w-full",
} as const;

export const scoreColors = {
  0: "tw:bg-(--ostudy-color-confidence-0) tw:enabled:hover:bg-(--ostudy-color-confidence-0)",
  1: "tw:bg-(--ostudy-color-confidence-1) tw:enabled:hover:bg-(--ostudy-color-confidence-1)",
  2: "tw:bg-(--ostudy-color-confidence-2) tw:enabled:hover:bg-(--ostudy-color-confidence-2)",
  3: "tw:bg-(--ostudy-color-confidence-3) tw:enabled:hover:bg-(--ostudy-color-confidence-3)",
} as const;
