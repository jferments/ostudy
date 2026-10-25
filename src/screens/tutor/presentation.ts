// Tutor presentation only. Scope, history, retrieval and operation identity remain controller-owned.
export const styles = {
  // The screen fills the main area: the chat list, the chat and an open source share its height.
  screen: "tw:flex tw:h-full tw:min-h-[30rem] tw:min-w-0 tw:flex-col",
  workspace: "tw:flex tw:min-h-0 tw:min-w-0 tw:flex-1 tw:flex-col",
  panelWorkspace: "tw:flex tw:min-h-0 tw:min-w-0 tw:flex-1 tw:flex-col tw:h-[min(76dvh,58rem)]",
  withHistory: "tw:grid tw:min-h-0 tw:min-w-0 tw:flex-1 tw:grid-cols-[minmax(13rem,16rem)_minmax(0,1fr)]",
  withSource: "tw:grid-cols-[minmax(13rem,16rem)_minmax(0,1fr)_minmax(16rem,20rem)]",
  // A shaded pane, like the side rail, sets the chat list apart from the conversation.
  historyPane: "tw:flex tw:min-h-0 tw:min-w-0 tw:flex-col tw:overflow-y-auto tw:border-0 tw:border-r tw:border-solid tw:border-border/70 tw:bg-foreground/4 tw:p-3",
  chat: "tw:flex tw:min-h-0 tw:min-w-0 tw:flex-1 tw:flex-col tw:text-body",
  transcriptFrame: "tw:relative tw:flex tw:min-h-0 tw:min-w-0 tw:flex-1 tw:flex-col",
  hintChat: "tw:flex tw:min-h-0 tw:min-w-0 tw:flex-1 tw:flex-col tw:gap-2 tw:text-body",
  chatHeader: "tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-x-2 tw:gap-y-2 tw:border-0 tw:border-b tw:border-solid tw:border-border/70 tw:px-6 tw:py-3 tw:compact:px-3",
  heading: "tw:m-0 tw:min-w-0 tw:flex-[1_1_10rem] tw:truncate tw:text-body tw:font-semibold",
  toolbar: "tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-1.5",
  scope: "tw:h-auto tw:min-h-8 tw:max-w-full tw:gap-1.5 tw:rounded-lg tw:px-2.5 tw:py-1 tw:text-small tw:font-normal tw:[&>span]:min-w-0 tw:[&>span]:truncate tw:coarse:min-h-touch",
  // Below the title on a phone; beside it on wider screens.
  scopeRow: "tw:flex tw:min-w-0 tw:basis-full tw:items-center",
  history: "tw:flex tw:min-h-0 tw:min-w-0 tw:flex-1 tw:flex-col tw:gap-3",
  historyTop: "tw:flex tw:min-w-0 tw:items-center tw:gap-1.5 tw:[&>button:first-child]:flex-1",
  historyItem: "tw:h-auto tw:min-h-9 tw:min-w-0 tw:flex-1 tw:flex-col tw:items-stretch tw:justify-center tw:gap-0 tw:whitespace-normal tw:break-words tw:rounded-lg tw:border-transparent tw:bg-transparent tw:px-2.5 tw:py-2 tw:text-left tw:text-small tw:font-normal tw:shadow-none tw:aria-[current=page]:bg-background tw:aria-[current=page]:font-semibold tw:aria-[current=page]:shadow-sm tw:coarse:min-h-touch",
  historyGroup: "tw:grid tw:min-w-0 tw:gap-0.5",
  historyGroupHeader: "tw:flex tw:min-h-8 tw:min-w-0 tw:items-center tw:justify-between tw:gap-2 tw:pl-2.5 tw:[&>strong]:min-w-0 tw:[&>strong]:truncate tw:[&>strong]:text-caption tw:[&>strong]:font-semibold tw:[&>strong]:text-muted-foreground",
  // Row actions stay quiet until the row is hovered, focused or current; touch screens always show them.
  historyRow: "tw:group tw:flex tw:min-w-0 tw:items-center tw:gap-0.5 tw:[&>button:last-child]:opacity-0 tw:[&>button:last-child]:focus-visible:opacity-100 tw:hover:[&>button:last-child]:opacity-100 tw:has-[[aria-current=page]]:[&>button:last-child]:opacity-100 tw:coarse:[&>button:last-child]:opacity-100",
  historyMeta: "tw:block tw:min-w-0 tw:pt-0.5 tw:text-caption tw:font-normal tw:text-muted-foreground",
  historyArchive: "tw:mt-auto tw:grid tw:min-w-0 tw:gap-1 tw:pt-2",
  archiveList: "tw:grid tw:min-w-0 tw:gap-1 tw:p-1",
  field: "tw:grid tw:min-w-0 tw:gap-1.5 tw:text-small tw:font-medium",
  checkbox: "tw:my-2 tw:flex tw:min-h-9 tw:items-center tw:gap-2.5 tw:text-small tw:coarse:min-h-touch",
  projectDefault: "tw:flex tw:min-w-0 tw:items-center tw:justify-between tw:gap-3 tw:rounded-lg tw:bg-muted/35 tw:p-3 tw:[&>div]:min-w-0 tw:[&>div>strong]:block tw:[&>div>span]:block",
  sourceChoices: "tw:grid tw:max-h-[45dvh] tw:gap-1 tw:overflow-y-auto",
  sourceChoice: "tw:flex tw:min-h-9 tw:min-w-0 tw:cursor-pointer tw:items-center tw:gap-2.5 tw:rounded-lg tw:bg-transparent tw:px-2.5 tw:py-2 tw:hover:bg-muted/50 tw:coarse:min-h-touch tw:[&>span]:min-w-0 tw:[&>span]:break-words tw:[&_small]:mt-0.5 tw:[&_small]:block tw:[&_small]:text-caption tw:[&_small]:text-muted-foreground",
  notice: "tw:rounded-lg tw:bg-muted/35 tw:p-3 tw:text-small tw:leading-relaxed",
  options: "",
  muted: "tw:text-small tw:leading-relaxed tw:break-words tw:text-muted-foreground",
  notices: "tw:mx-auto tw:grid tw:w-full tw:max-w-[calc(48rem+3rem)] tw:min-w-0 tw:gap-2 tw:px-6 tw:compact:px-3 tw:empty:hidden",
  transcript: "tw:flex tw:min-h-32 tw:min-w-0 tw:flex-1 tw:flex-col tw:gap-5 tw:overflow-y-auto tw:overscroll-contain tw:px-6 tw:py-5 tw:compact:px-3",
  hintTranscript: "tw:flex tw:min-h-32 tw:min-w-0 tw:flex-1 tw:flex-col tw:gap-4 tw:overflow-y-auto tw:overscroll-contain tw:px-1 tw:py-3",
  // A new chat centres its question and its two optional starts.
  empty: "tw:m-auto tw:grid tw:max-w-xl tw:justify-items-center tw:gap-4 tw:px-4 tw:py-8 tw:text-center tw:[&>h3]:m-0 tw:[&>h3]:text-title tw:[&>h3]:font-semibold",
  // A new full-page chat: the raccoon, the question, the composer and the starters, centred.
  welcome: "tw:m-auto tw:grid tw:w-full tw:max-w-3xl tw:min-w-0 tw:justify-items-center tw:gap-5 tw:py-6 tw:text-center tw:[&>h3]:m-0 tw:[&>h3]:text-title tw:[&>h3]:font-semibold tw:[&>h3]:tracking-tight tw:compact:gap-4",
  welcomeArt: "tw:pointer-events-none tw:block tw:size-48 tw:select-none tw:compact:size-36",
  welcomeComposer: "tw:w-full tw:text-left",
  starters: "tw:m-0 tw:grid tw:w-full tw:list-none tw:grid-cols-2 tw:gap-3 tw:p-0 tw:compact:grid-cols-1",
  starter: "tw:h-full tw:w-full tw:items-start tw:gap-3 tw:rounded-2xl tw:border tw:border-solid tw:border-border/70 tw:bg-foreground/3 tw:px-4 tw:py-3 tw:text-left tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:hover:border-primary/60 tw:focus-visible:outline-none tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:reduce-motion:transition-none tw:[&_strong]:block tw:[&_strong]:text-small tw:[&_strong]:font-semibold tw:[&_small]:mt-0.5 tw:[&_small]:block tw:[&_small]:text-caption tw:[&_small]:text-muted-foreground",
  starterIcon: "tw:grid tw:size-9 tw:shrink-0 tw:place-items-center tw:rounded-lg tw:bg-primary/12 tw:text-primary",
  // Shown while the reader has scrolled up from the newest message.
  jump: "tw:absolute tw:bottom-3 tw:left-1/2 tw:-translate-x-1/2 tw:rounded-full tw:shadow-md",
  turn: "tw:mx-auto tw:grid tw:w-full tw:max-w-3xl tw:min-w-0 tw:gap-4",
  userMessage: "tw:ml-auto tw:max-w-[min(85%,36rem)] tw:min-w-0 tw:rounded-2xl tw:bg-primary/12 tw:px-4 tw:py-2.5",
  assistantMessage: "tw:min-w-0 tw:grid tw:gap-2",
  message: "tw:whitespace-pre-wrap tw:break-words tw:leading-7 tw:select-text",
  citations: "tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-1.5",
  // A numbered chip; its accessible name carries the source's title.
  citation: "tw:h-6 tw:min-h-6 tw:min-w-6 tw:rounded-md tw:border-primary/25 tw:bg-primary/8 tw:px-1.5 tw:text-caption tw:font-semibold tw:text-primary tw:shadow-none tw:coarse:min-h-touch tw:coarse:min-w-touch",
  answerActions: "tw:-ml-2 tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-1 tw:[&_button]:text-small tw:[&_button]:font-normal tw:[&_button]:text-muted-foreground",
  composerWrap: "tw:px-6 tw:pt-2 tw:pb-4 tw:compact:px-3 tw:compact:pb-3",
  composer: "tw:mx-auto tw:flex tw:w-full tw:max-w-3xl tw:min-w-0 tw:items-end tw:gap-2 tw:rounded-2xl tw:border tw:border-solid tw:border-border tw:bg-background tw:py-1.5 tw:pr-1.5 tw:pl-4 tw:shadow-sm tw:focus-within:border-primary/60",
  hintComposer: "tw:grid tw:min-w-0 tw:gap-2 tw:border-0 tw:border-t tw:border-solid tw:border-border/70 tw:pt-3",
  composerInput: "tw:min-h-10 tw:max-h-48 tw:flex-1 tw:resize-none tw:border-0 tw:bg-transparent tw:px-0 tw:py-2 tw:shadow-none tw:focus-visible:ring-0",
  hintComposerInput: "tw:min-h-20 tw:max-h-48 tw:resize-y tw:bg-background",
  composerLabel: "tw:sr-only",
  // The filled send arrow is the composer's one primary action.
  send: "tw:rounded-full tw:bg-primary tw:text-primary-foreground tw:hover:bg-primary/90 tw:disabled:bg-muted tw:disabled:text-muted-foreground",
  stop: "tw:rounded-full tw:bg-foreground tw:text-background tw:hover:bg-foreground/85",
  // A recording microphone stays visibly on until it is tapped again.
  micRecording: "tw:rounded-full tw:bg-destructive-soft tw:text-destructive tw:hover:bg-destructive/20",
  recordingTime: "tw:self-center tw:text-small tw:tabular-nums tw:text-muted-foreground",
  level: "tw:mb-4 tw:h-1.5 tw:w-12 tw:shrink-0 tw:overflow-hidden tw:rounded-full tw:bg-muted tw:[&>span]:block tw:[&>span]:h-full tw:[&>span]:bg-destructive tw:[&>span]:transition-[width] tw:motion-reduce:[&>span]:transition-none",
  speechStatus: "tw:mx-auto tw:mt-1 tw:min-h-5 tw:w-full tw:max-w-3xl tw:px-4 tw:text-small tw:text-muted-foreground tw:empty:hidden",
  speechError: "tw:mx-auto tw:mt-1 tw:w-full tw:max-w-3xl tw:px-4 tw:text-small tw:text-destructive",
  sourcePanel: "tw:flex tw:min-h-0 tw:min-w-0 tw:flex-col tw:gap-3 tw:overflow-y-auto tw:border-0 tw:border-l tw:border-solid tw:border-border/70 tw:bg-foreground/4 tw:p-4",
  sourcePanelHeader: "tw:flex tw:min-w-0 tw:items-center tw:justify-between tw:gap-2 tw:[&>h3]:m-0 tw:[&>h3]:text-body tw:[&>h3]:font-semibold",
  sourceCard: "tw:grid tw:min-w-0 tw:gap-2 tw:rounded-xl tw:border tw:border-solid tw:border-border/70 tw:bg-background tw:p-4 tw:[&>strong]:text-small tw:[&>small]:text-caption tw:[&>small]:text-muted-foreground",
} as const;

/** An answer's text without the figures it shows, which mean nothing pasted elsewhere. */
export function withoutFigures(text: string): string {
  return text.replace(/!\[[^\]\n]*\]\(figure:[^)\s]*\)[ \t]*\n?/gu, "").replace(/\n{3,}/gu, "\n\n").trim();
}
