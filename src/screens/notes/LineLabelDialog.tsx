import { useState } from "react";
import { Dialog } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { FormField } from "../../ui/primitives/FormField";
import { TextField } from "../../ui/primitives/TextField";
import { MAX_LABEL_CHARS } from "../documents/annotator/connectors";

/** The few words written on a line, such as how two linked topics relate. An empty label takes them off. */
export function LineLabelDialog({ initial, onClose, onSave }: {
  initial: string;
  onClose: () => void;
  onSave: (label: string) => void;
}) {
  const [label, setLabel] = useState(initial);
  return (
    <Dialog
      title={initial ? "Edit label" : "Add label"}
      width="small"
      onClose={onClose}
      onSubmit={(event) => {
        event.preventDefault();
        onSave(label);
      }}
      actions={<>
        <Button variant="secondary" onClick={onClose}>Cancel</Button>
        <Button variant="primary" type="submit" disabled={!label.trim() && !initial}>{label.trim() || !initial ? "Save" : "Remove label"}</Button>
      </>}
    >
      <FormField label="Label">
        <TextField value={label} maxLength={MAX_LABEL_CHARS} autoFocus placeholder="causes, leads to, is part of…"
          onChange={(event) => setLabel(event.target.value)} />
      </FormField>
    </Dialog>
  );
}
