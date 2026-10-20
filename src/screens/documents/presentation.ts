// Static semantic presentation recipes; data, source identities and async jobs stay with their owners.
export const providerStyles = {
  "section": "tw:min-w-0"
} as const;

export const guideLibraryStyles = {
  "workspace": "tw:min-w-0",
  "toolbar": "tw:flex tw:min-w-0 tw:flex-[1_1_24rem] tw:flex-wrap tw:items-center tw:justify-end tw:gap-2",
  "search": "tw:relative tw:flex tw:min-w-0 tw:flex-[1_1_14rem] tw:items-center tw:sm:max-w-[24rem]",
  "summary": "tw:mt-0 tw:mb-3 tw:text-small tw:text-muted-foreground",
  "grid": "tw:m-0 tw:grid tw:list-none tw:grid-cols-[repeat(auto-fill,minmax(min(100%,20rem),1fr))] tw:gap-5 tw:p-0",
  "card": "tw:mr-1.5 tw:mb-1.5 tw:min-w-0",
  "stack": "tw:absolute tw:inset-0 tw:translate-x-1.5 tw:translate-y-1.5 tw:rounded-xl tw:border tw:border-solid tw:border-primary/30 tw:bg-primary/12",
  "cardSurface": "tw:relative tw:flex tw:h-full tw:min-w-0 tw:flex-col tw:gap-4 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:p-4 tw:shadow-sm tw:compact:p-3",
  "collectionSurface": "tw:border-primary/30",
  "cover": "tw:grid tw:h-14 tw:w-11 tw:shrink-0 tw:place-items-center tw:rounded-l-sm tw:rounded-r-lg tw:border-0 tw:border-l-4 tw:border-solid tw:border-primary tw:bg-primary/12 tw:text-primary",
  "metrics": "tw:flex tw:flex-wrap tw:gap-x-3 tw:gap-y-1 tw:text-small tw:tabular-nums tw:text-muted-foreground",
  "pagination": "tw:mt-4 tw:flex tw:flex-wrap tw:items-center tw:justify-end tw:gap-2 tw:text-small tw:tabular-nums tw:text-muted-foreground"
} as const;

export const guideReaderStyles = {
  "workspace": "tw:min-w-0",
  "header": "tw:mb-5 tw:flex tw:min-w-0 tw:flex-wrap tw:items-start tw:justify-between tw:gap-4 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-card tw:pb-4",
  "eyebrow": "tw:mt-0 tw:mb-1 tw:text-caption tw:font-medium tw:uppercase tw:tracking-wide tw:text-muted-foreground",
  "headerActions": "tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2",
  "layout": "tw:grid tw:min-w-0 tw:grid-cols-1 tw:items-start tw:gap-7 tw:xl:grid-cols-[14rem_minmax(0,1fr)]",
  "contents": "tw:grid tw:min-w-0 tw:gap-0.5 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-2 tw:text-small tw:xl:sticky tw:xl:top-4 tw:xl:max-h-[calc(100dvh-8rem)] tw:xl:overflow-y-auto tw:[&>strong]:mb-2 tw:[&>strong]:px-2 tw:[&_a]:rounded-md tw:[&_a]:px-2 tw:[&_a]:py-1.5 tw:[&_a]:text-muted-foreground tw:[&_a]:no-underline tw:[&_a:hover]:bg-muted tw:[&_a:hover]:text-foreground tw:[&_a:hover]:underline tw:[&_a:focus-visible]:outline-2 tw:[&_a:focus-visible]:outline-ring",
  "guide": "tw:grid tw:min-w-0 tw:gap-10",
  "overview": "tw:min-w-0 tw:scroll-mt-6 tw:rounded-xl tw:bg-accent/20 tw:p-5 tw:ring-1 tw:ring-primary/10 tw:[&_h2]:mt-0 tw:[&_h2]:text-section tw:[&_h2]:font-semibold tw:[&_h2]:tracking-tight tw:[&_h2]:text-primary tw:[&_h3]:mt-6 tw:[&_h3]:mb-2 tw:[&_h3]:text-body-large tw:[&_h3]:font-semibold tw:[&_p]:leading-7 tw:[&_li]:my-2",
  "section": "tw:min-w-0 tw:scroll-mt-6 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:pt-9 tw:first:border-t-0 tw:first:pt-0",
  "sectionHeader": "tw:mb-5 tw:flex tw:min-w-0 tw:items-start tw:justify-between tw:gap-3 tw:[&_h2]:m-0 tw:[&_h2]:text-title-small tw:[&_h2]:font-semibold tw:[&_h2]:tracking-tight tw:[&_h2]:text-primary tw:[&_h3]:m-0 tw:[&_h3]:text-title-small tw:[&_h3]:font-semibold tw:[&_h3]:tracking-tight tw:[&_h3]:text-primary",
  "sectionHeading": "tw:min-w-0 tw:flex-1",
  "sourceLabel": "tw:mt-0 tw:mb-1 tw:text-caption tw:font-medium tw:text-muted-foreground",
  "sectionMenu": "tw:mt-0.5 tw:shrink-0",
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
