import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import { build } from "vite";
import assert from "node:assert/strict";
import test from "node:test";

const root = fileURLToPath(new URL("../../", import.meta.url));

const migratedSources = [
  "src/ui/vendor/shadcn/button.tsx",
  "src/ui/vendor/shadcn/badge.tsx",
  "src/ui/vendor/shadcn/skeleton.tsx",
  "src/ui/vendor/shadcn/input.tsx",
  "src/ui/vendor/shadcn/native-select.tsx",
  "src/ui/vendor/shadcn/form-controls.tsx",
  "src/ui/vendor/shadcn/tabs.tsx",
  "src/ui/vendor/shadcn/card.tsx",
  "src/ui/primitives/FormControls.tsx",
  "src/ui/primitives/Button.tsx",
  "src/ui/primitives/Panel.tsx",
  "src/screens/SettingsScreen.tsx",
  "src/screens/settings/SettingsControls.tsx",
  "src/screens/settings/AppearanceSettings.tsx",
  "src/screens/settings/StorageBackupSettings.tsx",
  "src/screens/settings/ReviewSchedulingSettings.tsx",
  "src/screens/settings/DiagnosticsSettings.tsx",
  "src/screens/settings/DeviceSyncSettings.tsx",
  "src/screens/settings/AiLlmSettings.tsx",
  "src/screens/settings/DefaultAiModelSettings.tsx",
];

test("compiles every migrated utility, all themes and capability variants without writing source", async () => {
  const result = await build({
    configFile: false,
    root,
    publicDir: false,
    logLevel: "silent",
    plugins: [tailwindcss()],
    build: {
      write: false,
      minify: false,
      cssMinify: false,
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
  for (const theme of ["light", "dark", "console", "synthwave"]) {
    assert.match(css, new RegExp(`data-theme=["']?${theme}`));
  }
  assert.ok(css.includes("var(--ostudy-color-accent-on)"));
  assert.ok(css.includes("var(--ostudy-color-status-danger-on)"));
  assert.ok(css.includes("prefers-reduced-motion"));
  assert.ok(css.includes("pointer: coarse"));
  assert.ok(css.includes("ostudy-skeleton"));
  for (const file of migratedSources) {
    const source = await readFile(new URL(`../../${file}`, import.meta.url), "utf8");
    const candidates = new Set(source.match(/tw:[^\s"'`]+/gu));
    for (const candidate of candidates) {
      // Group/peer names are selector markers, not declarations in their own right.
      if (/^tw:(?:group|peer)(?:\/[-\w]+)?$/u.test(candidate)) continue;
      const escaped = candidate.replace(/[^a-zA-Z0-9_-]/gu, (character) => `\\${character}`);
      assert.ok(css.includes(`.${escaped}`), `${file}: missing compiled utility ${candidate}`);
    }
  }
  const foundation = await readFile(new URL("../../src/ui/foundations/index.css", import.meta.url), "utf8");
  assert.doesNotMatch(foundation, /@import\s+["']tailwindcss(?:["']|\/preflight)/u);
  assert.ok(foundation.includes('layer(components) source("../..") prefix(tw)'));
});
