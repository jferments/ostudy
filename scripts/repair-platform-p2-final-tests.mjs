#!/usr/bin/env node

import { readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const repairPath = join(root, "scripts", "repair-platform-p2-green-tests.mjs");

await import(new URL("./repair-platform-p2-green-tests.mjs", import.meta.url));

const decksTestPath = join(root, "src", "screens", "DecksScreen.extended.test.tsx");
let decksTest = await readFile(decksTestPath, "utf8");
const oldCanceledTest = `  it("does not report success when export publication is canceled", async () => {
    vi.mocked(platformFiles.publishExport).mockResolvedValue(null);
    const { showToast } = renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Export" }));

    await waitFor(() => {
      expect(decksIpc.prepareDeckExport).toHaveBeenCalledOnce();
      expect(platformFiles.publishExport).toHaveBeenCalledWith("2".repeat(32));
    });
    expect(showToast).not.toHaveBeenCalledWith("success", expect.any(String));
  });`;
const newCanceledTest = `  it("does not report success when export publication is canceled", async () => {
    vi.mocked(decksIpc.prepareDeckExport).mockResolvedValue({
      preparedExport: {
        id: "2".repeat(32),
        purpose: "studydeck",
        suggestedFilename: "cardiology.studydeck",
        mediaType: "application/vnd.ostudy.studydeck",
        byteSize: 128,
        sha256: "0".repeat(64),
        expiresAt: Date.now() + 60_000,
      },
      cardCount: 1,
      assetCount: 0,
    });
    vi.mocked(platformFiles.publishExport).mockResolvedValue(null);
    const { showToast } = renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Export" }));

    await waitFor(() => {
      expect(decksIpc.prepareDeckExport).toHaveBeenCalledOnce();
      expect(platformFiles.publishExport).toHaveBeenCalledWith("2".repeat(32));
    });
    expect(showToast).not.toHaveBeenCalledWith("success", expect.any(String));
  });`;
if (!decksTest.includes(oldCanceledTest)) {
  throw new Error("The canceled prepared-export test was not found after final repair");
}
decksTest = decksTest.replace(oldCanceledTest, newCanceledTest);
await writeFile(decksTestPath, decksTest, "utf8");

await rm(repairPath, { force: true });

console.log("Executed deterministic final P0-P2 test repairs.");
