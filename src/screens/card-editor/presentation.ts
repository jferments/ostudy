// Static, semantic Tailwind recipes. Domain state and rich-content styling live elsewhere.
export const presentation = {
  "card-editor-layout": "tw:grid tw:min-w-0 tw:grid-cols-1 tw:items-start tw:gap-5",
  "card-editor-fields": "tw:grid tw:min-w-0 tw:gap-5",
  "eyebrow": "tw:text-xs tw:font-medium tw:uppercase tw:tracking-wide tw:text-muted-foreground",
  "preview-panel-heading": "tw:mb-3 tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-2 tw:[&_h2]:m-0 tw:[&_h2]:text-base tw:[&_h2]:font-medium tw:[&_h3]:m-0 tw:[&_h3]:text-base tw:[&_h3]:font-medium tw:[&_label]:grid tw:[&_label]:gap-1.5",
  "cloze-editor": "tw:grid tw:gap-4 tw:rounded-xl tw:bg-card tw:p-4 tw:ring-1 tw:ring-border",
  "section-heading": "tw:flex tw:flex-wrap tw:items-start tw:justify-between tw:gap-3 tw:[&_h2]:m-0 tw:[&_h2]:text-base tw:[&_h2]:font-medium tw:[&_p]:mt-1 tw:[&_p]:text-sm tw:[&_p]:leading-relaxed tw:[&_p]:text-muted-foreground",
  "cloze-source-field": "tw:grid tw:gap-1.5 tw:text-sm tw:font-medium tw:[&_textarea]:min-h-48 tw:[&_textarea]:font-normal",
  "cloze-toolbar": "tw:flex tw:flex-wrap tw:items-center tw:gap-1.5",
  "cloze-preview-sides": "tw:grid tw:gap-3 tw:md:grid-cols-2 tw:[&>div]:rounded-lg tw:[&>div]:bg-muted/25 tw:[&>div]:p-3 tw:[&_small]:text-xs tw:[&_small]:font-medium tw:[&_small]:uppercase tw:[&_small]:text-muted-foreground tw:[&_p]:whitespace-pre-wrap",
  "template-entry-form": "tw:grid tw:min-w-0 tw:gap-4 tw:rounded-xl tw:bg-card tw:p-4 tw:ring-1 tw:ring-border",
  "template-rich-value-list": "tw:grid tw:min-w-0 tw:gap-4",
  "template-preview-actions": "tw:flex tw:flex-wrap tw:justify-between tw:gap-2",
  "template-card-previews": "tw:grid tw:min-w-0 tw:gap-3 tw:[&_article]:grid tw:[&_article]:min-w-0 tw:[&_article]:gap-2 tw:lg:[&_article]:grid-cols-[8rem_minmax(0,1fr)_minmax(0,1fr)] tw:[&_article>div]:overflow-x-auto tw:[&_article>div]:rounded-lg tw:[&_article>div]:bg-muted/25 tw:[&_article>div]:p-3 tw:[&_small]:text-xs tw:[&_small]:font-medium tw:[&_small]:uppercase tw:[&_small]:text-muted-foreground",
} as const;
