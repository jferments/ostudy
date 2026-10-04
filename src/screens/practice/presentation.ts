// Practice-specific presentation; review grades remain domain values.
export const practiceStyles = {
  practiceSideLabel: "tw:mb-3 tw:block tw:font-sans tw:text-caption tw:font-medium tw:uppercase tw:tracking-wide tw:text-muted-foreground",
  practiceFlipHint: "tw:mt-5 tw:block tw:text-center tw:font-sans tw:text-small tw:text-muted-foreground",
  practiceAnswerDivider: "tw:my-5 tw:h-px tw:bg-border",
  practiceSessionHeader: "tw:mb-3 tw:flex tw:min-w-0 tw:shrink-0 tw:flex-wrap tw:items-start tw:justify-between tw:gap-3",
  practiceSessionTitle: "tw:min-w-0 tw:flex-1 tw:[&_h1]:mt-0.5 tw:[&_h1]:mb-0 tw:[&_h1]:break-words tw:[&_h1]:text-section tw:[&_h1]:font-semibold tw:[&_h1]:tracking-tight",
  eyebrow: "tw:text-caption tw:font-medium tw:uppercase tw:tracking-wide tw:text-muted-foreground",
  practiceHeaderActions: "tw:flex tw:min-w-0 tw:shrink-0 tw:flex-wrap tw:items-center tw:justify-end tw:gap-1.5 tw:max-[760px]:w-full tw:max-[760px]:justify-start",
  practiceHeaderButton: "tw:whitespace-nowrap",
  practiceProgressTrack: "tw:mb-3 tw:shrink-0",
  practicePrimaryAction: "tw:flex tw:shrink-0 tw:justify-center tw:pt-3 tw:[&>button]:min-w-40",
  reviewRatingPanelConfidenceRatingPanel: "tw:mt-3 tw:min-w-0 tw:shrink-0 tw:rounded-lg tw:bg-muted/30 tw:p-3 tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50",
  confidenceRatingHeading: "tw:text-center tw:[&_h2]:m-0 tw:[&_h2]:text-small tw:[&_h2]:font-medium tw:[&_p]:mt-1 tw:[&_p]:mb-0 tw:[&_p]:text-small tw:[&_p]:text-muted-foreground",
  confidenceScoreActions: "tw:mt-3 tw:grid tw:min-w-0 tw:grid-cols-4 tw:gap-2 tw:max-[600px]:grid-cols-2",
  confidenceScoreNumber: "tw:grid tw:size-7 tw:shrink-0 tw:place-items-center tw:rounded-md tw:bg-foreground/10 tw:text-small tw:font-medium",
  confidenceScoreCopy: "tw:min-w-0 tw:text-small tw:font-medium",
  cardSurface: "tw:relative tw:min-w-0 tw:flex-1 tw:overflow-auto tw:rounded-xl tw:bg-card tw:p-6 tw:ring-1 tw:ring-border tw:font-(family-name:--card-font-family) tw:text-(length:--card-base-size) tw:sm:p-8 tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50",
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
  sectionHeading: "tw:flex tw:flex-wrap tw:items-end tw:justify-between tw:gap-3 tw:[&_h2]:mt-1 tw:[&_h2]:mb-0 tw:[&_h2]:text-title-small tw:[&_h2]:font-semibold",
  distributionCard: "tw:grid tw:min-w-0 tw:items-center tw:gap-4 tw:p-4 tw:lg:grid-cols-[minmax(0,1fr)_minmax(0,2fr)] tw:[&_h3]:m-0 tw:[&_h3]:text-body tw:[&_h3]:font-medium tw:[&_p]:mt-1 tw:[&_p]:mb-0 tw:[&_p]:text-small tw:[&_p]:leading-relaxed tw:[&_p]:text-muted-foreground",
  actions: "tw:flex tw:flex-wrap tw:items-center tw:justify-center tw:gap-2 tw:max-sm:[&>button]:w-full",
} as const;

export const scoreColors = {
  0: "tw:bg-(--ostudy-color-confidence-0) tw:enabled:hover:bg-(--ostudy-color-confidence-0)",
  1: "tw:bg-(--ostudy-color-confidence-1) tw:enabled:hover:bg-(--ostudy-color-confidence-1)",
  2: "tw:bg-(--ostudy-color-confidence-2) tw:enabled:hover:bg-(--ostudy-color-confidence-2)",
  3: "tw:bg-(--ostudy-color-confidence-3) tw:enabled:hover:bg-(--ostudy-color-confidence-3)",
} as const;
