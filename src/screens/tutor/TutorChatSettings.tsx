import { useState } from "react";
import { Dialog } from "../../ui/overlays/Dialog";
import { Checkbox } from "../../ui/primitives/FormControls";
import { Button } from "../../ui/primitives/Button";
import { MaterialPreparation } from "./MaterialPreparation";
import { TutorMemoryControls } from "./TutorMemoryControls";
import type { TutorController } from "./useTutorController";
import { styles } from "./presentation";

export function TutorChatSettings({ tutor, onClose, onDelete }: { tutor: TutorController; onClose: () => void; onDelete: () => Promise<void> }) {
  const [memoryBusy, setMemoryBusy] = useState(false);
  const locked = tutor.busy || tutor.loading || tutor.pending !== null || tutor.preparation.locked;
  return <Dialog title="Chat settings" onClose={onClose} busy={memoryBusy} width="medium"
    actions={<>
      {tutor.detail ? <Button variant="dangerGhost" className="tw:mr-auto" disabled={locked || memoryBusy} onClick={() => { void onDelete(); }}>Delete conversation</Button> : null}
      <Button disabled={locked || memoryBusy} onClick={() => { void tutor.initialize(); void tutor.refresh(); }}>Refresh saved state</Button>
    </>}>
    <p className={styles.muted}>Chat uses your Tutor model from Settings → AI/LLM. Search uses the selected embedding model. Switching the chat model does not reinterpret saved documents.</p>
    {tutor.conversationRetrievalAllowed ? <label className={styles.checkbox}>
      <Checkbox checked={tutor.includeConversations} disabled={locked || memoryBusy} onCheckedChange={tutor.setIncludeConversations} />
      Search permitted conversations in this material selection
    </label> : null}
    {tutor.detail ? <TutorMemoryControls conversationId={tutor.detail.conversation.id} revision={tutor.detail.conversation.revision}
      retrievalAvailable={tutor.conversationRetrievalAllowed} disabled={locked} onBusyChange={setMemoryBusy} onChanged={tutor.refresh} /> : null}
    {tutor.scope.kind !== "general" ? <MaterialPreparation controller={tutor.preparation} disabled={memoryBusy || tutor.busy || tutor.loading || tutor.pending !== null} /> : null}
  </Dialog>;
}
