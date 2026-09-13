// Static, semantic Tailwind recipes. Domain state and rich-content styling live elsewhere.
export const presentation = {
  "card-editor-layout": "tw:grid tw:min-w-0 tw:grid-cols-1 tw:items-start tw:gap-6",
  "with-preview": "tw:xl:grid-cols-[minmax(0,1.5fr)_minmax(20rem,1fr)]",
  "card-editor-fields": "tw:grid tw:min-w-0 tw:gap-6",
  "eyebrow": "tw:text-xs tw:font-medium tw:uppercase tw:tracking-wider tw:text-muted-foreground",
  "editor-preview-panel": "tw:min-w-0 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:p-5 tw:shadow-panel tw:xl:sticky tw:xl:top-5",
  "preview-panel-heading": "tw:mb-4 tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3 tw:[&_h2]:m-0 tw:[&_h2]:text-xl tw:[&_h3]:m-0 tw:[&_h3]:text-section tw:[&_label]:grid tw:[&_label]:gap-2",
  "flashcard-preview-surface": "tw:min-h-64 tw:overflow-x-auto tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-background tw:p-6",
  "cloze-editor": "tw:grid tw:gap-5 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:p-5",
  "section-heading": "tw:flex tw:flex-wrap tw:items-start tw:justify-between tw:gap-3 tw:[&_h2]:m-0 tw:[&_h2]:text-xl tw:[&_p]:mt-2 tw:[&_p]:text-label tw:[&_p]:leading-relaxed tw:[&_p]:text-muted-foreground",
  "cloze-source-field": "tw:grid tw:gap-2 tw:text-label tw:font-medium tw:[&_textarea]:min-h-48 tw:[&_textarea]:font-normal",
  "cloze-toolbar": "tw:flex tw:flex-wrap tw:items-center tw:gap-2",
  "cloze-preview-sides": "tw:grid tw:gap-4 tw:md:grid-cols-2 tw:[&>div]:rounded-lg tw:[&>div]:border tw:[&>div]:border-solid tw:[&>div]:border-border tw:[&>div]:bg-background tw:[&>div]:p-4 tw:[&_small]:text-xs tw:[&_small]:font-medium tw:[&_small]:uppercase tw:[&_small]:text-muted-foreground tw:[&_p]:whitespace-pre-wrap",
  "template-entry-form": "tw:grid tw:min-w-0 tw:gap-5 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:p-5",
  "template-rich-value-list": "tw:grid tw:min-w-0 tw:gap-5",
  "template-preview-actions": "tw:flex tw:flex-wrap tw:justify-between tw:gap-3",
  "template-card-previews": "tw:grid tw:min-w-0 tw:gap-4 tw:[&_article]:grid tw:[&_article]:min-w-0 tw:[&_article]:gap-3 tw:lg:[&_article]:grid-cols-[8rem_minmax(0,1fr)_minmax(0,1fr)] tw:[&_article>div]:overflow-x-auto tw:[&_article>div]:rounded-lg tw:[&_article>div]:border tw:[&_article>div]:border-solid tw:[&_article>div]:border-border tw:[&_article>div]:bg-background tw:[&_article>div]:p-4 tw:[&_small]:text-xs tw:[&_small]:font-medium tw:[&_small]:uppercase tw:[&_small]:text-muted-foreground"
} as const;
