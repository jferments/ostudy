// Static semantic presentation recipes; data, source identities and async jobs stay with their owners.
export const documentStyles = {
  "screen": "tw:min-w-0",
  "toolbar": "tw:mb-3 tw:flex tw:items-center tw:justify-between tw:gap-3",
  "count": "tw:text-small tw:text-muted-foreground tw:tabular-nums",
  "list": "tw:grid tw:min-w-0 tw:gap-2",
  "card": "tw:flex tw:min-w-0 tw:flex-col tw:gap-3 tw:p-4",
  "cardMain": "tw:min-w-0 tw:flex-1",
  "titleRow": "tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2 tw:[&_h2]:m-0 tw:[&_h2]:break-words tw:[&_h2]:text-body tw:[&_h2]:font-medium",
  "processingPhase": "tw:text-small tw:text-muted-foreground",
  "metadataButton": "tw:ml-auto tw:shrink-0",
  "sourceSummary": "tw:mt-1.5 tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2 tw:text-small tw:text-muted-foreground",
  "filename": "tw:min-w-0 tw:break-all",
  "cardCount": "tw:shrink-0 tw:font-medium tw:tabular-nums tw:text-foreground",
  "actions": "tw:border-0 tw:border-t tw:border-solid tw:border-border/70 tw:pt-3",
  "deckNote": "tw:mt-3 tw:mb-0 tw:text-small tw:leading-relaxed tw:text-muted-foreground"
} as const;

export const providerStyles = {
  "section": "tw:min-w-0",
  "providerPanel": "tw:mb-5 tw:flex tw:min-w-0 tw:items-center tw:gap-3 tw:p-4",
  "icon": "tw:grid tw:size-8 tw:shrink-0 tw:place-items-center tw:rounded-lg tw:bg-muted tw:text-muted-foreground",
  "providerCopy": "tw:grid tw:min-w-0 tw:flex-1 tw:gap-0.5 tw:[&_strong]:break-words tw:[&_strong]:text-small tw:[&_small]:text-small tw:[&_small]:leading-relaxed tw:[&_small]:text-muted-foreground",
  "label": "tw:m-0 tw:text-caption tw:font-medium tw:text-muted-foreground",
  "providerActions": "tw:shrink-0"
} as const;

export const guideLibraryStyles = {
  "workspace": "tw:p-4 tw:sm:p-5",
  "toolbar": "tw:mb-4 tw:flex tw:min-w-0 tw:flex-wrap tw:items-end tw:justify-between tw:gap-4",
  "search": "tw:max-sm:w-full tw:max-sm:flex-[0_1_auto] tw:max-sm:[&>label]:w-full tw:max-sm:[&>label]:max-w-none tw:max-sm:[&>label]:flex-[0_1_auto] tw:flex tw:min-w-0 tw:flex-1 tw:flex-wrap tw:items-end tw:gap-2 tw:[&>label]:grid tw:[&>label]:min-w-0 tw:[&>label]:flex-1 tw:[&>label]:gap-1.5 tw:[&>label]:text-small tw:[&>label]:font-medium",
  "tools": "tw:flex tw:min-w-0 tw:flex-wrap tw:items-end tw:gap-2 tw:[&>label]:grid tw:[&>label]:gap-1.5 tw:[&>label]:text-small tw:[&>label]:font-medium",
  "summary": "tw:mb-3 tw:flex tw:flex-wrap tw:justify-between tw:gap-2 tw:text-small tw:text-muted-foreground",
  "list": "tw:grid tw:min-w-0 tw:gap-1",
  "row": "tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-3 tw:rounded-lg tw:px-2 tw:py-2 tw:transition-colors tw:hover:bg-muted/40 tw:reduce-motion:transition-none",
  "rowMain": "tw:h-auto tw:min-w-0 tw:flex-1 tw:justify-start tw:whitespace-normal tw:rounded-lg tw:border-0 tw:bg-transparent tw:p-1 tw:text-left tw:font-normal tw:shadow-none",
  "rowIcon": "tw:grid tw:size-8 tw:shrink-0 tw:place-items-center tw:rounded-lg tw:bg-muted tw:text-muted-foreground",
  "rowCopy": "tw:grid tw:min-w-0 tw:gap-1 tw:[&>strong]:break-words tw:[&>strong]:text-small tw:[&>strong]:font-medium tw:[&>small]:break-all tw:[&>small]:text-caption tw:[&>small]:text-muted-foreground",
  "metrics": "tw:flex tw:flex-wrap tw:gap-x-3 tw:gap-y-1 tw:text-caption tw:tabular-nums tw:text-muted-foreground",
  "actions": "tw:shrink-0",
  "pagination": "tw:mt-4 tw:flex tw:flex-wrap tw:items-center tw:justify-end tw:gap-2 tw:text-small tw:tabular-nums tw:text-muted-foreground"
} as const;

export const guideReaderStyles = {
  "workspace": "tw:min-w-0",
  "header": "tw:mb-5 tw:flex tw:min-w-0 tw:flex-wrap tw:items-start tw:justify-between tw:gap-4 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-card tw:pb-4",
  "eyebrow": "tw:mt-0 tw:mb-1 tw:text-caption tw:font-medium tw:uppercase tw:tracking-wide tw:text-muted-foreground",
  "headerActions": "tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2",
  "annotationToggle": "tw:flex tw:min-h-9 tw:items-center tw:gap-2 tw:text-small tw:coarse:min-h-touch",
  "generationButton": "tw:border-destructive/30 tw:bg-destructive-soft tw:text-destructive",
  "stopSign": "tw:grid tw:size-5 tw:place-items-center tw:text-destructive",
  "layout": "tw:grid tw:min-w-0 tw:grid-cols-1 tw:items-start tw:gap-7 tw:xl:grid-cols-[14rem_minmax(0,1fr)]",
  "contents": "tw:grid tw:min-w-0 tw:gap-0.5 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-2 tw:text-small tw:xl:sticky tw:xl:top-4 tw:xl:max-h-[calc(100dvh-8rem)] tw:xl:overflow-y-auto tw:[&>strong]:mb-2 tw:[&>strong]:px-2 tw:[&_a]:rounded-md tw:[&_a]:px-2 tw:[&_a]:py-1.5 tw:[&_a]:text-muted-foreground tw:[&_a]:no-underline tw:[&_a:hover]:bg-muted tw:[&_a:hover]:text-foreground tw:[&_a:hover]:underline tw:[&_a:focus-visible]:outline-2 tw:[&_a:focus-visible]:outline-ring",
  "guide": "tw:grid tw:min-w-0 tw:gap-10",
  "guideTitle": "tw:m-0 tw:text-title tw:font-semibold tw:tracking-tight tw:text-foreground",
  "chapter": "tw:grid tw:min-w-0 tw:scroll-mt-6 tw:gap-7",
  "chapterHeader": "tw:grid tw:min-w-0 tw:gap-2 tw:border-0 tw:border-b tw:border-solid tw:border-primary/25 tw:pb-4 tw:[&_h2]:m-0 tw:[&_h2]:text-section tw:[&_h2]:font-semibold tw:[&_h2]:tracking-tight tw:[&_h2]:text-primary",
  "chapterIntro": "tw:max-w-[72ch] tw:text-body tw:leading-7 tw:text-foreground tw:[&_p]:my-2 tw:[&_p:first-child]:mt-0 tw:[&_p:last-child]:mb-0",
  "chapterSections": "tw:grid tw:min-w-0 tw:gap-9",
  "overview": "tw:min-w-0 tw:scroll-mt-6 tw:rounded-xl tw:bg-accent/20 tw:p-5 tw:ring-1 tw:ring-primary/10 tw:[&_h2]:mt-0 tw:[&_h2]:text-section tw:[&_h2]:font-semibold tw:[&_h2]:tracking-tight tw:[&_h2]:text-primary tw:[&_h3]:mt-6 tw:[&_h3]:mb-2 tw:[&_h3]:text-body-large tw:[&_h3]:font-semibold tw:[&_p]:leading-7 tw:[&_li]:my-2",
  "section": "tw:min-w-0 tw:scroll-mt-6 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:pt-9 tw:first:border-t-0 tw:first:pt-0",
  "sectionHeader": "tw:mb-5 tw:flex tw:min-w-0 tw:items-start tw:justify-between tw:gap-3 tw:[&_h2]:m-0 tw:[&_h2]:text-title-small tw:[&_h2]:font-semibold tw:[&_h2]:tracking-tight tw:[&_h2]:text-primary tw:[&_h3]:m-0 tw:[&_h3]:text-title-small tw:[&_h3]:font-semibold tw:[&_h3]:tracking-tight tw:[&_h3]:text-primary",
  "sectionHeading": "tw:min-w-0 tw:flex-1",
  "sourceLabel": "tw:mt-0 tw:mb-1 tw:text-caption tw:font-medium tw:text-muted-foreground",
  "sectionMenu": "tw:mt-0.5 tw:shrink-0",
  "structuredSection": "tw:min-w-0 tw:break-words tw:text-body tw:leading-7 tw:[&>section]:min-w-0 tw:[&_h3]:mt-9 tw:[&_h3]:mb-3 tw:[&_h3]:text-body-large tw:[&_h3]:font-semibold tw:[&_h3]:tracking-tight tw:[&_h3]:text-primary tw:[&_h4]:mt-7 tw:[&_h4]:mb-3 tw:[&_h4]:text-body tw:[&_h4]:font-semibold tw:[&_h4]:tracking-tight tw:[&_h4]:text-primary tw:[&_h5]:mt-5 tw:[&_h5]:mb-2 tw:[&_h5]:text-body tw:[&_h5]:font-semibold tw:[&_p]:my-3 tw:[&_li]:my-2 tw:[&>p:first-child]:mt-0",
  "terms": "tw:grid tw:min-w-0 tw:gap-3 tw:[&>div]:min-w-0 tw:[&>div]:overflow-hidden tw:[&>div]:break-words tw:[&>div]:rounded-xl tw:[&>div]:bg-accent/20 tw:[&>div]:p-4 tw:[&>div]:ring-1 tw:[&>div]:ring-primary/10 tw:[&_dt]:font-semibold tw:[&_dt]:text-primary tw:[&_dd]:mt-1 tw:[&_dd]:ml-0 tw:[&_dd]:text-muted-foreground",
  "keyConcepts": "tw:grid tw:min-w-0 tw:grid-cols-1 tw:gap-3 tw:lg:grid-cols-2 tw:[&>div]:min-w-0 tw:[&>div]:rounded-xl tw:[&>div]:bg-card tw:[&>div]:p-4 tw:[&>div]:ring-1 tw:[&>div]:ring-border tw:[&_dt]:font-semibold tw:[&_dt]:text-primary tw:[&_dd]:mt-1 tw:[&_dd]:ml-0 tw:[&_dd]:leading-relaxed tw:[&_dd]:text-muted-foreground",
  "comparisons": "tw:grid tw:min-w-0 tw:max-w-full tw:grid-cols-1 tw:gap-3 tw:2xl:grid-cols-2 tw:[&>article]:min-w-0 tw:[&>article]:max-w-full tw:[&>article]:overflow-hidden tw:[&>article]:break-words tw:[&>article]:rounded-xl tw:[&>article]:bg-card tw:[&>article]:p-4 tw:[&>article]:ring-1 tw:[&>article]:ring-border tw:[&>article>strong]:mb-2 tw:[&>article>strong]:block tw:[&>article>strong]:text-primary",
  "callouts": "tw:my-6 tw:grid tw:min-w-0 tw:gap-3",
  "callout": "tw:min-w-0 tw:overflow-hidden tw:break-words tw:rounded-r-lg tw:border-0 tw:border-l-4 tw:border-solid tw:border-primary/45 tw:bg-accent/30 tw:px-4 tw:py-3 tw:[&>strong]:text-primary tw:[&_p]:mb-0",
  "tableSection": "tw:min-w-0 tw:max-w-full",
  "tableWrap": "tw:max-w-full tw:overflow-x-auto tw:rounded-lg tw:ring-1 tw:ring-border",
  "table": "tw:w-full tw:min-w-[32rem] tw:border-collapse tw:text-left tw:text-small tw:[&_th]:bg-muted/55 tw:[&_th]:px-3 tw:[&_th]:py-2 tw:[&_th]:font-semibold tw:[&_td]:border-0 tw:[&_td]:border-t tw:[&_td]:border-solid tw:[&_td]:border-border tw:[&_td]:px-3 tw:[&_td]:py-2 tw:[&_td]:align-top tw:[&_td_p]:m-0",
  "tableCaption": "tw:mt-2 tw:text-small tw:text-muted-foreground",
  "workedExamples": "tw:grid tw:min-w-0 tw:gap-5",
  "workedExample": "tw:min-w-0 tw:overflow-hidden tw:break-words tw:rounded-xl tw:bg-muted/30 tw:p-5 tw:ring-1 tw:ring-border tw:[&_h4]:mt-0 tw:[&_h4]:mb-2 tw:[&_h4]:text-body tw:[&_h4]:font-semibold tw:[&_h4]:text-primary tw:[&_h5]:mt-0 tw:[&_h5]:mb-2 tw:[&_h5]:text-body tw:[&_h5]:font-semibold tw:[&_h5]:text-primary",
  "exampleAnswer": "tw:mt-4 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:pt-3 tw:[&>strong]:text-primary tw:[&_p]:mb-0",
  "exampleTakeaway": "tw:mt-3 tw:rounded-lg tw:bg-accent/25 tw:px-3 tw:py-2 tw:text-muted-foreground tw:[&>strong]:text-primary tw:[&_p]:mb-0",
  "retrievalChecks": "tw:grid tw:min-w-0 tw:gap-3",
  "retrievalCheck": "tw:min-w-0 tw:max-w-full tw:overflow-hidden tw:rounded-xl tw:bg-accent/40 tw:ring-1 tw:ring-primary/20 tw:transition-colors tw:hover:bg-accent/50 tw:reduce-motion:transition-none tw:[&_[data-slot=collapsible-trigger]]:font-semibold tw:[&_[data-slot=collapsible-trigger]]:text-foreground",
  "retrievalAnswer": "tw:border-0 tw:border-t tw:border-solid tw:border-primary/15 tw:bg-card tw:px-4 tw:pt-3 tw:pb-4 tw:[&>strong]:text-primary tw:[&_p]:mb-0",
  "visualNote": "tw:my-5 tw:rounded-xl tw:bg-accent/45 tw:p-4 tw:text-foreground tw:ring-1 tw:ring-primary/15",
  "personalNote": "tw:my-5 tw:rounded-lg tw:bg-accent/40 tw:p-4 tw:text-body tw:ring-1 tw:ring-primary/20 tw:[&_p]:mb-0 tw:[&_p]:whitespace-pre-wrap tw:[&_p]:leading-7",
  "overrideText": "tw:min-w-0 tw:break-words tw:leading-7 tw:[&_p]:whitespace-pre-wrap",
  "editor": "tw:mt-6 tw:grid tw:min-w-0 tw:gap-4 tw:rounded-lg tw:bg-muted/30 tw:p-4 tw:ring-1 tw:ring-border tw:[&>label]:grid tw:[&>label]:gap-2 tw:[&>p]:m-0 tw:[&>p]:text-small tw:[&>p]:leading-relaxed tw:[&>p]:text-muted-foreground",
  "editorActions": "tw:mt-3 tw:flex tw:flex-wrap tw:items-center tw:gap-2"
} as const;

export const progressStyles = {
  "panel": "tw:mt-4 tw:grid tw:min-w-0 tw:gap-3 tw:rounded-lg tw:bg-muted/30 tw:p-3",
  "heading": "tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-2 tw:text-small tw:font-medium tw:tabular-nums",
  "detail": "tw:text-small tw:leading-relaxed tw:text-muted-foreground",
  "stats": "tw:m-0 tw:grid tw:grid-cols-2 tw:gap-3 tw:lg:grid-cols-4 tw:[&_dt]:mb-1 tw:[&_dt]:text-caption tw:[&_dt]:text-muted-foreground tw:[&_dd]:m-0 tw:[&_dd]:break-words tw:[&_dd]:text-small tw:[&_dd]:font-medium tw:[&_dd]:tabular-nums",
  "warning": "tw:text-warning"
} as const;
