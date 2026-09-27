import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { IncomingShareSummary } from "../ipc/externalFiles";
import { IncomingShareCoordinator } from "./IncomingShareCoordinator";

const mocks = vi.hoisted(() => ({
  confirm: vi.fn(), list: vi.fn(), listen: vi.fn(), dismiss: vi.fn(), stage: vi.fn(),
  release: vi.fn(), importDeck: vi.fn(), importGenerated: vi.fn(), restore: vi.fn(),
  refresh: vi.fn(), toast: vi.fn(), report: vi.fn(), diagnostic: vi.fn(),
}));
vi.mock("../platform/PlatformProvider", () => ({ usePlatformCapabilities: () => ({ native: { operatingSystem: "android" } }) }));
vi.mock("../ui/overlays/overlayContext", () => ({ useConfirm: () => mocks.confirm }));
vi.mock("../ipc/platform", () => ({ listenToIncomingShare: mocks.listen }));
vi.mock("../ipc/externalFiles", () => ({
  listIncomingShares: mocks.list, dismissIncomingShare: mocks.dismiss,
  stageIncomingShare: mocks.stage, releaseStagedExternalImport: mocks.release,
}));
vi.mock("../ipc/decks", () => ({ importDeck: mocks.importDeck }));
vi.mock("../ipc/generated-import", () => ({ importGeneratedFlashcards: mocks.importGenerated }));
vi.mock("../ipc/backup", () => ({ prepareLibraryRestore: mocks.restore }));
vi.mock("../ipc/diagnostics", () => ({ reportFrontendFailure: mocks.report, queueFrontendDiagnostic: mocks.diagnostic }));

function share(token: string): IncomingShareSummary {
  return { token, displayName: `${token}.studydeck`, byteSize: 42, reportedMediaType: null };
}
function deferred<T>() {
  let complete: ((value: T) => void) | undefined;
  const promise = new Promise<T>((resolve) => { complete = resolve; });
  return { promise, resolve(value: T) {
    if (!complete) throw new Error("Missing resolver");
    complete(value);
  } };
}
function mount() { return render(<IncomingShareCoordinator refreshDecks={mocks.refresh} showToast={mocks.toast} />); }
function receive(incoming: IncomingShareSummary) {
  const callback = mocks.listen.mock.calls[0]?.[0];
  if (!callback) throw new Error("Share listener missing");
  callback(incoming);
}

beforeEach(() => {
  vi.resetAllMocks();
  mocks.list.mockResolvedValue([]);
  mocks.listen.mockResolvedValue(vi.fn());
  mocks.confirm.mockResolvedValue(false);
  mocks.dismiss.mockResolvedValue(undefined);
  mocks.release.mockResolvedValue(undefined);
  mocks.refresh.mockResolvedValue(undefined);
});

describe("incoming share acquisition", () => {
  it("merges a late initial snapshot without losing live queued shares or repeating a handled one", async () => {
    const snapshot = deferred<IncomingShareSummary[]>();
    mocks.list.mockReturnValue(snapshot.promise);
    const view = mount();
    await act(async () => receive(share("live")));
    await waitFor(() => expect(mocks.dismiss).toHaveBeenCalledWith("live"));
    await act(async () => snapshot.resolve([share("live"), share("older")]));
    await waitFor(() => expect(mocks.dismiss).toHaveBeenCalledWith("older"));
    expect(mocks.confirm).toHaveBeenCalledTimes(2);
    expect(mocks.dismiss).toHaveBeenCalledTimes(2);
    view.unmount();
  });

  it("does not overwrite an in-flight live share when the list response arrives", async () => {
    const snapshot = deferred<IncomingShareSummary[]>();
    const answer = deferred<boolean>();
    mocks.list.mockReturnValue(snapshot.promise);
    mocks.confirm.mockReturnValueOnce(answer.promise);
    const view = mount();
    await act(async () => receive(share("live")));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalledOnce());
    await act(async () => snapshot.resolve([share("older")]));
    await act(async () => answer.resolve(false));
    await waitFor(() => expect(mocks.dismiss).toHaveBeenCalledTimes(2));
    expect(mocks.dismiss.mock.calls.map(([token]) => token)).toEqual(["live", "older"]);
    view.unmount();
  });

  it("disposes a late registration and refuses late confirmation after unmount", async () => {
    const registration = deferred<() => void>();
    const answer = deferred<boolean>();
    mocks.listen.mockReturnValue(registration.promise);
    mocks.list.mockResolvedValue([share("pending")]);
    mocks.confirm.mockReturnValue(answer.promise);
    const view = mount();
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalledOnce());
    view.unmount();
    const unlisten = vi.fn();
    await act(async () => {
      registration.resolve(unlisten);
      receive(share("too-late"));
      answer.resolve(true);
    });
    expect(unlisten).toHaveBeenCalledOnce();
    expect(mocks.stage).not.toHaveBeenCalled();
    expect(mocks.confirm).toHaveBeenCalledOnce();
  });

  it("records caught list/listener failures instead of only writing the console", async () => {
    const failure = new Error("private platform details");
    mocks.list.mockRejectedValue(failure);
    mocks.listen.mockRejectedValue(failure);
    const view = mount();
    await waitFor(() => expect(mocks.report).toHaveBeenCalledTimes(2));
    expect(mocks.report).toHaveBeenCalledWith("incoming_share_list_failed", failure);
    expect(mocks.report).toHaveBeenCalledWith("incoming_share_listener_failed", failure);
    view.unmount();
  });

  it("never treats failed post-commit refresh as a failed import or releases committed staging", async () => {
    mocks.list.mockResolvedValue([share("committed")]);
    mocks.confirm.mockResolvedValue(true);
    mocks.stage.mockResolvedValue({ id: "a".repeat(32), detectedKind: "studydeck" });
    mocks.importDeck.mockResolvedValue({ cardCount: 2, deck: { name: "Copied" } });
    mocks.refresh.mockRejectedValue(new Error("Refresh failed"));
    const view = mount();
    await waitFor(() => expect(mocks.toast).toHaveBeenCalledWith("info", "The shared content was imported. Refresh the view to see it."));
    expect(mocks.importDeck).toHaveBeenCalledOnce();
    expect(mocks.release).not.toHaveBeenCalled();
    expect(mocks.toast).toHaveBeenCalledWith("success", "Imported 2 cards into Copied.");
    expect(mocks.diagnostic).toHaveBeenCalledWith(expect.objectContaining({ event: "incoming_share_reconciliation_failed" }));
    view.unmount();
  });
});
