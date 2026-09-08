import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { changeTutorMemory, getTutorMemory, type TutorMemory } from "../../ipc/tutorMemory";
import { TutorMemoryControls } from "./TutorMemoryControls";

vi.mock("../../ipc/tutorMemory", () => ({ getTutorMemory: vi.fn(), changeTutorMemory: vi.fn() }));
vi.mock("../../ui/overlays/overlayContext", () => ({ useConfirm: () => async () => true }));
const get = vi.mocked(getTutorMemory);
const change = vi.mocked(changeTutorMemory);
const memory: TutorMemory = {
  conversationId: "a".repeat(32), revision: 3, historyEnabled: true, retrievalEnabled: false,
  startSequence: 0, summary: "An earlier topic", summaryThroughSequence: 2,
};

beforeEach(() => {
  vi.clearAllMocks();
  get.mockResolvedValue(memory);
});

function mount(onChanged = vi.fn(async () => {})) {
  const onBusyChange = vi.fn();
  render(<TutorMemoryControls conversationId={memory.conversationId} revision={3} retrievalAvailable={true} disabled={false}
    onChanged={onChanged} onBusyChange={onBusyChange} />);
  fireEvent.click(screen.getByText("Conversation memory"));
  return { onChanged, onBusyChange };
}

describe("conversation memory controls", () => {
  it("loads locally without changing sharing or sending provider requests", async () => {
    mount();
    await screen.findByText("An earlier topic");
    expect(get).toHaveBeenCalledWith(memory.conversationId);
    expect(change).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Allow this conversation in semantic retrieval")).not.toBeChecked();
  });

  it("keeps the exact operation for an ambiguous retry", async () => {
    change.mockRejectedValueOnce(new Error("IPC disconnected"));
    change.mockResolvedValueOnce({ ...memory, revision: 4, retrievalEnabled: true });
    const callbacks = mount();
    await screen.findByText("An earlier topic");
    fireEvent.click(screen.getByLabelText("Allow this conversation in semantic retrieval"));
    await screen.findByRole("button", { name: "Check memory change result" });
    const original = change.mock.calls[0][0];
    expect(original.action).toBe("allow_retrieval");
    expect(original.expectedRevision).toBe(3);
    fireEvent.click(screen.getByRole("button", { name: "Check memory change result" }));
    await waitFor(() => expect(callbacks.onChanged).toHaveBeenCalledTimes(1));
    expect(change.mock.calls[1][0]).toEqual(original);
  });

  it("does not replay a saved reset because the conversation refresh failed", async () => {
    change.mockResolvedValueOnce({ ...memory, revision: 4, startSequence: 3, summary: "", summaryThroughSequence: 3 });
    mount(vi.fn(async () => { throw new Error("refresh unavailable"); }));
    await screen.findByText("An earlier topic");
    fireEvent.click(screen.getByRole("button", { name: "Forget earlier context" }));
    await screen.findByText(/Memory settings were saved/);
    expect(change).toHaveBeenCalledTimes(1);
    expect(change.mock.calls[0][0].action).toBe("forget_earlier");
    expect(screen.queryByRole("button", { name: "Check memory change result" })).not.toBeInTheDocument();
  });
});
