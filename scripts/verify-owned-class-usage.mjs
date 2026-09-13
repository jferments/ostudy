#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { readFile, readdir } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const argument = process.argv.indexOf("--root");
const root = resolve(argument >= 0 ? process.argv[argument + 1] : fileURLToPath(new URL("..", import.meta.url)));
const ts = await loadTypeScript();
const failures = [];
const utilitySource = await readFile(join(root, "src/ui/foundations/utilities.css"), "utf8");
const globalUtilities = new Set([...utilitySource.matchAll(/\.([A-Za-z_][A-Za-z0-9_-]*)/gu)].map((match) => match[1]));
let checked = 0;

for (const path of await files(join(root, "src"))) {
  if (!path.endsWith(".tsx") || /\.(?:test|spec|stories)\.tsx$/u.test(path)
      || /\/(?:__tests__|test)\//u.test(path)) continue;
  const owner = relative(root, path).replaceAll("\\", "/");
  const source = ts.createSourceFile(owner, await readFile(path, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const modules = new Map();
  for (const statement of source.statements) {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier)) continue;
    const specifier = statement.moduleSpecifier.text;
    if (/classOwnership(?:\.[jt]s)?$/u.test(specifier)) {
      failures.push(`${owner}: retired classOwnership API; use shared components and complete utilities.`);
    }
    if (!specifier.endsWith(".module.css")) continue;
    try {
      const css = await readFile(resolve(dirname(path), specifier), "utf8");
      const names = new Set([...css.replace(/\/\*[\s\S]*?\*\//gu, "").matchAll(/\.([A-Za-z_][A-Za-z0-9_-]*)/gu)].map((match) => match[1]));
      if (statement.importClause?.name) modules.set(statement.importClause.name.text, names);
    } catch { failures.push(`${owner}: missing CSS Module owner ${specifier}.`); }
  }
  function report(node, message) {
    const line = source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;
    failures.push(`${owner}:${line}: ${message}`);
  }
  function visit(node) {
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === "ownedClassNames") {
      report(node, "retired ownedClassNames API; use explicit CSS Module values or complete tw: utilities.");
    }
    if (ts.isPropertyAccessExpression(node) && ts.isIdentifier(node.expression) && modules.has(node.expression.text)) {
      if (!modules.get(node.expression.text).has(node.name.text)) report(node, `CSS Module has no .${node.name.text} selector.`);
    }
    if (ts.isJsxAttribute(node) && node.name.text === "className" && node.initializer) {
      checked++;
      const expression = ts.isJsxExpression(node.initializer) ? node.initializer.expression : node.initializer;
      for (const text of literalClasses(expression)) {
        for (const token of text.split(/\s+/u).filter(Boolean)) {
          if (token === "${}" || globalUtilities.has(token)) continue;
          if (token.startsWith("tw:") && token.length > 3 && !token.includes("${}")) continue;
          report(node, `emits raw production class ${JSON.stringify(token)}; use an owned CSS Module or complete tw: utility.`);
        }
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
}
if (failures.length) {
  console.error(`Owned class usage verification failed:\n${failures.join("\n")}`);
  process.exitCode = 1;
} else console.log(`OK   ${checked} className expressions use explicit owners; retired class maps are absent.`);

function literalClasses(node) {
  if (!node) return [];
  if (ts.isStringLiteralLike(node)) return [node.text];
  if (ts.isTemplateExpression(node)) return [node.head.text + node.templateSpans.map((span) => "${}" + span.literal.text).join("")];
  if (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node)) return literalClasses(node.expression);
  if (ts.isConditionalExpression(node)) return [...literalClasses(node.whenTrue), ...literalClasses(node.whenFalse)];
  if (ts.isArrayLiteralExpression(node)) return node.elements.flatMap(literalClasses);
  if (ts.isObjectLiteralExpression(node)) return node.properties.flatMap((property) =>
    ts.isPropertyAssignment(property) && (ts.isStringLiteralLike(property.name) || ts.isIdentifier(property.name)) ? [property.name.text] : []);
  if (ts.isBinaryExpression(node)) {
    if (node.operatorToken.kind === ts.SyntaxKind.PlusToken) return literalClasses(node.left).map((text) => text + "${}");
    if (node.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken) return literalClasses(node.right);
    if ([ts.SyntaxKind.BarBarToken, ts.SyntaxKind.QuestionQuestionToken].includes(node.operatorToken.kind)) return [...literalClasses(node.left), ...literalClasses(node.right)];
    return [];
  }
  if (ts.isCallExpression(node)) {
    if (ts.isIdentifier(node.expression) && node.expression.text === "cn") return node.arguments.flatMap(literalClasses);
    if (ts.isPropertyAccessExpression(node.expression) && ["filter", "join"].includes(node.expression.name.text)) return literalClasses(node.expression.expression);
  }
  return [];
}
async function files(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (["node_modules", "dist", "target", "gen"].includes(entry.name)) continue;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) result.push(...await files(path)); else if (entry.isFile()) result.push(path);
  }
  return result;
}

async function loadTypeScript() {
  try {
    const local = await import("typescript");
    return local.default ?? local;
  } catch (localError) {
    try {
      const globalRoot = execFileSync("npm", ["root", "--global"], {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
      }).trim();
      const globalModule = await import(
        pathToFileURL(join(globalRoot, "typescript/lib/typescript.js")).href
      );
      return globalModule.default ?? globalModule;
    } catch {
      throw new Error(
        `verify-owned-class-usage.mjs requires the pinned TypeScript package. Run pnpm install first. (${localError instanceof Error ? localError.message : String(localError)})`,
      );
    }
  }
}
