#!/usr/bin/env node

import { access, readFile, readdir } from "node:fs/promises";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptRoot = fileURLToPath(new URL("..", import.meta.url));
const rootArgumentIndex = process.argv.indexOf("--root");
const root = resolve(
  rootArgumentIndex >= 0 && process.argv[rootArgumentIndex + 1]
    ? process.argv[rootArgumentIndex + 1]
    : scriptRoot,
);
const baselinePath = join(root, "scripts/style-ownership-baseline.json");
const baseline = JSON.parse(await readFile(baselinePath, "utf8"));
const failures = [];

if (baseline.schemaVersion !== 1 || !["ratchet", "zero"].includes(baseline.mode)) {
  failures.push("scripts/style-ownership-baseline.json must use schemaVersion 1 and mode ratchet|zero.");
}

const sourceRoot = join(root, "src");
const files = await collectFiles(sourceRoot);
const cssFiles = files.filter((path) => path.endsWith(".css"));
const sourceFiles = files.filter((path) => /\.(?:ts|tsx)$/u.test(path));
const display = (path) => relative(root, path).replaceAll("\\", "/");
const legacyFiles = cssFiles.filter((path) => {
  const candidate = display(path);
  return candidate.startsWith("src/styles/base/") || candidate.startsWith("src/styles/features/");
});
const globalCssImports = [];
const policyAllowances = baseline.policyAllowances ?? {};

for (const path of sourceFiles) {
  const source = await readFile(path, "utf8");
  for (const match of source.matchAll(/import\s+(?:[^;]*?\s+from\s+)?["']([^"']+\.css)["']/gu)) {
    if (!match[1].endsWith(".module.css")) {
      globalCssImports.push({ path: display(path), specifier: match[1] });
    }
  }
}

const foundationEntry = join(root, "src/ui/foundations/index.css");
let foundationSource = "";
try {
  foundationSource = await readFile(foundationEntry, "utf8");
} catch {
  failures.push("Missing required global foundation entry: src/ui/foundations/index.css");
}

if (baseline.mode === "ratchet") {
  const allowedFiles = baseline.legacyFiles ?? {};
  for (const path of legacyFiles) {
    const candidate = display(path);
    const allowance = allowedFiles[candidate];
    if (!allowance) {
      failures.push(`New legacy stylesheet is not allowed: ${candidate}`);
      continue;
    }
    const source = await readFile(path, "utf8");
    const currentSelectors = selectorClasses(source);
    const allowedSelectors = new Set(allowance.selectors ?? []);
    for (const selector of currentSelectors) {
      if (!allowedSelectors.has(selector)) {
        failures.push(`${candidate} introduced legacy selector .${selector}; move it to an explicit owner instead.`);
      }
    }
  }

  const allowedImports = new Set(
    (baseline.allowedGlobalCssImports ?? []).map(({ path, specifier }) => `${path}\u0000${specifier}`),
  );
  for (const cssImport of globalCssImports) {
    const key = `${cssImport.path}\u0000${cssImport.specifier}`;
    if (!allowedImports.has(key)) {
      failures.push(
        `${cssImport.path} introduced global CSS import ${cssImport.specifier}; use the foundation entry or a CSS Module.`,
      );
    }
  }
} else {
  const expectedImport = "src/main.tsx\u0000./ui/foundations/index.css";
  const configuredImports = new Set(
    (baseline.allowedGlobalCssImports ?? []).map(({ path, specifier }) => `${path}\u0000${specifier}`),
  );
  const actualImports = new Set(
    globalCssImports.map(({ path, specifier }) => `${path}\u0000${specifier}`),
  );

  if (legacyFiles.length > 0) {
    failures.push(
      `Zero-legacy mode forbids src/styles/base and src/styles/features CSS: ${legacyFiles.map(display).join(", ")}`,
    );
  }
  if (Object.keys(baseline.legacyFiles ?? {}).length !== 0) {
    failures.push("Zero-legacy mode requires an empty legacyFiles baseline.");
  }
  if (configuredImports.size !== 1 || !configuredImports.has(expectedImport)) {
    failures.push(
      "Zero-legacy mode requires allowedGlobalCssImports to contain exactly src/main.tsx -> ./ui/foundations/index.css.",
    );
  }
  if (actualImports.size !== 1 || !actualImports.has(expectedImport)) {
    failures.push(
      `Zero-legacy mode requires exactly one global TypeScript CSS import (src/main.tsx -> ./ui/foundations/index.css); found ${[...actualImports].join(", ") || "none"}.`,
    );
  }
  if (Object.keys(policyAllowances).length !== 0) {
    failures.push("Zero-legacy mode requires policyAllowances to be empty; migrate every temporary style-policy exception.");
  }

  const expectedLayerOrder =
    "@layer reset, tokens, theme, base, components, patterns, features, utilities;";
  if (!foundationSource.includes(expectedLayerOrder)) {
    failures.push(`Zero-legacy foundation must declare the final cascade exactly: ${expectedLayerOrder}`);
  }
  if (/\blegacy\b/u.test(foundationSource)) {
    failures.push("Zero-legacy mode forbids a legacy cascade layer or legacy import in src/ui/foundations/index.css.");
  }
  if (/src\/styles\/(?:base|features)|\.\.\/\.\.\/styles\/(?:base|features)/u.test(foundationSource)) {
    failures.push("Zero-legacy mode forbids legacy stylesheet imports from src/ui/foundations/index.css.");
  }

  const allowedGlobalCssFiles = new Set([
    "src/ui/foundations/index.css",
    "src/ui/foundations/local-fonts.css",
    "src/ui/foundations/reset.css",
    "src/ui/foundations/document.css",
    "src/ui/foundations/platform.css",
    "src/ui/foundations/utilities.css",
  ]);
  for (const path of cssFiles) {
    const candidate = display(path);
    if (candidate.endsWith(".module.css") || candidate.includes("/tokens/generated/")) continue;
    if (!allowedGlobalCssFiles.has(candidate)) {
      failures.push(
        `Zero-legacy mode forbids authored global CSS outside the foundation: ${candidate}.`,
      );
    }
  }
}

// Production className semantics are checked structurally by
// verify-owned-class-usage.mjs after the pinned TypeScript package is installed.
// Do not duplicate that check here with raw text matching: semantic keys passed
// through ownedClassNames are CSS Module lookups, not global class dependencies.

// CSS policy discovery is intentionally dynamic. During migration the baseline may
// grandfather existing violations, but those allowances may only shrink and are
// removed entirely before the zero-legacy checkpoint.
for (const path of cssFiles) {
  const candidate = display(path);
  if (candidate.includes("/tokens/generated/")) continue;
  if (candidate.startsWith("src/styles/base/") || candidate.startsWith("src/styles/features/")) continue;
  const source = await readFile(path, "utf8");
  const actual = policyCounts(source);
  const allowed = policyAllowances[candidate] ?? {
    rawColors: 0,
    numericZIndexes: 0,
    arbitraryDurations: 0,
  };
  for (const key of ["rawColors", "numericZIndexes", "arbitraryDurations"]) {
    if (actual[key] > (allowed[key] ?? 0)) {
      failures.push(
        `${candidate} has ${actual[key]} ${key}; baseline allows ${(allowed[key] ?? 0)}. Use generated semantic tokens.`,
      );
    }
  }
}

for (const candidate of Object.keys(policyAllowances)) {
  if (!cssFiles.some((path) => display(path) === candidate)) {
    failures.push(`Style-policy allowance references missing stylesheet ${candidate}; remove the stale allowance.`);
  }
}

if (failures.length > 0) {
  console.error("Style ownership verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `OK   Style ownership (${baseline.mode}): ${cssFiles.length} authored/generated CSS files discovered; ${legacyFiles.length} legacy stylesheets remain.`,
);

function selectorClasses(source) {
  return new Set([...source.matchAll(/\.([A-Za-z_][A-Za-z0-9_-]*)/gu)].map((match) => match[1]));
}

function policyCounts(source) {
  const rawColors = [...source.matchAll(/(?:^|[\s:,(])#[0-9a-f]{3,8}\b|\brgba?\(|\bhsla?\(/gimu)].length;
  const numericZIndexes = [...source.matchAll(/z-index\s*:\s*(-?\d+)\s*;/gu)].filter(
    (match) => Math.abs(Number(match[1])) > 1,
  ).length;
  const arbitraryDurations = [...source.matchAll(/(?:animation|transition)[^;]*;/gu)].filter((match) =>
    /\b\d+(?:\.\d+)?m?s\b/u.test(match[0]),
  ).length;
  return { rawColors, numericZIndexes, arbitraryDurations };
}

async function collectFiles(directory) {
  try {
    await access(directory);
  } catch {
    return [];
  }
  const output = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (["node_modules", "dist", "target", "gen", "artifacts"].includes(entry.name)) continue;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) output.push(...(await collectFiles(path)));
    else if (entry.isFile()) output.push(path);
  }
  return output;
}
