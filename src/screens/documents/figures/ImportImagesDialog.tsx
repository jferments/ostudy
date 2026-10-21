import { useId, useState } from "react";
import { Dialog } from "../../../ui/overlays/Dialog";
import { Button } from "../../../ui/primitives/Button";
import { Checkbox } from "../../../ui/primitives/FormControls";

/**
 * Asks, for a PDF the learner chose, whether to use its images. Figures are
 * found once the pages are read, and kept with the pages that explain them.
 */
export function ImportImagesDialog({ fileName, onImport, onCancel }: {
  fileName: string;
  onImport: (useImages: boolean) => void;
  onCancel: () => void;
}) {
  const [useImages, setUseImages] = useState(true);
  const hintId = useId();
  return (
    <Dialog title="Import document" description={fileName} width="small" onClose={onCancel}
      onSubmit={() => onImport(useImages)}
      actions={<>
        <Button onClick={onCancel}>Cancel</Button>
        <Button type="submit" variant="primary">Import</Button>
      </>}>
      <div className="tw:grid tw:gap-0.5">
        <label className="tw:flex tw:min-h-11 tw:items-center tw:gap-3">
          <Checkbox checked={useImages} aria-describedby={hintId} onCheckedChange={setUseImages} />
          <span className="tw:text-small tw:font-medium tw:text-foreground">Use images from this document</span>
        </label>
        {/* Indented past the checkbox so the hint lines up with its label. */}
        <p id={hintId} className="tw:m-0 tw:pl-8 tw:text-small tw:text-muted-foreground tw:coarse:pl-9">
          oStudy finds its figures and keeps them with the pages that explain them. Turn this off to use only the text.
        </p>
      </div>
    </Dialog>
  );
}
