import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as externalFiles from "../../ipc/externalFiles";
import * as imageSets from "../../ipc/imageSets";
import * as platformFiles from "../../platform/files";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { ImageSetWorkspace } from "./ImageSetWorkspace";

vi.mock("../../platform/files", () => ({ selectExternalImport: vi.fn() }));
vi.mock("../../ipc/externalFiles", async (importOriginal) => {
  const original = await importOriginal<typeof import("../../ipc/externalFiles")>();
  return { ...original, releaseStagedExternalImport: vi.fn() };
});
vi.mock("../../ipc/imageSets", () => ({
  inspectImageSet: vi.fn(),
  importImageSet: vi.fn(),
  getImageSet: vi.fn(),
  getImageSetPreview: vi.fn(),
  prepareImageSet: vi.fn(),
  amendImageSet: vi.fn(),
  setImageSetFilters: vi.fn(),
}));

const documentId = "a".repeat(32);
const memberId = "b".repeat(32);
const regionId = "c".repeat(64);

function staged() {
  return {
    id: "1".repeat(32),
    purpose: "image" as const,
    detectedKind: "png" as const,
    displayName: "lecture-01.png",
    mediaType: "image/png",
    byteSize: 96,
    sha256: "d".repeat(64),
    expiresAt: Date.now() + 60_000,
  };
}

function inspection(ok: boolean): imageSets.ImageSetInspection {
  return {
    maximumCount: 128,
    maximumSetBytes: 512 * 1024 * 1024,
    maximumSetPixels: 128 * 16_000_000,
    canImport: ok,
    decodedImages: ok ? 1 : 0,
    reusedImages: 0,
    members: [{
      id: staged().id,
      image: ok ? {
        originalFilename: "lecture-01.png",
        mediaType: "image/png",
        byteSize: 96,
        sha256: "d".repeat(64),
        width: 800,
        height: 600,
        pixelSha256: "e".repeat(64),
        pixelContract: "fixture-v1",
      } : null,
      error: ok ? null : { code: "invalid_image", message: "This selection is not a readable image." },
    }],
  };
}

function details(overrides: Partial<imageSets.ImageSetDetails> = {}): imageSets.ImageSetDetails {
  return {
    documentId,
    title: "Lecture screenshots",
    revision: 3,
    filterRevision: 2,
    includeInterface: false,
    prepared: true,
    members: [{
      id: memberId,
      originalFilename: "lecture-01.png",
      mediaType: "image/png",
      byteSize: 96,
      width: 800,
      height: 600,
      quarterTurns: 0,
      inclusion: null,
      processing: {
        memberId,
        width: 800,
        height: 600,
        ocrAvailable: true,
        visualWithheld: false,
        regions: [{
          id: regionId,
          text: "Accept all cookies",
          bounds: null,
          automaticRole: "interface",
          role: "interface",
          reason: "A standalone interface label at the image edge is excluded.",
          overrideAction: null,
          equivalentTo: null,
        }],
      },
    }],
    ...overrides,
  };
}

function renderWorkspace(
  documentIdValue: string | null,
  onImported = vi.fn(),
  onChanged = vi.fn().mockResolvedValue(undefined),
  showToast = vi.fn(),
) {
  render(
    <OverlayProvider>
      <ImageSetWorkspace
        documentId={documentIdValue}
        showToast={showToast}
        onImported={onImported}
        onChanged={onChanged}
        onClose={vi.fn()}
      />
    </OverlayProvider>,
  );
  return { onImported, onChanged, showToast };
}

describe("ImageSetWorkspace", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(externalFiles.releaseStagedExternalImport).mockResolvedValue(undefined);
    vi.mocked(imageSets.prepareImageSet).mockResolvedValue({
      documentId,
      revision: 3,
      filterRevision: 2,
    });
    vi.mocked(imageSets.getImageSetPreview).mockResolvedValue({
      revision: 3,
      width: 800,
      height: 600,
      dataUrl: "data:image/png;base64,fixture",
    });
  });

  it("keeps a failed selection visible for retry before an ordered import", async () => {
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue(staged());
    vi.mocked(imageSets.inspectImageSet)
      .mockResolvedValueOnce(inspection(false))
      .mockResolvedValueOnce(inspection(true));
    vi.mocked(imageSets.importImageSet).mockResolvedValue({ documentId, revision: 1 });
    const { onImported, onChanged } = renderWorkspace(null);

    fireEvent.click(screen.getByRole("button", { name: "Add image" }));
    expect(await screen.findByText("This selection is not a readable image.")).toBeTruthy();
    expect(
      (screen.getByRole("button", { name: "Import image set" }) as HTMLButtonElement).disabled,
    ).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Retry inspection" }));
    expect(await screen.findByText(/800 × 600/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Import image set" }));

    await waitFor(() => {
      expect(imageSets.importImageSet).toHaveBeenCalledTimes(1);
      const input = vi.mocked(imageSets.importImageSet).mock.calls[0]?.[0];
      expect(input?.title).toBe("lecture-01");
      expect(input?.operationId).toHaveLength(32);
      expect(input?.members).toEqual([{
        stagedFileId: staged().id,
        quarterTurns: 0,
        inclusion: null,
      }]);
      expect(imageSets.prepareImageSet).toHaveBeenCalledWith(documentId, false);
      expect(onImported).toHaveBeenCalledWith(documentId);
      expect(onChanged).toHaveBeenCalled();
    });
    expect(externalFiles.releaseStagedExternalImport).not.toHaveBeenCalled();
  });

  it("inspects the retained original and saves source transforms before local preparation", async () => {
    vi.mocked(imageSets.getImageSet)
      .mockResolvedValueOnce(details())
      .mockResolvedValue(details({
        revision: 4,
        members: [{ ...details().members[0]!, quarterTurns: 1 }],
      }));
    vi.mocked(imageSets.amendImageSet).mockResolvedValue({
      documentId,
      revision: 4,
      filterRevision: 2,
    });
    renderWorkspace(documentId);

    const image = await screen.findByRole("img", { name: "lecture-01.png" });
    expect(image.getAttribute("src")).toContain("data:image/png;base64");
    fireEvent.click(screen.getByRole("button", { name: "Rotate right" }));
    fireEvent.click(screen.getByRole("button", { name: "Save source changes" }));

    await waitFor(() => {
      const input = vi.mocked(imageSets.amendImageSet).mock.calls[0]?.[0];
      expect(input?.documentId).toBe(documentId);
      expect(input?.expectedRevision).toBe(3);
      expect(input?.expectedFilterRevision).toBe(2);
      expect(input?.members[0]?.quarterTurns).toBe(1);
      expect(imageSets.prepareImageSet).toHaveBeenCalledWith(documentId, false);
    });
  });

  it("publishes explicit Content-filter choices and reapplies the local projection", async () => {
    vi.mocked(imageSets.getImageSet).mockResolvedValue(details());
    vi.mocked(imageSets.setImageSetFilters).mockResolvedValue({
      documentId,
      revision: 3,
      filterRevision: 3,
    });
    renderWorkspace(documentId);

    const filter = await screen.findByRole("combobox", {
      name: "Content filter for Accept all cookies",
    });
    fireEvent.change(filter, { target: { value: "include" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Content filter" }));

    await waitFor(() => {
      const input = vi.mocked(imageSets.setImageSetFilters).mock.calls[0]?.[0];
      expect(input?.operationId).toHaveLength(32);
      expect(input?.expectedRevision).toBe(3);
      expect(input?.expectedFilterRevision).toBe(2);
      expect(input?.overrides).toEqual([{ regionId, action: "include" }]);
      expect(imageSets.prepareImageSet).toHaveBeenCalledWith(documentId, false);
    });
    const filterSection = screen.getByRole("region", { name: "Content filter" });
    expect(within(filterSection).getByText(/Automatic: interface/u)).toBeTruthy();
  });
});
