#!/usr/bin/env node
import { readFile, readdir } from "node:fs/promises";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { importViolation } from "./ui/import-policy.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const failures = [];
for (const path of await files(join(root, "src"))) {
  if (!/\.[cm]?[jt]sx?$/u.test(path)) continue;
  const owner = relative(root, path).replaceAll("\\", "/");
  const source = ts.createSourceFile(owner, await readFile(path, "utf8"), ts.ScriptTarget.Latest, true);
  function check(node, specifier) {
    if (!specifier || !ts.isStringLiteralLike(specifier)) return;
    const violation = importViolation(owner, specifier.text);
    if (violation) failures.push(`${owner}:${source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1}: ${violation}`);
  }
  function visit(node) {
    if (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) check(node, node.moduleSpecifier);
    if (ts.isImportEqualsDeclaration(node) && ts.isExternalModuleReference(node.moduleReference)) {
      check(node, node.moduleReference.expression);
    }
    if (ts.isImportTypeNode(node) && ts.isLiteralTypeNode(node.argument)) check(node, node.argument.literal);
    if (ts.isCallExpression(node)
        && (node.expression.kind === ts.SyntaxKind.ImportKeyword
          || (ts.isIdentifier(node.expression) && node.expression.text === "require"))) {
      check(node, node.arguments[0]);
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
}
if (failures.length) {
  console.error(`UI import boundary verification failed:\n${failures.join("\n")}`);
  process.exitCode = 1;
} else console.log("OK   shadcn and Base UI remain behind oStudy presentation adapters.");

async function files(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.name === "node_modules") continue;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) result.push(...await files(path));
    else if (entry.isFile()) result.push(path);
  }
  return result;
}
