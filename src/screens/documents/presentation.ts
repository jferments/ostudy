// Static semantic presentation recipes; data, source identities and async jobs stay with their owners.
export const documentStyles = {
  "screen": "tw:min-w-0",
  "toolbar": "tw:mb-3 tw:flex tw:items-center tw:justify-between tw:gap-3",
  "count": "tw:text-sm tw:text-muted-foreground tw:tabular-nums",
  "list": "tw:grid tw:min-w-0 tw:gap-2",
  "card": "tw:flex tw:min-w-0 tw:flex-col tw:gap-3 tw:p-4",
  "cardMain": "tw:min-w-0 tw:flex-1",
  "titleRow": "tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2 tw:[&_h2]:m-0 tw:[&_h2]:break-words tw:[&_h2]:text-base tw:[&_h2]:font-medium",
  "processingPhase": "tw:text-sm tw:text-muted-foreground",
  "metadataButton": "tw:ml-auto tw:shrink-0",
  "sourceSummary": "tw:mt-1.5 tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2 tw:text-sm tw:text-muted-foreground",
  "filename": "tw:min-w-0 tw:break-all",
  "cardCount": "tw:shrink-0 tw:font-medium tw:tabular-nums tw:text-foreground",
  "actions": "tw:border-0 tw:border-t tw:border-solid tw:border-border/70 tw:pt-3",
  "deckNote": "tw:mt-3 tw:mb-0 tw:text-sm tw:leading-relaxed tw:text-muted-foreground"
} as const;

export const providerStyles = {
  "section": "tw:min-w-0",
  "providerPanel": "tw:mb-5 tw:flex tw:min-w-0 tw:items-center tw:gap-3 tw:p-4",
  "icon": "tw:grid tw:size-8 tw:shrink-0 tw:place-items-center tw:rounded-lg tw:bg-muted tw:text-muted-foreground",
  "providerCopy": "tw:grid tw:min-w-0 tw:flex-1 tw:gap-0.5 tw:[&_strong]:break-words tw:[&_strong]:text-sm tw:[&_small]:text-sm tw:[&_small]:leading-relaxed tw:[&_small]:text-muted-foreground",
  "label": "tw:m-0 tw:text-xs tw:font-medium tw:text-muted-foreground",
  "providerActions": "tw:shrink-0"
} as const;

export const guideLibraryStyles = {
  "workspace": "tw:p-4 tw:sm:p-5",
  "toolbar": "tw:mb-4 tw:flex tw:min-w-0 tw:flex-wrap tw:items-end tw:justify-between tw:gap-4",
  "search": "tw:max-sm:w-full tw:max-sm:flex-[0_1_auto] tw:max-sm:[&>label]:w-full tw:max-sm:[&>label]:max-w-none tw:max-sm:[&>label]:flex-[0_1_auto] tw:flex tw:min-w-0 tw:flex-1 tw:flex-wrap tw:items-end tw:gap-2 tw:[&>label]:grid tw:[&>label]:min-w-0 tw:[&>label]:flex-1 tw:[&>label]:gap-1.5 tw:[&>label]:text-sm tw:[&>label]:font-medium",
  "tools": "tw:flex tw:min-w-0 tw:flex-wrap tw:items-end tw:gap-2 tw:[&>label]:grid tw:[&>label]:gap-1.5 tw:[&>label]:text-sm tw:[&>label]:font-medium",
  "summary": "tw:mb-3 tw:flex tw:flex-wrap tw:justify-between tw:gap-2 tw:text-sm tw:text-muted-foreground",
  "list": "tw:grid tw:min-w-0 tw:gap-1",
  "row": "tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-3 tw:rounded-lg tw:px-2 tw:py-2 tw:transition-colors tw:hover:bg-muted/40 tw:reduce-motion:transition-none",
  "rowMain": "tw:h-auto tw:min-w-0 tw:flex-1 tw:justify-start tw:whitespace-normal tw:rounded-lg tw:border-0 tw:bg-transparent tw:p-1 tw:text-left tw:font-normal tw:shadow-none",
  "rowIcon": "tw:grid tw:size-8 tw:shrink-0 tw:place-items-center tw:rounded-lg tw:bg-muted tw:text-muted-foreground",
  "rowCopy": "tw:grid tw:min-w-0 tw:gap-1 tw:[&>strong]:break-words tw:[&>strong]:text-sm tw:[&>strong]:font-medium tw:[&>small]:break-all tw:[&>small]:text-xs tw:[&>small]:text-muted-foreground",
  "metrics": "tw:flex tw:flex-wrap tw:gap-x-3 tw:gap-y-1 tw:text-xs tw:tabular-nums tw:text-muted-foreground",
  "actions": "tw:shrink-0",
  "pagination": "tw:mt-4 tw:flex tw:flex-wrap tw:items-center tw:justify-end tw:gap-2 tw:text-sm tw:tabular-nums tw:text-muted-foreground"
} as const;

export const guideReaderStyles = {
  "workspace": "tw:min-w-0",
  "header": "tw:mb-5 tw:flex tw:min-w-0 tw:flex-wrap tw:items-start tw:justify-between tw:gap-4 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-card tw:pb-4",
  "eyebrow": "tw:mt-0 tw:mb-1 tw:text-xs tw:font-medium tw:uppercase tw:tracking-wide tw:text-muted-foreground",
  "headerActions": "tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2",
  "annotationToggle": "tw:flex tw:min-h-9 tw:items-center tw:gap-2 tw:text-sm tw:coarse:min-h-touch",
  "generationButton": "tw:border-destructive/30 tw:bg-destructive-soft tw:text-destructive",
  "stopSign": "tw:grid tw:size-5 tw:place-items-center tw:text-destructive",
  "layout": "tw:grid tw:min-w-0 tw:grid-cols-1 tw:items-start tw:gap-7 tw:xl:grid-cols-[14rem_minmax(0,1fr)]",
  "contents": "tw:grid tw:min-w-0 tw:gap-0.5 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-2 tw:text-sm tw:xl:sticky tw:xl:top-4 tw:xl:max-h-[calc(100dvh-8rem)] tw:xl:overflow-y-auto tw:[&>strong]:mb-2 tw:[&>strong]:px-2 tw:[&_a]:rounded-md tw:[&_a]:px-2 tw:[&_a]:py-1.5 tw:[&_a]:text-muted-foreground tw:[&_a]:no-underline tw:[&_a:hover]:bg-muted tw:[&_a:hover]:text-foreground tw:[&_a:hover]:underline tw:[&_a:focus-visible]:outline-2 tw:[&_a:focus-visible]:outline-ring",
  "guide": "tw:grid tw:min-w-0 tw:gap-8",
  "overview": "tw:min-w-0 tw:scroll-mt-6 tw:pb-2 tw:[&_h2]:mt-0 tw:[&_h2]:text-2xl tw:[&_h3]:text-lg tw:[&_p]:leading-7 tw:[&_li]:my-2",
  "section": "tw:min-w-0 tw:scroll-mt-6 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:pt-7 tw:first:border-t-0 tw:first:pt-0",
  "sectionHeader": "tw:mb-4 tw:grid tw:min-w-0 tw:gap-3 tw:[&_h2]:m-0 tw:[&_h2]:text-xl tw:[&_h2]:font-semibold",
  "sourceLabel": "tw:mt-0 tw:mb-1 tw:text-xs tw:font-medium tw:text-muted-foreground",
  "sectionActions": "tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2",
  "structuredSection": "tw:min-w-0 tw:break-words tw:text-body tw:leading-7 tw:[&_h3]:mt-6 tw:[&_h3]:mb-2 tw:[&_h3]:text-lg tw:[&_p]:my-3 tw:[&_li]:my-2 tw:[&>p:first-child]:mt-0",
  "terms": "tw:grid tw:gap-2 tw:[&>div]:rounded-lg tw:[&>div]:bg-muted/35 tw:[&>div]:p-3 tw:[&_dt]:font-medium tw:[&_dd]:mt-1 tw:[&_dd]:ml-0 tw:[&_dd]:text-muted-foreground",
  "comparisons": "tw:grid tw:gap-3 tw:md:grid-cols-2 tw:[&>article]:rounded-lg tw:[&>article]:ring-1 tw:[&>article]:ring-border tw:[&>article]:p-4",
  "visualNote": "tw:my-4 tw:rounded-lg tw:bg-accent/50 tw:p-4 tw:text-foreground",
  "personalNote": "tw:my-5 tw:rounded-lg tw:bg-accent/40 tw:p-4 tw:text-body tw:ring-1 tw:ring-primary/20 tw:[&_p]:mb-0 tw:[&_p]:whitespace-pre-wrap tw:[&_p]:leading-7",
  "overrideText": "tw:min-w-0 tw:break-words tw:leading-7 tw:[&_p]:whitespace-pre-wrap",
  "editor": "tw:mt-6 tw:grid tw:min-w-0 tw:gap-4 tw:rounded-lg tw:bg-muted/30 tw:p-4 tw:ring-1 tw:ring-border tw:[&>label]:grid tw:[&>label]:gap-2 tw:[&>p]:m-0 tw:[&>p]:text-sm tw:[&>p]:leading-relaxed tw:[&>p]:text-muted-foreground",
  "editorActions": "tw:mt-3 tw:flex tw:flex-wrap tw:items-center tw:gap-2"
} as const;

export const progressStyles = {
  "panel": "tw:mt-4 tw:grid tw:min-w-0 tw:gap-3 tw:rounded-lg tw:bg-muted/30 tw:p-3",
  "heading": "tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-2 tw:text-sm tw:font-medium tw:tabular-nums",
  "detail": "tw:text-sm tw:leading-relaxed tw:text-muted-foreground",
  "stats": "tw:m-0 tw:grid tw:grid-cols-2 tw:gap-3 tw:lg:grid-cols-4 tw:[&_dt]:mb-1 tw:[&_dt]:text-xs tw:[&_dt]:text-muted-foreground tw:[&_dd]:m-0 tw:[&_dd]:break-words tw:[&_dd]:text-sm tw:[&_dd]:font-medium tw:[&_dd]:tabular-nums",
  "warning": "tw:text-warning"
} as const;
