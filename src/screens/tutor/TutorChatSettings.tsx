import { useState } from "react";

import { Dialog } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { MaterialPreparation } from "./MaterialPreparation";
import { TutorMemoryControls } from "./TutorMemoryControls";
import type { TutorController } from "./useTutorController";
import styles from "./TutorWorkspace.module.css";

export function TutorChatSettings({ tutor, onClose, onDelete }: {
  tutor: TutorController;
  onClose: () => void;
  onDelete: () => Promise<void>;
}) {
  const [memoryBusy, setMemoryBusy] = useState(false);
  const locked = tutor.busy || tutor.loading || tutor.pending !== null || tutor.preparation.locked;
  return <Dialog title="Chat settings" onClose={onClose} busy={memoryBusy} width="medium">
    <p className={styles.muted}>Chat uses your Tutor model from Settings → AI/LLM. Search uses the selected embedding model.
      Switching the chat model does not reinterpret saved documents.</p>
    {tutor.scope.kind !== "general" ? <label className={styles.checkbox}>
      <input type="checkbox" checked={tutor.includeConversations} disabled={locked || memoryBusy}
        onChange={(event) => tutor.setIncludeConversations(event.target.checked)} />
      Search permitted conversations in this material selection
    </label> : null}
    {tutor.detail ? <TutorMemoryControls conversationId={tutor.detail.conversation.id}
      revision={tutor.detail.conversation.revision} retrievalAvailable={tutor.scope.kind !== "general"}
      disabled={locked} onBusyChange={setMemoryBusy} onChanged={tutor.refresh} /> : null}
    {tutor.scope.kind !== "general" ? <MaterialPreparation controller={tutor.preparation} disabled={memoryBusy || tutor.busy || tutor.loading || tutor.pending !== null} /> : null}
    <div className={styles.toolbar}>
      <Button disabled={locked || memoryBusy} onClick={() => { void tutor.initialize(); void tutor.refresh(); }}>Refresh saved state</Button>
      {tutor.detail ? <Button variant="dangerGhost" disabled={locked || memoryBusy} onClick={() => { void onDelete(); }}>Delete conversation</Button> : null}
    </div>
  </Dialog>;
}
