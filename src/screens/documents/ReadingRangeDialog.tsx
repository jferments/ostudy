import { useState } from "react";

import type { SourceRangeInput } from "../../ipc/collections";
import type { DocumentSummary } from "../../ipc/documents";
import type { DocumentStudyRange } from "../../ipc/subjects";
import { Dialog } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { TextField } from "../../ui/primitives/TextField";

/** Which pages, slides or sections of a document its subject studies; empty studies all of it. */
export function ReadingRangeDialog({ document, ranges, busy, error, onClose, onSave }: {
  document: DocumentSummary;
  ranges: readonly DocumentStudyRange[];
  busy: boolean;
  error: string | null;
  onClose: () => void;
  onSave: (ranges: SourceRangeInput[]) => Promise<boolean>;
}) {
  const [value, setValue] = useState(() => ranges.map((range) => (
    range.startUnit === range.endUnit ? String(range.startUnit) : `${range.startUnit}-${range.endUnit}`
  )).join(", "));
  const [invalid, setInvalid] = useState<string | null>(null);
  const units = unitPlural(document.sourceUnitKind);

  async function save(next: SourceRangeInput[]): Promise<void> {
    if (await onSave(next)) onClose();
  }

  return (
    <Dialog
      title="Reading range"
      description={`${capitalize(units)} 1 to ${document.sourceUnitCount}`}
      onClose={onClose}
      busy={busy}
      width="small"
      onSubmit={() => {
        const parsed = parseRanges(value, document);
        if (typeof parsed === "string") setInvalid(parsed);
        else void save(parsed);
      }}
      actions={<>
        <Button onClick={onClose}>Cancel</Button>
        <Button loading={busy} onClick={() => void save([])}>Whole document</Button>
        <Button type="submit" variant="primary" loading={busy}>Save range</Button>
      </>}
    >
      <label className="tw:grid tw:min-w-0 tw:gap-2 tw:text-small tw:font-medium">
        {capitalize(units)}
        <TextField
          autoFocus
          value={value}
          placeholder={document.sourceUnitCount > 1 ? "1-3, 7" : "1"}
          onChange={(event) => { setValue(event.target.value); setInvalid(null); }}
        />
      </label>
      {invalid ?? error ? <InlineNotice kind="error">{invalid ?? error}</InlineNotice> : null}
    </Dialog>
  );
}

export function parseRanges(value: string, document: DocumentSummary): SourceRangeInput[] | string {
  const text = value.trim();
  if (!text) return [];
  if (document.sourceUnitCount < 1) return "This source does not currently expose bounded source units.";
  const ranges: SourceRangeInput[] = [];
  for (const part of text.split(",")) {
    const match = /^\s*(\d+)(?:\s*-\s*(\d+))?\s*$/.exec(part);
    if (!match) return "Enter ranges like 1-3, 7.";
    const start = Number(match[1]);
    const end = Number(match[2] ?? match[1]);
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 1 || end < start || end > document.sourceUnitCount) {
      return `Use ${unitPlural(document.sourceUnitKind)} between 1 and ${document.sourceUnitCount}.`;
    }
    ranges.push({ unitKind: document.sourceUnitKind, startUnit: start, endUnit: end });
  }
  return ranges;
}

/** "pages 1–3 studied", or null when the subject studies the whole document. */
export function rangeLabel(document: DocumentSummary, ranges: readonly DocumentStudyRange[]): string | null {
  if (!ranges.length) return null;
  if (ranges.some((range) => range.sourceSha256 !== document.sha256)) return "reading range needs review";
  const pieces = ranges.map((range) => range.startUnit === range.endUnit ? String(range.startUnit) : `${range.startUnit}–${range.endUnit}`);
  return `${unitPlural(document.sourceUnitKind)} ${pieces.join(", ")} studied`;
}

function unitPlural(kind: string): string {
  switch (kind) {
    case "page": return "pages";
    case "slide": return "slides";
    case "chapter": return "chapters";
    case "image": return "images";
    case "segment": return "segments";
    case "time_segment": return "time segments";
    default: return "sections";
  }
}

function capitalize(value: string): string {
  return value ? value.charAt(0).toUpperCase() + value.slice(1) : value;
}
