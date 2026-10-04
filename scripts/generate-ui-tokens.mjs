#!/usr/bin/env node

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import { renderTailwindTheme } from "./ui/tailwind-theme.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const sourceDirectory = join(root, "src", "ui", "tokens");
const outputDirectory = join(sourceDirectory, "generated");
const checkOnly = process.argv.includes("--check");

const reference = await readJson(join(sourceDirectory, "source", "reference.tokens.json"));
const component = await readJson(join(sourceDirectory, "source", "component.tokens.json"));
const themeNames = ["light", "dark", "console", "synthwave"];
const themes = new Map();
for (const theme of themeNames) {
  themes.set(theme, await readJson(join(sourceDirectory, "themes", `${theme}.tokens.json`)));
}

// Viewport breakpoints cannot be CSS variables (media queries do not read them), so they
// generate the layout media-query strings and the Tailwind layout variants instead.
const { breakpoint: breakpointSource, ...referenceSource } = reference;
const breakpoints = readBreakpoints(breakpointSource);
const referenceTokens = flattenTokens(referenceSource);
const componentTokens = flattenTokens(component);
const themeTokens = new Map(
  [...themes.entries()].map(([name, value]) => [name, flattenTokens(value)]),
);
validateThemeContract(themeTokens);
validateAliases(referenceTokens, componentTokens, themeTokens);

const outputs = new Map([
  [join(outputDirectory, "tailwind-theme.css"), renderTailwindTheme(new Set([
    ...referenceTokens.keys(), ...componentTokens.keys(), ...themeTokens.get("light").keys(),
  ]))],
  [join(outputDirectory, "reference.css"), renderRootCss(referenceTokens)],
  [join(outputDirectory, "component.css"), renderRootCss(componentTokens)],
  ...[...themeTokens.entries()].map(([theme, tokens]) => [
    join(outputDirectory, `theme-${theme}.css`),
    renderThemeCss(theme, tokens),
  ]),
  [
    join(outputDirectory, "tokenNames.ts"),
    renderTypeScript(referenceTokens, componentTokens, themeTokens),
  ],
  [join(outputDirectory, "viewport.ts"), renderViewportTypeScript(breakpoints)],
  [join(outputDirectory, "viewport-variants.css"), renderViewportVariants(breakpoints)],
]);

if (checkOnly) {
  let current = true;
  for (const [path, expected] of outputs) {
    let actual = "";
    try {
      actual = await readFile(path, "utf8");
    } catch {
      current = false;
      console.error(`Missing generated UI token file: ${relative(root, path)}`);
      continue;
    }
    if (actual !== expected) {
      current = false;
      console.error(`Generated UI token file is stale: ${relative(root, path)}`);
    }
  }
  if (!current) {
    console.error("Run: node scripts/generate-ui-tokens.mjs");
    process.exit(1);
  }
  console.log("OK   UI token sources and generated artifacts are current.");
  process.exit(0);
}

await mkdir(outputDirectory, { recursive: true });
for (const [path, content] of outputs) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, content, "utf8");
}
console.log(`Generated ${outputs.size} UI token artifacts.`);

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

function flattenTokens(value, prefix = [], output = new Map()) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`Token group ${prefix.join(".") || "<root>"} must be an object.`);
  }
  if (Object.hasOwn(value, "$value")) {
    if (prefix.length === 0) throw new Error("The token root cannot be a token value.");
    output.set(prefix.join("."), {
      type: typeof value.$type === "string" ? value.$type : "unknown",
      value: value.$value,
    });
    return output;
  }
  for (const [key, child] of Object.entries(value)) {
    if (key.startsWith("$")) continue;
    flattenTokens(child, [...prefix, key], output);
  }
  return output;
}

function validateThemeContract(themeMaps) {
  const [baselineName, baseline] = themeMaps.entries().next().value;
  const baselineKeys = [...baseline.keys()].sort();
  for (const [theme, tokens] of themeMaps) {
    const keys = [...tokens.keys()].sort();
    const missing = baselineKeys.filter((key) => !tokens.has(key));
    const extra = keys.filter((key) => !baseline.has(key));
    if (missing.length > 0 || extra.length > 0) {
      throw new Error(
        `${theme} does not match ${baselineName}. Missing: ${missing.join(", ") || "none"}. Extra: ${extra.join(", ") || "none"}.`,
      );
    }
  }
}

/** Reference primitives are theme-independent. Component tokens may alias any tier because
 * every tier resolves on :root. Theme tokens may alias primitives and other roles of the same
 * theme (radius.button -> radius.pill, chart.1 -> subject.1), so shape such as radius can vary
 * per theme while each theme file stays the single owner of its values. */
function validateAliases(referenceTokens, componentTokens, themeMaps) {
  const semanticNames = new Set([...themeMaps.values()][0].keys());
  for (const name of componentTokens.keys()) {
    if (referenceTokens.has(name)) throw new Error(`Component token ${name} repeats a reference token name.`);
  }
  for (const name of semanticNames) {
    if (referenceTokens.has(name) || componentTokens.has(name)) {
      throw new Error(`Theme token ${name} repeats a reference or component token name.`);
    }
  }
  assertReferences(referenceTokens, (name) => referenceTokens.has(name), "Reference");
  assertReferences(
    componentTokens,
    (name) => referenceTokens.has(name) || componentTokens.has(name) || semanticNames.has(name),
    "Component",
  );
  for (const [theme, tokens] of themeMaps) {
    assertReferences(tokens, (name) => referenceTokens.has(name) || tokens.has(name), `${theme} theme`);
    assertAcyclic(new Map([...referenceTokens, ...componentTokens, ...tokens]), theme);
  }
}

function tokenReferences(token) {
  if (typeof token.value !== "string") return [];
  return [...token.value.matchAll(/\{([a-zA-Z0-9_.-]+)\}/gu)].map((match) => match[1]);
}

function assertReferences(tokens, isAvailable, scope) {
  for (const [name, token] of tokens) {
    for (const target of tokenReferences(token)) {
      if (!isAvailable(target)) throw new Error(`${scope} token ${name} references unknown token ${target}.`);
    }
  }
}

function assertAcyclic(tokens, theme) {
  const state = new Map();
  const visit = (name, path) => {
    if (state.get(name) === "done") return;
    if (state.get(name) === "visiting") {
      throw new Error(`Token aliases form a cycle in the ${theme} theme: ${[...path, name].join(" -> ")}.`);
    }
    state.set(name, "visiting");
    for (const target of tokenReferences(tokens.get(name))) visit(target, [...path, name]);
    state.set(name, "done");
  };
  for (const name of tokens.keys()) visit(name, []);
}

function readBreakpoints(source) {
  const tokens = flattenTokens(source ?? {});
  const widths = {};
  for (const name of ["medium", "wide"]) {
    const value = tokens.get(name)?.value;
    const match = typeof value === "string" ? /^(\d+)px$/u.exec(value) : null;
    if (!match) throw new Error(`breakpoint.${name} must be a whole pixel width such as 760px.`);
    widths[name] = Number(match[1]);
  }
  if (tokens.size !== 2) throw new Error("Only breakpoint.medium and breakpoint.wide are supported.");
  if (widths.medium >= widths.wide) throw new Error("breakpoint.medium must be narrower than breakpoint.wide.");
  return {
    widths,
    compact: `(width < ${widths.medium}px)`,
    medium: `(${widths.medium}px <= width < ${widths.wide}px)`,
    wide: `(width >= ${widths.wide}px)`,
    belowWide: `(width < ${widths.wide}px)`,
  };
}

function renderViewportTypeScript(breakpoints) {
  return [
    "// This file is generated by scripts/generate-ui-tokens.mjs. Do not edit directly.",
    "/** Viewport widths (CSS px) where the medium and wide layouts begin. */",
    `export const viewportBreakpoints = { medium: ${breakpoints.widths.medium}, wide: ${breakpoints.widths.wide} } as const;`,
    "",
    "/** Layout media queries. Tailwind's compact, medium and wide variants use the same conditions. */",
    "export const viewportQueries = {",
    `  compact: ${JSON.stringify(breakpoints.compact)},`,
    `  belowWide: ${JSON.stringify(breakpoints.belowWide)},`,
    "} as const;",
    "",
  ].join("\n");
}

function renderViewportVariants(breakpoints) {
  const variant = (name, query) => [
    `@custom-variant ${name} {`,
    `  @media ${query} {`,
    "    @slot;",
    "  }",
    "}",
  ];
  return [
    "/* This file is generated by scripts/generate-ui-tokens.mjs. Do not edit directly.",
    " * Layout variants share their conditions with viewportQueries in viewport.ts. CSS Modules",
    " * use them through @reference and a nested @variant. */",
    ...variant("compact", breakpoints.compact),
    ...variant("medium", breakpoints.medium),
    ...variant("wide", breakpoints.wide),
    "",
  ].join("\n");
}

function renderRootCss(tokens) {
  const lines = [
    "/* This file is generated by scripts/generate-ui-tokens.mjs. Do not edit directly. */",
    "@layer tokens {",
    "  :root {",
  ];
  for (const [name, token] of sortedEntries(tokens)) {
    lines.push(`    ${cssVariable(name)}: ${cssValue(token.value)};`);
  }
  lines.push("  }", "}", "");
  return lines.join("\n");
}

function renderThemeCss(theme, tokens) {
  const lines = [
    "/* This file is generated by scripts/generate-ui-tokens.mjs. Do not edit directly. */",
    "@layer theme {",
    `  :root[data-theme=\"${theme}\"] {`,
  ];
  for (const [name, token] of sortedEntries(tokens)) {
    lines.push(`    ${cssVariable(name)}: ${cssValue(token.value)};`);
  }
  lines.push("  }", "}", "");
  return lines.join("\n");
}

function renderTypeScript(referenceTokens, componentTokens, themeMaps) {
  const semantic = [...themeMaps.values()][0];
  const lines = [
    "// This file is generated by scripts/generate-ui-tokens.mjs. Do not edit directly.",
    `export const themeNames = ${JSON.stringify([...themeMaps.keys()])} as const;`,
    "export type ThemeName = (typeof themeNames)[number];",
    "",
    `export const referenceTokenNames = ${JSON.stringify([...referenceTokens.keys()].sort(), null, 2)} as const;`,
    "export type ReferenceTokenName = (typeof referenceTokenNames)[number];",
    "",
    `export const componentTokenNames = ${JSON.stringify([...componentTokens.keys()].sort(), null, 2)} as const;`,
    "export type ComponentTokenName = (typeof componentTokenNames)[number];",
    "",
    `export const semanticTokenNames = ${JSON.stringify([...semantic.keys()].sort(), null, 2)} as const;`,
    "export type SemanticTokenName = (typeof semanticTokenNames)[number];",
    "",
    "export function tokenVariable(name: ReferenceTokenName | ComponentTokenName | SemanticTokenName): `--ostudy-${string}` {",
    '  const normalized = name.replace(/[A-Z]/gu, (value) => `-${value.toLowerCase()}`).replaceAll(".", "-").replace(/([a-z])(\\d)/gu, "$1-$2");',
    "  return `--ostudy-${normalized}`;",
    "}",
    "",
  ];
  return lines.join("\n");
}

function cssVariable(name) {
  const normalized = name
    .replace(/[A-Z]/gu, (value) => `-${value.toLowerCase()}`)
    .replaceAll(".", "-")
    .replace(/([a-z])(\d)/gu, "$1-$2");
  return `--ostudy-${normalized}`;
}

function cssValue(value) {
  if (typeof value === "number") return String(value);
  if (typeof value !== "string") {
    throw new Error(`Only string and number token values are supported, received ${typeof value}.`);
  }
  return value.replace(/\{([a-zA-Z0-9_.-]+)\}/gu, (_, name) => `var(${cssVariable(name)})`);
}

function sortedEntries(tokens) {
  return [...tokens.entries()].sort(([left], [right]) => left.localeCompare(right));
}
