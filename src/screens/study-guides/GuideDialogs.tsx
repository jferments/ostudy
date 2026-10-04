import { useId, useRef, useState } from "react";

import { Sheet } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { FormField } from "../../ui/primitives/FormField";
import { Checkbox, Textarea } from "../../ui/primitives/FormControls";

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

/** Asks whether the PDF should carry the learner's notes before the native export prepares it. */
export function GuideExportDialog({ exporting, onExport, onClose }: {
  exporting: boolean;
  onExport: (includeNotes: boolean) => void;
  onClose: () => void;
}) {
  const [includeNotes, setIncludeNotes] = useState(true);
  const hintId = `guide-export-hint-${useId().replaceAll(":", "")}`;
  return (
    <Sheet title="Export as PDF" description="Save the whole guide as a PDF you can print or share." width="small"
      busy={exporting} onClose={onClose} onSubmit={() => onExport(includeNotes)}
      actions={<>
        <Button variant="secondary" disabled={exporting} onClick={onClose}>Cancel</Button>
        <Button type="submit" variant="primary" loading={exporting}>Export PDF</Button>
      </>}>
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
    </Sheet>
  );
}
