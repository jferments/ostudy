// Screen-owned layout recipes; generic controls render through oStudy UI facades.
const styles = {
  "root": "tw:grid tw:min-w-0 tw:gap-3",
  "primaryBar": "tw:grid tw:min-w-0 tw:gap-3 tw:p-4 tw:items-end tw:lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)_minmax(0,1fr)_auto]",
  "searchField": "tw:grid tw:min-w-0 tw:gap-2 tw:text-small tw:font-medium",
  "sortField": "tw:grid tw:min-w-0 tw:gap-2 tw:text-small tw:font-medium",
  "searchControl": "tw:relative tw:flex tw:min-w-0 tw:items-center tw:[&>svg]:absolute tw:[&>svg]:left-3 tw:[&>svg]:z-10 tw:[&>svg]:pointer-events-none tw:[&>svg]:text-muted-foreground tw:[&_input]:pl-10",
  "scopeButton": "tw:justify-start tw:[&_span]:truncate",
  "filterButton": "tw:gap-2",
  "filterCount": "tw:grid tw:size-5 tw:place-items-center tw:rounded-full tw:bg-primary tw:text-caption tw:text-primary-foreground",
  "filterSheet": "tw:grid tw:gap-5",
  "group": "tw:grid tw:min-w-0 tw:gap-3 tw:m-0 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:p-4 tw:[&_legend]:px-2 tw:[&_legend]:text-small tw:[&_legend]:font-semibold tw:[&_legend_small]:block tw:[&_legend_small]:font-normal tw:[&_legend_small]:text-muted-foreground",
  "folderChoices": "tw:grid tw:gap-2 tw:sm:grid-cols-2",
  "scopeChoice": "tw:h-auto tw:min-h-11 tw:justify-start tw:gap-2 tw:whitespace-normal tw:aria-pressed:border-primary tw:aria-pressed:bg-accent tw:[&_small]:ml-auto tw:[&_small]:text-caption tw:[&_small]:font-normal tw:[&_small]:text-muted-foreground",
  "checklist": "tw:grid tw:max-h-72 tw:min-w-0 tw:overflow-auto tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-background tw:[&_label]:grid tw:[&_label]:grid-cols-[auto_minmax(0,1fr)_auto] tw:[&_label]:min-h-12 tw:[&_label]:items-center tw:[&_label]:gap-3 tw:[&_label]:p-3 tw:[&_label]:text-small tw:[&_label]:cursor-pointer tw:[&_label]:hover:bg-muted/40 tw:[&_small]:text-caption tw:[&_small]:text-muted-foreground tw:[&_span]:min-w-0",
  "controlGrid": "tw:grid tw:min-w-0 tw:gap-4 tw:md:grid-cols-3 tw:[&_label]:grid tw:[&_label]:min-w-0 tw:[&_label]:gap-2 tw:[&_label]:text-small",
  "emptyCopy": "tw:m-0 tw:text-small tw:leading-relaxed tw:text-muted-foreground",
  "sheetActions": "tw:flex tw:flex-wrap tw:justify-end tw:gap-2"
} as const;
export default styles;
