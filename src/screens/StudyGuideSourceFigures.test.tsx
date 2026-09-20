import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";

import * as guideIpc from "../ipc/studyGuides";
import {
  detectedImageMimeType,
  selectFigureAssetIds,
  StudyGuideSourceFigures,
} from "./StudyGuideSourceFigures";

vi.mock("../ipc/studyGuides", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../ipc/studyGuides")>();
  return { ...actual, readDocumentSourceAsset: vi.fn() };
});

beforeEach(() => {
  vi.resetAllMocks();
  vi.stubGlobal("URL", {
    createObjectURL: vi.fn(() => "blob:figure"),
    revokeObjectURL: vi.fn(),
  });
});

it("matches native bounded asset selection deterministically", () => {
  expect(selectFigureAssetIds(["c", "a", "b", "c"], 2)).toEqual(["a", "c"]);
  expect(selectFigureAssetIds(["c", "a", "b"], 1)).toEqual(["b"]);
  expect(selectFigureAssetIds(["b", "a"], 4)).toEqual(["a", "b"]);
});

it("detects supported canonical image containers", () => {
  expect(detectedImageMimeType(Uint8Array.from([0x89, 0x50, 0x4e, 0x47, 13, 10, 26, 10]).buffer))
    .toBe("image/png");
  expect(detectedImageMimeType(Uint8Array.from([0xff, 0xd8, 0xff]).buffer)).toBe("image/jpeg");
  expect(detectedImageMimeType(new TextEncoder().encode("RIFF0000WEBP").buffer)).toBe("image/webp");
  expect(detectedImageMimeType(Uint8Array.from([0x49, 0x49, 0x2a, 0x00]).buffer)).toBe("image/tiff");
  expect(detectedImageMimeType(new TextEncoder().encode("not-image").buffer)).toBeNull();
});

it("renders a verified source figure and keeps the explanation as its caption", async () => {
  vi.mocked(guideIpc.readDocumentSourceAsset).mockResolvedValue(
    Uint8Array.from([0x89, 0x50, 0x4e, 0x47, 13, 10, 26, 10]).buffer,
  );
  render(
    <StudyGuideSourceFigures
      documentId={"d".repeat(32)}
      locator={{ unitKind: "page", unitNumbers: [2], assetIds: ["figure-a"] }}
      explanations={[{ title: "Pressure-volume loop", explanation: "Trace filling before ejection." }]}
    />,
  );
  await waitFor(() => expect(guideIpc.readDocumentSourceAsset).toHaveBeenCalledWith(
    "d".repeat(32),
    "figure-a",
  ));
  const image = await screen.findByRole("img", { name: "Pressure-volume loop" });
  expect(image.getAttribute("src")).toBe("blob:figure");
  expect(screen.getByText("Trace filling before ejection.")).toBeTruthy();
});
