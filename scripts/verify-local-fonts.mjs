#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { localFontSources } from "./font-sources.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const requireInstalled = process.argv.includes("--installed");
const failures = [];

const [appearance, controls, rustSettings, localCss, foundations, packageJson, gitignore] =
  await Promise.all([
    read("src/ui/appearance/appearanceTypes.ts"),
    read("src/screens/settings/SettingsControls.tsx"),
    read("src-tauri/src/settings.rs"),
    read("src/ui/foundations/local-fonts.css"),
    read("src/ui/foundations/index.css"),
    read("package.json"),
    read(".gitignore"),
  ]);

for (const font of localFontSources) {
  expectContains(appearance, `value: "${font.id}"`, `appearance option ${font.id}`);
  expectContains(appearance, `'${font.family}'`, `appearance family ${font.family}`);
  expectContains(rustSettings, `"${font.id}"`, `Rust font preset ${font.id}`);
  expectContains(localCss, `font-family: "${font.family}";`, `@font-face ${font.family}`);

  for (const face of font.faces) {
    expectContains(localCss, face.filename, `local font asset ${face.filename}`);
    if (requireInstalled) {
      const path = join(root, "src", "assets", "fonts", face.filename);
      try {
        const bytes = await readFile(path);
        if (bytes.length < 4 || bytes.subarray(0, 4).toString("ascii") !== "wOF2") {
          failures.push(`Installed font is not a valid WOFF2 file: ${face.filename}`);
        }
      } catch {
        failures.push(`Installed local font is missing: ${face.filename}`);
      }
    }
  }
}

expectContains(controls, "FONT_PRESET_OPTIONS", "shared Appearance font option registry");
expectContains(foundations, '@import "./local-fonts.css"', "local font stylesheet import");
expectContains(packageJson, '"postinstall": "node ./scripts/install-local-fonts.mjs"', "font installer lifecycle");
expectContains(packageJson, '"prebuild": "node ./scripts/install-local-fonts.mjs"', "build font guard");
expectContains(packageJson, '"predev": "node ./scripts/install-local-fonts.mjs"', "development font guard");
expectContains(gitignore, "src/assets/fonts/*.woff2", "downloaded font ignore rule");

if (failures.length > 0) {
  console.error("Local font contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  requireInstalled
    ? "OK   Selectable local font assets are installed and valid WOFF2 files."
    : "OK   Selectable local font manifests, CSS, settings, and installer contracts agree.",
);

async function read(relativePath) {
  return readFile(join(root, relativePath), "utf8");
}

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}
