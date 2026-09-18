// Editor chrome uses semantic utilities; RichContent owns document descendants.
export const editorStyles = {
  "field": "tw:min-w-0",
  "heading": "tw:mb-2 tw:flex tw:flex-wrap tw:items-end tw:justify-between tw:gap-2 tw:[&_h3]:m-0 tw:[&_h3]:text-base tw:[&_h3]:font-medium tw:[&_p]:mt-0.5 tw:[&_p]:mb-0 tw:[&_p]:text-sm tw:[&_p]:text-muted-foreground",
  "shell": "tw:min-w-0 tw:overflow-hidden tw:rounded-lg tw:bg-card tw:ring-1 tw:ring-border tw:focus-within:ring-2 tw:focus-within:ring-primary",
  "surface": "tw:cursor-text tw:overflow-x-auto tw:bg-background tw:p-4",
  "loading": "tw:min-h-24 tw:rounded-lg tw:bg-muted/30 tw:p-4 tw:text-muted-foreground tw:ring-1 tw:ring-border",
  "insertForm": "tw:grid tw:gap-3 tw:p-4",
  "insertGrid": "tw:grid tw:gap-3 tw:sm:grid-cols-2 tw:[&_label]:grid tw:[&_label]:gap-1.5 tw:[&_label]:text-sm tw:[&_label]:font-medium",
  "insertCheckbox": "tw:flex tw:items-center tw:gap-2.5 tw:text-sm",
  "linkActions": "tw:flex tw:flex-wrap tw:justify-end tw:gap-2",
  "insertActions": "tw:flex tw:flex-wrap tw:justify-end tw:gap-2",
  "insertHelp": "tw:m-0 tw:text-sm tw:leading-relaxed tw:text-muted-foreground",
  "checklistDraft": "tw:grid tw:gap-2",
  "checklistDraftRow": "tw:grid tw:items-center tw:gap-2 tw:sm:grid-cols-[auto_minmax(0,1fr)_auto]",
  "correctToggle": "tw:flex tw:items-center tw:gap-2 tw:whitespace-nowrap tw:text-sm",
} as const;
