#!/usr/bin/env node
import { readFile, readdir } from "node:fs/promises";
import { join, posix, relative } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { importViolation } from "./ui/import-policy.mjs";
import { cssModuleViolations, isProductionSource, nativeControlViolation } from "./ui/presentation-policy.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const paths = await files(join(root, "src"));
const failures = [];
const cssImports = new Set();

for (const path of paths) {
  if (!/\.[cm]?[jt]sx?$/u.test(path)) continue;
  const owner = relative(root, path).replaceAll("\\", "/");
  const production = isProductionSource(owner);
  const source = ts.createSourceFile(owner, await readFile(path, "utf8"), ts.ScriptTarget.Latest, true);
  function fail(node, message) {
    failures.push(`${owner}:${source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1}: ${message}`);
  }
  function checkImport(node, specifier) {
    if (!specifier || !ts.isStringLiteralLike(specifier)) return;
    const violation = importViolation(owner, specifier.text);
    if (violation) fail(node, violation);
    if (production && specifier.text.endsWith(".module.css")) {
      cssImports.add(specifier.text.startsWith("@/")
        ? posix.normalize(`src/${specifier.text.slice(2)}`)
        : posix.normalize(posix.join(posix.dirname(owner), specifier.text)));
    }
  }
  function visit(node) {
    if (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) checkImport(node, node.moduleSpecifier);
    if (ts.isImportEqualsDeclaration(node) && ts.isExternalModuleReference(node.moduleReference)) {
      checkImport(node, node.moduleReference.expression);
    }
    if (ts.isImportTypeNode(node) && ts.isLiteralTypeNode(node.argument)) checkImport(node, node.argument.literal);
    if (ts.isCallExpression(node)
        && (node.expression.kind === ts.SyntaxKind.ImportKeyword
          || (ts.isIdentifier(node.expression) && node.expression.text === "require"))) {
      checkImport(node, node.arguments[0]);
    }
    if (production && (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node))
        && ts.isIdentifier(node.tagName)) {
      const attributes = {};
      for (const attribute of node.attributes.properties) {
        if (ts.isJsxSpreadAttribute(attribute)) { attributes["...spread"] = null; continue; }
        const name = attribute.name.getText(source);
        const value = attribute.initializer;
        attributes[name] = !value ? true : ts.isStringLiteral(value) ? value.text
          : ts.isJsxExpression(value) && value.expression?.kind === ts.SyntaxKind.TrueKeyword ? true : null;
      }
      const violation = nativeControlViolation(owner, node.tagName.text, attributes);
      if (violation) fail(node, violation);
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
}
failures.push(...cssModuleViolations(
  paths.filter((path) => path.endsWith(".module.css")).map((path) => relative(root, path).replaceAll("\\", "/")),
  cssImports,
));
if (failures.length) {
  console.error(`UI boundary verification failed:\n${failures.join("\n")}`);
  process.exitCode = 1;
} else console.log("OK   Vendor imports, shared controls and retained specialized CSS have explicit owners.");

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
