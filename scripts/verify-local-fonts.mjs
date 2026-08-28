#!/usr/bin/env node

import { readFile, readdir } from "node:fs/promises";
import { basename, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { localFontSources } from "./font-sources.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const requireInstalled = process.argv.includes("--installed");
const packagedDirectoryIndex = process.argv.indexOf("--packaged-dir");
const requirePackaged = process.argv.includes("--packaged") || packagedDirectoryIndex >= 0;
const packagedDirectory =
  packagedDirectoryIndex >= 0
    ? resolve(process.argv[packagedDirectoryIndex + 1] ?? "")
    : join(root, "dist");
const failures = [];

if (packagedDirectoryIndex >= 0 && !process.argv[packagedDirectoryIndex + 1]) {
  failures.push("--packaged-dir requires a directory path.");
}

const plex = localFontSources.find((font) => font.id === "plex");
if (!plex?.license) {
  failures.push("IBM Plex Sans must declare audited license metadata.");
}

const [
  appearance,
  provider,
  controls,
  controller,
  settingsScreen,
  rustSettings,
  localCss,
  foundations,
  documentCss,
  richContentCss,
  packageJson,
  gitignore,
  checkState,
  checkStatic,
  checkFast,
  checkFull,
  licenseNotice,
] = await Promise.all([
  read("src/ui/appearance/appearanceTypes.ts"),
  read("src/ui/appearance/AppearanceProvider.tsx"),
  read("src/screens/settings/SettingsControls.tsx"),
  read("src/controllers/useAppController.ts"),
  read("src/screens/SettingsScreen.tsx"),
  read("src-tauri/src/settings.rs"),
  read("src/ui/foundations/local-fonts.css"),
  read("src/ui/foundations/index.css"),
  read("src/ui/foundations/document.css"),
  read("src/editor/RichContent.module.css"),
  read("package.json"),
  read(".gitignore"),
  read("scripts/check-state.sh"),
  read("scripts/check-static.sh"),
  read("scripts/check-fast.sh"),
  read("scripts/check.sh"),
  plex?.license ? read(plex.license.noticePath) : Promise.resolve(""),
]);

for (const font of localFontSources) {
  expectContains(appearance, `value: "${font.id}"`, `appearance option ${font.id}`);
  expectContains(appearance, `'${font.family}'`, `appearance family ${font.family}`);
  expectContains(rustSettings, `"${font.id}"`, `Rust font preset ${font.id}`);
  expectContains(localCss, `font-family: "${font.family}";`, `@font-face ${font.family}`);

  for (const face of font.faces) {
    expectContains(localCss, face.filename, `local font asset ${face.filename}`);
    if (requireInstalled) {
      await verifyWoff2(
        join(root, "src", "assets", "fonts", face.filename),
        `Installed local font ${face.filename}`,
      );
    }
  }
}

expectContains(
  appearance,
  'export const DEFAULT_FONT_PRESET = "plex"',
  "shared IBM Plex default preset",
);
expectContains(
  appearance,
  "'IBM Plex Sans', var(--ostudy-font-family-system)",
  "IBM Plex missing-face fallback stack",
);
expectContains(provider, 'setProperty("--ui-font-family"', "semantic interface font publication");
expectContains(provider, '"--card-font-family"', "semantic card font publication");
expectContains(controller, "uiFont: DEFAULT_FONT_PRESET", "controller interface font default");
expectContains(controller, "cardFont: DEFAULT_FONT_PRESET", "controller card font default");
expectContains(settingsScreen, "uiFont: DEFAULT_FONT_PRESET", "Settings interface font default");
expectContains(settingsScreen, "cardFont: DEFAULT_FONT_PRESET", "Settings card font default");
expectContains(
  documentCss,
  '--ui-font-family: "IBM Plex Sans", var(--ostudy-font-family-system)',
  "foundation interface font fallback",
);
expectContains(
  documentCss,
  '--card-font-family: "IBM Plex Sans", var(--ostudy-font-family-system)',
  "foundation card font fallback",
);
expectContains(
  richContentCss,
  "font-family: var(--card-font-family)",
  "card typography semantic consumer",
);
expectContains(controls, "FONT_PRESET_OPTIONS", "shared Appearance font option registry");
expectContains(foundations, '@import "./local-fonts.css"', "local font stylesheet import");
expectContains(
  packageJson,
  '"postinstall": "node ./scripts/install-local-fonts.mjs"',
  "font installer lifecycle",
);
expectContains(
  packageJson,
  '"prebuild": "node ./scripts/install-local-fonts.mjs"',
  "build font guard",
);
expectContains(
  packageJson,
  '"postbuild": "node ./scripts/verify-local-fonts.mjs --packaged"',
  "packaged font guard",
);
expectContains(
  packageJson,
  '"predev": "node ./scripts/install-local-fonts.mjs"',
  "development font guard",
);
expectContains(gitignore, "src/assets/fonts/*.woff2", "downloaded font ignore rule");

expectDoesNotMatch(
  localCss,
  /https?:\/\/|fonts\.(?:googleapis|gstatic)\.com/iu,
  "runtime remote font reference in local CSS",
);

if (plex?.license) {
  expectContains(plex.license.spdx, "OFL-1.1", "IBM Plex Sans SPDX license");
  expectContains(licenseNotice, plex.license.copyright, "IBM Plex Sans copyright notice");
  expectContains(licenseNotice, "SIL OPEN FONT LICENSE Version 1.1", "IBM Plex Sans OFL text");
  expectContains(licenseNotice, "PERMISSION & CONDITIONS", "complete IBM Plex Sans OFL terms");
}

expectContains(
  checkState,
  "ostudy_local_fonts_present()",
  "qualification-state local-font prerequisite",
);
expectContains(
  checkState,
  'verify-local-fonts.mjs" --installed',
  "qualification-state installed-font check",
);
expectContains(
  checkState,
  "ostudy_frontend_prerequisites_present()",
  "combined frontend prerequisite guard",
);
for (const [source, label] of [
  [checkStatic, "static qualification reuse"],
  [checkFast, "fast qualification reuse"],
  [checkFull, "full qualification reuse"],
]) {
  expectContains(source, "ostudy_frontend_prerequisites_present", label);
}
expectContains(checkStatic, "pnpm run fonts:install", "explicit static-gate font provisioning");
expectBefore(
  checkStatic,
  "pnpm run fonts:install",
  "pnpm run verify:fonts",
  "font provisioning before installed-font verification",
);

if (requirePackaged) await verifyPackagedFonts(packagedDirectory, licenseNotice);

if (failures.length > 0) {
  console.error("Local font contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

const verified = [
  "selectable local font manifests, defaults, CSS, settings, installer, fallback, and license contracts",
];
if (requireInstalled) verified.push("installed WOFF2 assets");
if (requirePackaged) verified.push("hashed packaged WOFF2 assets and OFL notice");
console.log(`OK   Verified ${verified.join("; ")}.`);

async function verifyPackagedFonts(directory, sourceLicenseNotice) {
  const packagedFiles = await collectFiles(directory);
  const cssFiles = packagedFiles.filter((path) => extname(path) === ".css");
  const htmlFiles = packagedFiles.filter((path) => extname(path) === ".html");
  if (packagedFiles.length === 0) {
    failures.push(`Packaged frontend directory is missing or empty: ${directory}`);
    return;
  }
  if (cssFiles.length === 0) {
    failures.push(`Packaged frontend contains no CSS assets: ${directory}`);
    return;
  }

  const packagedCss = (await Promise.all(cssFiles.map((path) => readFile(path, "utf8")))).join("\n");
  const packagedHtml = (await Promise.all(htmlFiles.map((path) => readFile(path, "utf8")))).join("\n");
  expectDoesNotMatch(
    `${packagedCss}\n${packagedHtml}`,
    /fonts\.(?:googleapis|gstatic)\.com|@import\s+(?:url\()?['"]?https?:\/\//iu,
    "packaged runtime remote-font reference",
  );
  expectContains(packagedCss, "IBM Plex Sans", "packaged IBM Plex Sans @font-face/default");
  expectContains(
    packagedCss,
    '--ui-font-family: "IBM Plex Sans", var(--ostudy-font-family-system)',
    "packaged interface missing-face fallback",
  );
  expectContains(
    packagedCss,
    '--card-font-family: "IBM Plex Sans", var(--ostudy-font-family-system)',
    "packaged card missing-face fallback",
  );

  for (const font of localFontSources) {
    for (const face of font.faces) {
      const extension = extname(face.filename);
      const stem = basename(face.filename, extension);
      const hashedName = new RegExp(
        `^${escapeRegex(stem)}-[A-Za-z0-9_-]+\\${extension}$`,
        "u",
      );
      const matches = packagedFiles.filter((path) => hashedName.test(basename(path)));
      if (matches.length !== 1) {
        failures.push(
          `Expected one hashed packaged asset for ${face.filename}, found ${matches.length}.`,
        );
        continue;
      }
      await verifyWoff2(matches[0], `Packaged local font ${basename(matches[0])}`);
      expectContains(
        packagedCss,
        basename(matches[0]),
        `packaged CSS reference ${basename(matches[0])}`,
      );
    }
  }

  if (plex?.license) {
    const packagedLicensePath = join(directory, "licenses", basename(plex.license.noticePath));
    try {
      const packagedLicense = await readFile(packagedLicensePath, "utf8");
      if (packagedLicense !== sourceLicenseNotice) {
        failures.push("Packaged IBM Plex Sans license notice does not match the audited source notice.");
      }
    } catch {
      failures.push(`Packaged IBM Plex Sans license notice is missing: ${packagedLicensePath}`);
    }
  }
}

async function verifyWoff2(path, label) {
  try {
    const bytes = await readFile(path);
    if (bytes.length < 4 || bytes.subarray(0, 4).toString("ascii") !== "wOF2") {
      failures.push(`${label} is not a valid WOFF2 file.`);
    }
  } catch {
    failures.push(`${label} is missing.`);
  }
}

async function collectFiles(directory) {
  try {
    const entries = await readdir(directory, { withFileTypes: true });
    const nested = await Promise.all(
      entries.map((entry) => {
        const path = join(directory, entry.name);
        return entry.isDirectory() ? collectFiles(path) : Promise.resolve([path]);
      }),
    );
    return nested.flat();
  } catch {
    return [];
  }
}

async function read(relativePath) {
  return readFile(join(root, relativePath), "utf8");
}

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}

function expectDoesNotMatch(source, pattern, label) {
  if (pattern.test(source)) failures.push(`Found forbidden ${label}: ${pattern}`);
}

function expectBefore(source, first, second, label) {
  const firstIndex = source.indexOf(first);
  const secondIndex = source.indexOf(second);
  if (firstIndex < 0 || secondIndex < 0 || firstIndex >= secondIndex) {
    failures.push(`Invalid ${label}: expected ${first} before ${second}`);
  }
}

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}
