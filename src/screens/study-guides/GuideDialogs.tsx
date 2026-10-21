import { useRef, useState } from "react";

import type { StudyGuidePdfTheme } from "../../ipc/studyGuides";
import { Sheet } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { FormField } from "../../ui/primitives/FormField";
import { RadioGroup, Textarea } from "../../ui/primitives/FormControls";

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

const PDF_THEMES: ReadonlyArray<{ value: StudyGuidePdfTheme; name: string; type: string; colors: PdfThemeColors }> = [
  { value: "navy", name: "Navy", type: "Plex Sans and Libertinus", colors: {
    paper: "var(--ostudy-pdf-theme-preview-navy-paper)", primary: "var(--ostudy-pdf-theme-preview-navy-primary)",
    accent: "var(--ostudy-pdf-theme-preview-navy-accent)", highlight: "var(--ostudy-pdf-theme-preview-navy-highlight)" } },
  { value: "sage", name: "Sage", type: "Caladea and Lato", colors: {
    paper: "var(--ostudy-pdf-theme-preview-sage-paper)", primary: "var(--ostudy-pdf-theme-preview-sage-primary)",
    accent: "var(--ostudy-pdf-theme-preview-sage-accent)", highlight: "var(--ostudy-pdf-theme-preview-sage-highlight)" } },
  { value: "slate", name: "Slate", type: "Sora and Plex Sans", colors: {
    paper: "var(--ostudy-pdf-theme-preview-slate-paper)", primary: "var(--ostudy-pdf-theme-preview-slate-primary)",
    accent: "var(--ostudy-pdf-theme-preview-slate-accent)", highlight: "var(--ostudy-pdf-theme-preview-slate-highlight)" } },
  { value: "plum", name: "Plum", type: "Roboto Slab and Libertinus", colors: {
    paper: "var(--ostudy-pdf-theme-preview-plum-paper)", primary: "var(--ostudy-pdf-theme-preview-plum-primary)",
    accent: "var(--ostudy-pdf-theme-preview-plum-accent)", highlight: "var(--ostudy-pdf-theme-preview-plum-highlight)" } },
];

interface PdfThemeColors { paper: string; primary: string; accent: string; highlight: string; }

/** A miniature of the PDF's cover colours beside the theme's name and typefaces. */
function PdfThemeSwatch({ name, type, colors }: { name: string; type: string; colors: PdfThemeColors }) {
  return <span className="tw:grid tw:w-full tw:gap-1.5">
    <span aria-hidden="true" className="tw:flex tw:h-10 tw:w-full tw:flex-col tw:overflow-hidden tw:rounded-md tw:ring-1 tw:ring-border" style={{ background: colors.paper }}>
      <span className="tw:flex tw:h-6 tw:items-center tw:justify-end tw:px-2" style={{ background: colors.primary }}>
        <span className="tw:size-2.5 tw:rounded-full" style={{ background: colors.highlight }} />
      </span>
      <span className="tw:m-1.5 tw:h-1 tw:w-1/2 tw:rounded-full" style={{ background: colors.accent }} />
    </span>
    <span className="tw:grid">
      <strong className="tw:text-small tw:font-medium tw:text-foreground">{name}</strong>
      <span className="tw:text-caption tw:text-muted-foreground">{type}</span>
    </span>
  </span>;
}

/** The study guide PDF themes as cards, each a miniature of its cover beside its typefaces. */
export function GuidePdfThemePicker({ value, onChange }: {
  value: StudyGuidePdfTheme;
  onChange: (theme: StudyGuidePdfTheme) => void;
}) {
  return (
    <RadioGroup label="Study guide PDF theme" value={value} presentation="cards"
      className="tw:md:grid-cols-2 tw:xl:grid-cols-4 tw:gap-3" onValueChange={onChange}
      options={PDF_THEMES.map((option) => ({
        value: option.value,
        ariaLabel: option.name,
        className: "tw:p-3",
        label: <PdfThemeSwatch name={option.name} type={option.type} colors={option.colors} />,
      }))} />
  );
}
