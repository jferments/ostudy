#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const [css, tauriConfigText] = await Promise.all([
  readFile(join(root, "src/screens/flashcards/DeckLibraryWorkspace.module.css"), "utf8"),
  readFile(join(root, "src-tauri/tauri.conf.json"), "utf8"),
]);
const failures = [];

expectRule(
  css,
  ".root",
  ["container-type: inline-size", "container-name: deck-library"],
  "deck-library container",
);
expectRule(css, ".toolbar", ["display: flex", "justify-content: flex-end"], "deck toolbar");
expectRule(
  css,
  ".toolbarActions",
  ["display: flex", "justify-content: flex-end"],
  "deck toolbar action group",
);
expectRule(
  css,
  ".toolbarActions > button",
  ["flex: 0 0 auto", "white-space: nowrap"],
  "stable action button sizing",
);
expectRule(
  css,
  ".sectionActions",
  ["display: flex", "justify-content: flex-end"],
  "folder section action group",
);
expectContains(
  css,
  "@container deck-library (max-width: 620px)",
  "content-width compact breakpoint",
);
expectNestedRule(
  css,
  "@container deck-library (max-width: 620px)",
  ".toolbarActions",
  ["width: 100%", "justify-content: stretch"],
  "compact toolbar wrapping",
);
expectNestedRule(
  css,
  "@container deck-library (max-width: 620px)",
  ".sectionActions",
  ["width: 100%", "justify-content: stretch"],
  "compact folder controls",
);
expectContains(
  css,
  ':global(:root[data-input-mode="coarse"]) .dragHandle',
  "coarse-pointer drag handle sizing",
);
expectAbsent(css, ".density", "removed deck-density selector");
expectRuleAbsent(
  css,
  ".dragHandle",
  "border-inline-end",
  "removed move-handle divider",
);

const tauriConfig = JSON.parse(tauriConfigText);
const mainWindow = tauriConfig?.app?.windows?.find((window) => window?.label === "main");
if (!mainWindow || typeof mainWindow.width !== "number" || mainWindow.width < 1280) {
  failures.push("The default desktop main window must be at least 1280 CSS pixels wide.");
}

if (failures.length > 0) {
  console.error("Deck library layout contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Default desktop sizing, container-responsive deck controls, compact density, and coarse-pointer targets are consistent.",
);

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}

function expectAbsent(source, forbidden, label) {
  if (source.includes(forbidden)) failures.push(`Unexpected ${label}: ${forbidden}`);
}

function expectRule(source, selector, declarations, label) {
  const body = ruleBody(source, selector);
  if (body === null) {
    failures.push(`Missing ${label}: ${selector}`);
    return;
  }
  for (const declaration of declarations) {
    if (!body.includes(declaration)) {
      failures.push(`Missing ${label} declaration in ${selector}: ${declaration}`);
    }
  }
}

function expectRuleAbsent(source, selector, declaration, label) {
  const body = ruleBody(source, selector);
  if (body?.includes(declaration)) {
    failures.push(`Unexpected ${label} declaration in ${selector}: ${declaration}`);
  }
}

function expectNestedRule(source, containerHeader, selector, declarations, label) {
  const containerStart = source.indexOf(containerHeader);
  if (containerStart < 0) {
    failures.push(`Missing ${label} container: ${containerHeader}`);
    return;
  }
  const containerBody = balancedBlock(source, source.indexOf("{", containerStart));
  if (containerBody === null) {
    failures.push(`Malformed ${label} container: ${containerHeader}`);
    return;
  }
  expectRule(containerBody, selector, declarations, label);
}

function ruleBody(source, selector) {
  const selectorStart = source.indexOf(selector);
  if (selectorStart < 0) return null;
  const openingBrace = source.indexOf("{", selectorStart + selector.length);
  if (openingBrace < 0) return null;
  return balancedBlock(source, openingBrace);
}

function balancedBlock(source, openingBrace) {
  if (openingBrace < 0 || source[openingBrace] !== "{") return null;
  let depth = 0;
  for (let index = openingBrace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(openingBrace + 1, index);
    }
  }
  return null;
}
