#!/usr/bin/env node

import { access, readFile, readdir } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const failures = [];

const tokenCheck = spawnSync(
  process.execPath,
  [join(root, "scripts/generate-ui-tokens.mjs"), "--check"],
  { cwd: root, encoding: "utf8" },
);
if (tokenCheck.status !== 0) {
  failures.push(
    tokenCheck.stderr || tokenCheck.stdout || "Generated UI tokens are stale.",
  );
}

const requiredFiles = [
  "src/ui/tokens/themes/light.tokens.json",
  "src/ui/tokens/themes/dark.tokens.json",
  "src/ui/tokens/themes/console.tokens.json",
  "src/ui/tokens/themes/synthwave.tokens.json",
  "src/ui/artwork/ARTWORK-LICENSES.md",
];
for (const path of requiredFiles) {
  try {
    await access(join(root, path));
  } catch {
    failures.push(`Missing required UI-system file: ${path}`);
  }
}

const styleBaseline = JSON.parse(
  await readFile(join(root, "scripts/style-ownership-baseline.json"), "utf8"),
);
const policyAllowances = styleBaseline.policyAllowances ?? {};
const governedCss = (await collectCss(join(root, "src")))
  .map((path) => relative(root, path).replaceAll("\\", "/"))
  .filter(
    (path) =>
      !path.includes("/tokens/generated/") &&
      !path.startsWith("src/styles/base/") &&
      !path.startsWith("src/styles/features/"),
  );

for (const path of governedCss) {
  const absolute = join(root, path);
  let source;
  try {
    source = await readFile(absolute, "utf8");
  } catch {
    failures.push(`Missing discovered CSS file: ${path}`);
    continue;
  }

  const actual = {
    numericZIndexes: [...source.matchAll(/z-index\s*:\s*(-?\d+)\s*;/gu)].filter(
      (match) => Math.abs(Number(match[1])) > 1,
    ).length,
    rawColors: [
      ...source.matchAll(/(?:^|[\s:,(])#[0-9a-f]{3,8}\b|\brgba?\(|\bhsla?\(/gimu),
    ].length,
    arbitraryDurations: [...source.matchAll(/(?:animation|transition)[^;]*;/gu)].filter(
      (match) => /\b\d+(?:\.\d+)?m?s\b/u.test(match[0]),
    ).length,
  };
  const allowance = policyAllowances[path] ?? {
    numericZIndexes: 0,
    rawColors: 0,
    arbitraryDurations: 0,
  };
  for (const key of ["numericZIndexes", "rawColors", "arbitraryDurations"]) {
    if (actual[key] > (allowance[key] ?? 0)) {
      failures.push(
        `${path} has ${actual[key]} ${key}; migration baseline allows ${allowance[key] ?? 0}.`,
      );
    }
  }
}

const governedTypeScript = [
  "src/screens/flashcards/DeckLibraryWorkspace.tsx",
  "src/screens/practice/PracticeComplete.tsx",
  "src/screens/DocumentsDevelopmentStub.tsx",
  "src/ui/artwork/Artwork.tsx",
];
for (const path of governedTypeScript) {
  const source = await readFile(join(root, path), "utf8");
  if (/from\s+["'][^"']+\.(?:png|jpe?g|webp|svg)["']/iu.test(source)) {
    failures.push(
      `${path} imports an artwork file directly; use the semantic artwork registry.`,
    );
  }
}

if (failures.length > 0) {
  console.error("UI policy verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `OK   UI token generation, theme completeness, governed styles, layer usage, timing, and artwork metadata are valid (${relative(root, import.meta.filename)}).`,
);

async function collectCss(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    if (["node_modules", "dist", "target", "gen", "artifacts"].includes(entry.name)) continue;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await collectCss(path)));
    else if (entry.isFile() && path.endsWith(".css")) files.push(path);
  }
  return files;
}
