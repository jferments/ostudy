import { useId, useRef, useState } from "react";

import type { StudyGuidePdfTheme } from "../../ipc/studyGuides";
import { Sheet } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { FormField } from "../../ui/primitives/FormField";
import { Checkbox, RadioGroup, Textarea } from "../../ui/primitives/FormControls";

/**
 * Writes a guide note, a section note or a learner's version of a section. Saving blank text
 * removes an existing note or edit, so the dialog never stores an empty document.
 */
export function GuideTextDialog({ title, description, label, initialValue, width = "medium", saving, onSave, onClose }: {
  title: string;
  description: string;
  label: string;
  initialValue: string;
  /** A rewrite of a whole section gets more room than a note. */
  width?: "medium" | "large";
  saving: boolean;
  onSave: (value: string) => void;
  onClose: () => void;
}) {
  const [value, setValue] = useState(initialValue);
  const fieldRef = useRef<HTMLTextAreaElement>(null);
  return (
    <Sheet title={title} description={description} width={width} busy={saving} initialFocusRef={fieldRef}
      onClose={onClose} onSubmit={() => onSave(value)}
      actions={<>
        <Button variant="secondary" disabled={saving} onClick={onClose}>Cancel</Button>
        <Button type="submit" variant="primary" loading={saving}>Save</Button>
      </>}>
      <FormField label={label}>
        <Textarea ref={fieldRef} value={value} rows={12} disabled={saving}
          onChange={(event) => setValue(event.currentTarget.value)} />
      </FormField>
    </Sheet>
  );
}

const PDF_THEMES: ReadonlyArray<{ value: StudyGuidePdfTheme; name: string; type: string }> = [
  { value: "navy", name: "Navy", type: "Plex Sans and Libertinus" },
  { value: "sage", name: "Sage", type: "Caladea and Lato" },
  { value: "slate", name: "Slate", type: "Sora and Plex Sans" },
  { value: "plum", name: "Plum", type: "Roboto Slab and Libertinus" },
];

/** A miniature of the PDF's cover colours beside the theme's name and typefaces. */
function PdfThemeSwatch({ theme, name, type }: { theme: StudyGuidePdfTheme; name: string; type: string }) {
  const color = (part: string) => `var(--ostudy-pdf-theme-preview-${theme}-${part})`;
  return <span className="tw:grid tw:w-full tw:gap-1.5">
    <span aria-hidden="true" className="tw:flex tw:h-10 tw:w-full tw:flex-col tw:overflow-hidden tw:rounded-md tw:ring-1 tw:ring-border" style={{ background: color("paper") }}>
      <span className="tw:flex tw:h-6 tw:items-center tw:justify-end tw:px-2" style={{ background: color("primary") }}>
        <span className="tw:size-2.5 tw:rounded-full" style={{ background: color("highlight") }} />
      </span>
      <span className="tw:m-1.5 tw:h-1 tw:w-1/2 tw:rounded-full" style={{ background: color("accent") }} />
    </span>
    <span className="tw:grid">
      <strong className="tw:text-small tw:font-medium tw:text-foreground">{name}</strong>
      <span className="tw:text-caption tw:text-muted-foreground">{type}</span>
    </span>
  </span>;
}

/** Asks for the PDF's theme and whether it should carry the learner's notes before the native export prepares it. */
export function GuideExportDialog({ exporting, onExport, onClose }: {
  exporting: boolean;
  onExport: (includeNotes: boolean, theme: StudyGuidePdfTheme) => void;
  onClose: () => void;
}) {
  const [includeNotes, setIncludeNotes] = useState(true);
  const [theme, setTheme] = useState<StudyGuidePdfTheme>("navy");
  const hintId = `guide-export-hint-${useId().replaceAll(":", "")}`;
  return (
    <Sheet title="Export as PDF" description="Save the whole guide as a PDF you can print or share." width="small"
      busy={exporting} onClose={onClose} onSubmit={() => onExport(includeNotes, theme)}
      actions={<>
        <Button variant="secondary" disabled={exporting} onClick={onClose}>Cancel</Button>
        <Button type="submit" variant="primary" loading={exporting}>Export PDF</Button>
      </>}>
      <div className="tw:grid tw:gap-5">
        <div className="tw:grid tw:gap-2">
          <span aria-hidden="true" className="tw:text-small tw:font-medium tw:text-foreground">Theme</span>
          <RadioGroup label="PDF theme" value={theme} disabled={exporting} presentation="cards"
            className="tw:lg:grid-cols-2 tw:gap-3" onValueChange={setTheme}
            options={PDF_THEMES.map((option) => ({
              value: option.value,
              ariaLabel: option.name,
              className: "tw:p-3",
              label: <PdfThemeSwatch theme={option.value} name={option.name} type={option.type} />,
            }))} />
        </div>
        <div className="tw:grid tw:gap-0.5">
          <label className="tw:flex tw:min-h-11 tw:items-center tw:gap-3">
            <Checkbox checked={includeNotes} disabled={exporting} aria-describedby={hintId} onCheckedChange={setIncludeNotes} />
            <span className="tw:text-small tw:font-medium tw:text-foreground">Include my notes</span>
          </label>
          {/* Indented past the checkbox so the hint lines up with its label. */}
          <p id={hintId} className="tw:m-0 tw:pl-8 tw:text-small tw:text-muted-foreground tw:coarse:pl-9">
            Adds your guide note and section notes to the PDF.
          </p>
        </div>
      </div>
    </Sheet>
  );
}
