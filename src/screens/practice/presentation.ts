// Practice-specific presentation; review grades remain domain values.
export const practiceStyles = {
  practiceSideLabel: "tw:mb-4 tw:block tw:font-sans tw:text-xs tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground",
  practiceFlipHint: "tw:mt-6 tw:block tw:text-center tw:font-sans tw:text-sm tw:text-muted-foreground",
  practiceAnswerDivider: "tw:my-6 tw:h-px tw:bg-border",
  practiceSessionHeader: "tw:mb-4 tw:flex tw:min-w-0 tw:shrink-0 tw:flex-wrap tw:items-start tw:justify-between tw:gap-4",
  practiceSessionTitle: "tw:min-w-0 tw:flex-1 tw:[&_h1]:mt-1 tw:[&_h1]:mb-0 tw:[&_h1]:break-words tw:[&_h1]:text-2xl tw:[&_h1]:font-semibold tw:[&_h1]:tracking-tight",
  eyebrow: "tw:text-xs tw:font-medium tw:uppercase tw:tracking-wide tw:text-muted-foreground",
  practiceHeaderActions: "tw:flex tw:min-w-0 tw:shrink-0 tw:flex-wrap tw:items-center tw:justify-end tw:gap-2 tw:max-[760px]:w-full tw:max-[760px]:justify-start",
  practiceHeaderButton: "tw:min-h-control tw:whitespace-nowrap",
  practiceProgressTrack: "tw:mb-4 tw:shrink-0",
  practicePrimaryAction: "tw:flex tw:shrink-0 tw:justify-center tw:pt-4 tw:[&>button]:min-w-48",
  reviewRatingPanelConfidenceRatingPanel: "tw:mt-4 tw:min-w-0 tw:shrink-0 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:p-4 tw:focus-visible:outline-2 tw:focus-visible:outline-offset-2 tw:focus-visible:outline-ring",
  confidenceRatingHeading: "tw:text-center tw:[&_h2]:m-0 tw:[&_h2]:text-base tw:[&_h2]:font-semibold tw:[&_p]:mt-1 tw:[&_p]:mb-0 tw:[&_p]:text-sm tw:[&_p]:text-muted-foreground",
  confidenceScoreActions: "tw:mt-3 tw:grid tw:min-w-0 tw:grid-cols-4 tw:gap-3 tw:max-[600px]:grid-cols-2",
  confidenceScoreNumber: "tw:grid tw:size-8 tw:shrink-0 tw:place-items-center tw:rounded-lg tw:border tw:border-solid tw:border-current/25 tw:text-base tw:font-semibold",
  confidenceScoreCopy: "tw:min-w-0 tw:text-sm tw:font-semibold",
  cardSurface: "tw:relative tw:min-w-0 tw:flex-1 tw:overflow-auto tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:p-6 tw:shadow-panel tw:font-(family-name:--card-font-family) tw:text-(length:--card-base-size) tw:sm:p-8 tw:focus-visible:outline-2 tw:focus-visible:outline-offset-2 tw:focus-visible:outline-ring",
  confidenceButton: "tw:h-auto tw:min-h-14 tw:min-w-0 tw:rounded-xl tw:border-transparent tw:px-3 tw:py-3 tw:text-(--ostudy-color-confidence-on) tw:enabled:hover:text-(--ostudy-color-confidence-on) tw:enabled:hover:brightness-110",
  confidenceButtonContent: "tw:flex tw:w-full tw:items-center tw:justify-center tw:gap-3"
} as const;
export const completionStyles = {
  screen: "tw:grid tw:content-start tw:gap-6",
  hero: "tw:grid tw:min-w-0 tw:grid-cols-1 tw:items-center tw:gap-6 tw:p-6 tw:lg:grid-cols-[minmax(0,1fr)_minmax(0,2fr)] tw:lg:p-8",
  artworkWrap: "tw:grid tw:min-w-0 tw:place-items-center tw:[&_[data-artwork-slot]]:[--ostudy-artwork-max:15rem]",
  heroCopy: "tw:grid tw:min-w-0 tw:justify-items-start tw:gap-3 tw:[&_h1]:m-0 tw:[&_h1]:text-3xl tw:[&_h1]:font-semibold tw:[&_h1]:tracking-tight tw:[&_p]:m-0 tw:[&_p]:max-w-prose tw:[&_p]:leading-relaxed tw:[&_p]:text-muted-foreground",
  completionMark: "tw:grid tw:size-11 tw:place-items-center tw:rounded-xl tw:border tw:border-solid tw:border-success/30 tw:bg-success-soft tw:text-success",
  eyebrow: "tw:text-xs tw:font-medium tw:uppercase tw:tracking-wide tw:text-muted-foreground",
  analytics: "tw:grid tw:min-w-0 tw:gap-4",
  sectionHeading: "tw:flex tw:flex-wrap tw:items-end tw:justify-between tw:gap-3 tw:[&_h2]:mt-2 tw:[&_h2]:mb-0 tw:[&_h2]:text-xl tw:[&_h2]:font-semibold",
  distributionCard: "tw:grid tw:min-w-0 tw:items-center tw:gap-5 tw:p-5 tw:lg:grid-cols-[minmax(0,1fr)_minmax(0,2fr)] tw:[&_h3]:m-0 tw:[&_h3]:text-base tw:[&_h3]:font-semibold tw:[&_p]:mt-2 tw:[&_p]:mb-0 tw:[&_p]:text-sm tw:[&_p]:leading-relaxed tw:[&_p]:text-muted-foreground",
  actions: "tw:flex tw:flex-wrap tw:items-center tw:justify-center tw:gap-3 tw:max-sm:[&>button]:w-full"
} as const;
export const scoreColors = {
  0: "tw:bg-(--ostudy-color-confidence-0) tw:enabled:hover:bg-(--ostudy-color-confidence-0)",
  1: "tw:bg-(--ostudy-color-confidence-1) tw:enabled:hover:bg-(--ostudy-color-confidence-1)",
  2: "tw:bg-(--ostudy-color-confidence-2) tw:enabled:hover:bg-(--ostudy-color-confidence-2)",
  3: "tw:bg-(--ostudy-color-confidence-3) tw:enabled:hover:bg-(--ostudy-color-confidence-3)"
} as const;
