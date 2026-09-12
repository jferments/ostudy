import { readFile, readdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import { build } from "vite";
import assert from "node:assert/strict";
import test from "node:test";

const rootUrl = new URL("../../", import.meta.url);
const root = fileURLToPath(rootUrl);
// The live Phase 4–8 owners, not a hand-maintained subset of control examples.
const directories = ["src/ui", "src/screens/settings", "src/screens/cards", "src/screens/flashcards"];
const entryFiles = [
  "src/App.tsx", "src/components/MobileAppBar.tsx", "src/components/NavigationSidebar.tsx",
  "src/components/AppErrorBoundary.tsx", "src/components/TagInput.tsx", "src/components/Toast.tsx",
  "src/screens/SettingsScreen.tsx", "src/screens/TemplatesScreen.tsx", "src/screens/templates/TemplateLayoutEditor.tsx",
];
async function sourceFiles(directory) {
  const result = [];
  for (const entry of await readdir(new URL(`${directory}/`, rootUrl), { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) result.push(...await sourceFiles(path));
    else if (/\.tsx?$/u.test(entry.name) && !/\.(?:test|spec)\.tsx?$/u.test(entry.name)) result.push(path);
  }
  return result;
}
const migratedSources = [...new Set([...entryFiles, ...(await Promise.all(directories.map(sourceFiles))).flat()])].sort();

test("compiles every migrated utility, all themes and capability variants without writing source", async () => {
  const result = await build({
    configFile: false, root, publicDir: false, logLevel: "silent", plugins: [tailwindcss()],
    build: {
      write: false, minify: false, cssMinify: false,
      lib: { entry: fileURLToPath(new URL("../ui/tailwind-entry.ts", import.meta.url)), formats: ["es"], name: "StyleContract" },
    },
  });
  const bundles = Array.isArray(result) ? result : [result];
  const css = bundles.flatMap((bundle) => "output" in bundle ? bundle.output : [])
    .filter((item) => item.type === "asset" && item.fileName.endsWith(".css"))
    .map((item) => item.type === "asset" ? (typeof item.source === "string" ? item.source : new TextDecoder().decode(item.source)) : "").join("\n");
  assert.ok(css.length > 1000, "Expected generated CSS");
  assert.ok(!css.includes("@theme"));
  assert.ok(!css.includes("@tailwind"));
  for (const theme of ["light", "dark", "console", "synthwave"]) assert.match(css, new RegExp(`data-theme=["']?${theme}`));
  assert.ok(css.includes("var(--ostudy-color-accent-on)"));
  assert.ok(css.includes("var(--ostudy-color-status-danger-on)"));
  assert.ok(css.includes("prefers-reduced-motion"));
  assert.ok(css.includes("pointer: coarse"));
  assert.ok(css.includes("ostudy-skeleton"));
  for (const file of migratedSources) {
    const source = await readFile(new URL(file, rootUrl), "utf8");
    const candidates = new Set(source.match(/tw:[^\s"'`]+/gu));
    for (const candidate of candidates) {
      if (/^tw:(?:group|peer)(?:\/[-\w]+)?$/u.test(candidate)) continue;
      const escaped = candidate.replace(/[^a-zA-Z0-9_-]/gu, (character) => `\\${character}`);
      assert.ok(css.includes(`.${escaped}`), `${file}: missing compiled utility ${candidate}`);
    }
  }
  const foundation = await readFile(new URL("src/ui/foundations/index.css", rootUrl), "utf8");
  assert.doesNotMatch(foundation, /@import\s+["']tailwindcss(?:["']|\/preflight)/u);
  assert.ok(foundation.includes('layer(components) source("../..") prefix(tw)'));
});

test("live migrated features use shared controls rather than another native-control vocabulary", async () => {
  for (const file of migratedSources) {
    if (file.startsWith("src/ui/")) continue;
    const source = await readFile(new URL(file, rootUrl), "utf8");
    assert.doesNotMatch(source, /<(?:button|input|select|textarea|details|summary)(?=[\s/>])/u,
      `${file}: use the shared oStudy control; keep genuinely special rendering behind an explicit adapter`);
  }
});

test("no-Preflight shell and overlay separators explicitly clear the other border edges", async () => {
  for (const file of ["src/ui/layout/AppFrame.tsx", "src/ui/layout/AppNavigation.tsx", "src/ui/vendor/shadcn/modal.tsx", "src/ui/overlays/OverflowMenu.tsx"]) {
    const source = await readFile(new URL(file, rootUrl), "utf8");
    for (const match of source.matchAll(/["']([^"']*tw:border-[bt](?:\s)[^"']*)["']/gu)) {
      assert.ok(match[1].includes("tw:border-0"), `${file}: partial border must not inherit medium browser-default edges`);
    }
  }
});
