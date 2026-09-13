// Editor chrome uses semantic utilities; RichContent owns document descendants.
export const editorStyles = {
  "field": "tw:min-w-0",
  "heading": "tw:mb-3 tw:flex tw:flex-wrap tw:items-end tw:justify-between tw:gap-3 tw:[&_h3]:m-0 tw:[&_h3]:text-section tw:[&_h3]:font-semibold tw:[&_p]:mt-1 tw:[&_p]:mb-0 tw:[&_p]:text-label tw:[&_p]:text-muted-foreground",
  "shell": "tw:min-w-0 tw:overflow-hidden tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:shadow-panel",
  "surface": "tw:min-h-44 tw:overflow-x-auto tw:bg-background tw:p-5",
  "loading": "tw:min-h-44 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:p-5 tw:text-muted-foreground",
  "insertForm": "tw:grid tw:gap-4 tw:p-5",
  "insertGrid": "tw:grid tw:gap-4 tw:sm:grid-cols-2 tw:[&_label]:grid tw:[&_label]:gap-2 tw:[&_label]:text-label",
  "insertCheckbox": "tw:flex tw:items-center tw:gap-3 tw:text-label",
  "linkActions": "tw:flex tw:flex-wrap tw:justify-end tw:gap-2",
  "insertActions": "tw:flex tw:flex-wrap tw:justify-end tw:gap-2",
  "insertHelp": "tw:m-0 tw:text-label tw:leading-relaxed tw:text-muted-foreground",
  "checklistDraft": "tw:grid tw:gap-3",
  "checklistDraftRow": "tw:grid tw:items-center tw:gap-3 tw:sm:grid-cols-[auto_minmax(0,1fr)_auto]",
  "correctToggle": "tw:flex tw:items-center tw:gap-2 tw:whitespace-nowrap tw:text-label"
} as const;
