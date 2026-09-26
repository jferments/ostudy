import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const html = readFileSync("index.html", "utf8");
const native = readFileSync("src-tauri/src/startup.rs", "utf8");

describe("first-paint startup shell", () => {
  it("provides a complete loading view before any application JavaScript or stylesheet", () => {
    const parsed = new DOMParser().parseFromString(html, "text/html");
    const shell = parsed.querySelector("#root > #ostudy-boot");
    expect(shell?.getAttribute("role")).toBe("status");
    expect(shell?.textContent).toContain("Opening ostudy");
    expect(shell?.textContent).toContain("Preparing your library");
    expect(parsed.querySelector("head #ostudy-boot-style")?.textContent).toContain("#0d1117");
    expect(html.indexOf('id="ostudy-boot"')).toBeLessThan(html.indexOf('type="module"'));
    expect(shell?.querySelector("img, link, script")).toBeNull();
  });

  it("gives the base and Linux native windows the same opaque first-paint background", () => {
    for (const path of ["src-tauri/tauri.conf.json", "src-tauri/tauri.linux.conf.json"]) {
      const config = JSON.parse(readFileSync(path, "utf8"));
      const window = config.app.windows.find((entry: { label: string }) => entry.label === "main");
      expect(window.backgroundColor).toBe("#0d1117");
      expect(window.transparent).toBe(false);
    }
  });

  it("dispatches durable initialization off the event loop and publishes readiness last", () => {
    const setup = native.slice(native.indexOf("pub(crate) fn setup"), native.indexOf("fn initialize("));
    expect(setup).toContain("tauri::async_runtime::spawn_blocking");
    expect(setup.indexOf("spawn_blocking")).toBeLessThan(setup.indexOf("initialize(&worker)"));
    expect(setup).toContain("present_startup_failure(&handle");
    const initializeStart = native.indexOf("fn initialize(");
    const reconcileStart = native.indexOf("pub(crate) fn reconcile_library(");
    const initialization = native.slice(initializeStart, reconcileStart);
    const reconciliation = native.slice(
      reconcileStart,
      native.indexOf("#[cfg(target_os = \"linux\")]", reconcileStart),
    );
    const publishDatabase = initialization.indexOf("app.manage(database)");
    const reconcileCall = initialization.indexOf("reconcile_library(&database, &paths)");
    expect(reconcileCall).toBeGreaterThan(-1);
    expect(reconciliation).toContain("crate::tutor::reconcile_startup");
    expect(publishDatabase).toBeGreaterThan(initialization.indexOf("app.manage(ai_runtime)"));
    expect(publishDatabase).toBeGreaterThan(initialization.indexOf("app.manage(pdf_render_runtime)"));
    expect(publishDatabase).toBeGreaterThan(reconcileCall);
    expect(publishDatabase).toBeGreaterThan(initialization.indexOf("logging::flush"));
  });
});
