// Editor chrome uses semantic utilities; RichContent owns document descendants.
export const editorStyles = {
  "field": "tw:min-w-0",
  "heading": "tw:flex tw:min-h-9 tw:flex-wrap tw:items-center tw:justify-between tw:gap-2 tw:bg-muted tw:px-4 tw:pt-2 tw:[&_h3]:m-0 tw:[&_h3]:text-caption tw:[&_h3]:font-semibold tw:[&_h3]:tracking-wide tw:[&_h3]:text-muted-foreground tw:[&_h3]:uppercase tw:[&_p]:mt-0.5 tw:[&_p]:mb-0 tw:[&_p]:text-small tw:[&_p]:text-muted-foreground",
  "shell": "tw:min-w-0 tw:overflow-hidden tw:rounded-lg tw:bg-card tw:shadow-sm tw:ring-1 tw:ring-border tw:focus-within:ring-2 tw:focus-within:ring-primary",
  "surface": "tw:cursor-text tw:overflow-x-auto tw:bg-card tw:p-4",
  "loading": "tw:min-h-24 tw:p-4 tw:text-muted-foreground",
  "insertGrid": "tw:grid tw:gap-3 tw:sm:grid-cols-2 tw:[&_label]:grid tw:[&_label]:gap-1.5 tw:[&_label]:text-small tw:[&_label]:font-medium",
  "insertCheckbox": "tw:flex tw:items-center tw:gap-2.5 tw:text-small",
  "insertHelp": "tw:m-0 tw:text-small tw:leading-relaxed tw:text-muted-foreground",
  "checklistDraft": "tw:grid tw:gap-2",
  "checklistDraftRow": "tw:grid tw:items-center tw:gap-2 tw:sm:grid-cols-[auto_minmax(0,1fr)_auto]",
  "correctToggle": "tw:flex tw:items-center tw:gap-2 tw:whitespace-nowrap tw:text-small",
} as const;
