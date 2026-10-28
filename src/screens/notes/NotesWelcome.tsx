import { Icon, type IconName } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";

const FEATURES: ReadonlyArray<{ icon: IconName; title: string; text: string }> = [
  { icon: "pen", title: "Write and draw", text: "Pens, highlighters, shapes and text on blank, lined or grid paper, with the same tools as your documents." },
  { icon: "link", title: "Link it together", text: "Point a note at a flashcard, a page of a document or another notebook, and open it with a tap." },
  { icon: "folder", title: "Keep it in order", text: "File notebooks in your own folders, pin the ones you use most, and find recent ones in Recent." },
];

/**
 * Notes before the first notebook: what a notebook is for, with a drawing of
 * one, and New notebook to start.
 */
export function NotesWelcome({ onCreate, onCreateFolder }: { onCreate: () => void; onCreateFolder: () => void }) {
  return (
    <div data-testid="notes-welcome" className="tw:grid tw:min-w-0 tw:items-center tw:gap-8 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:p-8 tw:compact:p-5 tw:wide:grid-cols-[minmax(0,1fr)_minmax(0,0.9fr)]">
      <div className="tw:grid tw:min-w-0 tw:gap-6">
        <div className="tw:grid tw:gap-2">
          <h2 className="tw:m-0 tw:text-section tw:font-semibold">Your notebooks live here</h2>
          <p className="tw:m-0 tw:text-muted-foreground">
            A notebook is pages of paper to think on: work a problem through, sketch a diagram, or
            sum up a chapter in your own words, and link it to the cards and pages it is about.
          </p>
        </div>
        <ul aria-label="What notebooks do" className="tw:m-0 tw:grid tw:list-none tw:gap-4 tw:p-0">
          {FEATURES.map((feature) => (
            <li key={feature.title} className="tw:flex tw:min-w-0 tw:gap-3">
              <span aria-hidden="true" className="tw:grid tw:size-9 tw:shrink-0 tw:place-items-center tw:rounded-lg tw:bg-accent tw:text-accent-foreground">
                <Icon name={feature.icon} size={18} />
              </span>
              <span className="tw:grid tw:min-w-0 tw:gap-0.5">
                <span className="tw:text-label tw:font-semibold">{feature.title}</span>
                <span className="tw:text-small tw:text-muted-foreground">{feature.text}</span>
              </span>
            </li>
          ))}
        </ul>
        <div className="tw:flex tw:flex-wrap tw:gap-2">
          <Button variant="primary" onClick={onCreate}><Icon name="plus" size={16} />New notebook</Button>
          <Button onClick={onCreateFolder}><Icon name="folder" size={16} />New folder</Button>
        </div>
      </div>
      <NotebookDrawing />
    </div>
  );
}

/** A notebook page with handwriting, a highlight, a sketch and two links. Drawn, so it follows the theme. */
function NotebookDrawing() {
  return (
    <div aria-hidden="true" className="tw:relative tw:mx-auto tw:w-full tw:max-w-96 tw:compact:hidden">
      <div className="tw:absolute tw:inset-0 tw:translate-x-3 tw:translate-y-3 tw:rotate-2 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-muted" />
      <div className="tw:relative tw:grid tw:gap-4 tw:rounded-lg tw:border tw:border-solid tw:border-border-strong tw:bg-background tw:p-5 tw:shadow-sm">
        <svg viewBox="0 0 320 170" className="tw:block tw:w-full tw:text-muted-foreground" fill="none" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round">
          {[30, 62, 94, 126, 158].map((y) => <line key={y} x1="0" x2="320" y1={y} y2={y} strokeWidth="0.75" opacity="0.35" />)}
          <path d="M6 22c10-9 18-9 22 0s10 9 18 0 14-8 20 0 12 7 20-2M98 22c8-8 14-6 18 0s12 4 18-3" strokeWidth="2.2" className="tw:text-foreground" />
          <rect x="4" y="40" width="150" height="16" rx="3" fill="currentColor" stroke="none" className="tw:text-warning" opacity="0.35" />
          <path d="M8 52c12-8 20-8 26 0s14 6 22-1 16-6 22 1 14 5 22-2 14-4 20 2M128 52c8-6 12-5 18 0" strokeWidth="2" className="tw:text-foreground" />
          <path d="M8 86c14-7 22-6 28 0s12 6 20 0M70 86c6-5 12-5 16 0s14 4 22-2" strokeWidth="2" className="tw:text-foreground" />
          <circle cx="232" cy="72" r="26" strokeWidth="2.2" className="tw:text-primary" />
          <path d="M210 72h44M232 50v44M258 72l40-22M298 50l-10 1 4 8" strokeWidth="2.2" className="tw:text-primary" />
          <path d="M8 118c10-6 18-6 24 0s14 5 20-1 12-5 18 0" strokeWidth="2" className="tw:text-foreground" />
          <path d="M6 150c30 6 70-12 104-4" strokeWidth="2" className="tw:text-chart-1" />
        </svg>
        <div className="tw:flex tw:flex-wrap tw:gap-2">
          <span className="tw:inline-flex tw:items-center tw:gap-1.5 tw:rounded-full tw:border tw:border-solid tw:border-border tw:bg-card tw:px-2.5 tw:py-1 tw:text-caption">
            <Icon name="cards" size={14} className="tw:text-primary" />Card: Krebs cycle
          </span>
          <span className="tw:inline-flex tw:items-center tw:gap-1.5 tw:rounded-full tw:border tw:border-solid tw:border-border tw:bg-card tw:px-2.5 tw:py-1 tw:text-caption">
            <Icon name="document" size={14} className="tw:text-primary" />Chapter 4, page 12
          </span>
        </div>
      </div>
    </div>
  );
}
