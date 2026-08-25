#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { access, readFile, readdir } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { fileURLToPath } from "node:url";

const scriptRoot = fileURLToPath(new URL("..", import.meta.url));
const rootArgumentIndex = process.argv.indexOf("--root");
const root = resolve(
  rootArgumentIndex >= 0 && process.argv[rootArgumentIndex + 1]
    ? process.argv[rootArgumentIndex + 1]
    : scriptRoot,
);
const ts = await loadTypeScript();
const failures = [];
const sharedOwnerPaths = [
  "src/ui/patterns/ScreenPatterns.module.css",
  "src/ui/primitives/ControlPatterns.module.css",
  "src/ui/overlays/OverlayPatterns.module.css",
];
const sharedClasses = new Set();
for (const ownerPath of sharedOwnerPaths) {
  const absolute = join(root, ownerPath);
  try {
    for (const className of selectorClasses(await readFile(absolute, "utf8"))) {
      sharedClasses.add(className);
    }
  } catch {
    failures.push(`Missing shared CSS Module owner ${ownerPath}.`);
  }
}

const utilityPath = join(root, "src/ui/foundations/utilities.css");
let approvedGlobalUtilities = new Set();
try {
  approvedGlobalUtilities = selectorClasses(await readFile(utilityPath, "utf8"));
} catch {
  failures.push("Missing global utility owner src/ui/foundations/utilities.css.");
}

const sourceRoot = join(root, "src");
const sourceFiles = (await collectFiles(sourceRoot)).filter(
  (path) => path.endsWith(".tsx") && !isTestSource(path),
);
let checkedCalls = 0;
let checkedRawClassNames = 0;

for (const path of sourceFiles) {
  const source = await readFile(path, "utf8");
  if (!source.includes("className") && !source.includes("ownedClassNames")) continue;
  const displayPath = relative(root, path).replaceAll("\\", "/");
  const sourceFile = ts.createSourceFile(
    displayPath,
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TSX,
  );
  const cssModuleImports = cssModuleImportPaths(sourceFile, path);
  const ownerClasses = new Set(sharedClasses);
  const ownedStyleMapIdentifiers = findOwnedStyleMapIdentifiers(sourceFile);

  for (const identifier of ownedStyleMapIdentifiers) {
    const modulePath = cssModuleImports.get(identifier);
    if (!modulePath) continue;
    try {
      for (const className of selectorClasses(await readFile(modulePath, "utf8"))) {
        ownerClasses.add(className);
      }
    } catch {
      failures.push(`${displayPath} references missing CSS Module owner ${relative(root, modulePath).replaceAll("\\", "/")}.`);
    }
  }

  function visit(node) {
    if (
      ts.isCallExpression(node)
      && ts.isIdentifier(node.expression)
      && node.expression.text === "ownedClassNames"
    ) {
      checkedCalls += 1;
      const firstArgument = node.arguments[0];
      if (!firstArgument || !ts.isIdentifier(firstArgument) || firstArgument.text !== "ownedStyleMaps") {
        failures.push(`${location(sourceFile, node)} must pass ownedStyleMaps as the first ownedClassNames argument.`);
      }
      const values = [];
      for (const argument of node.arguments.slice(1)) collectClassValues(argument, values, true);
      for (const value of values) {
        validateOwnedValue(value, ownerClasses, approvedGlobalUtilities, sourceFile, node, failures);
      }
    }

    if (ts.isJsxAttribute(node) && node.name.text === "className" && node.initializer) {
      const expression = jsxClassExpression(node.initializer);
      if (expression && !containsOwnedClassCall(expression)) {
        const values = [];
        collectClassValues(expression, values, false);
        for (const value of values) {
          for (const token of literalTokens(value)) {
            if (token === "${}" || approvedGlobalUtilities.has(token)) continue;
            failures.push(
              `${location(sourceFile, node)} emits raw production class ${JSON.stringify(token)}; use a CSS Module value or ownedClassNames with an explicit owner.`,
            );
          }
        }
        checkedRawClassNames += 1;
      }
    }

    ts.forEachChild(node, visit);
  }

  visit(sourceFile);
}

if (failures.length > 0) {
  console.error("Owned class usage verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `OK   Owned class usage: ${checkedCalls} semantic class resolutions and ${checkedRawClassNames} direct className expressions checked.`,
);

function cssModuleImportPaths(sourceFile, sourcePath) {
  const imports = new Map();
  for (const statement of sourceFile.statements) {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier)) continue;
    if (!statement.moduleSpecifier.text.endsWith(".module.css")) continue;
    const localName = statement.importClause?.name?.text;
    if (!localName) continue;
    imports.set(localName, resolve(dirname(sourcePath), statement.moduleSpecifier.text));
  }
  return imports;
}

function findOwnedStyleMapIdentifiers(sourceFile) {
  const identifiers = new Set();
  function collect(node) {
    if (ts.isIdentifier(node)) {
      if (node.text !== "sharedClassMaps") identifiers.add(node.text);
      return;
    }
    ts.forEachChild(node, collect);
  }
  for (const statement of sourceFile.statements) {
    if (!ts.isVariableStatement(statement)) continue;
    for (const declaration of statement.declarationList.declarations) {
      if (
        ts.isIdentifier(declaration.name)
        && declaration.name.text === "ownedStyleMaps"
        && declaration.initializer
      ) {
        collect(declaration.initializer);
      }
    }
  }
  return identifiers;
}

function jsxClassExpression(initializer) {
  if (ts.isStringLiteral(initializer)) return initializer;
  if (ts.isJsxExpression(initializer)) return initializer.expression ?? null;
  return null;
}

function containsOwnedClassCall(node) {
  let found = false;
  function visit(candidate) {
    if (
      ts.isCallExpression(candidate)
      && ts.isIdentifier(candidate.expression)
      && candidate.expression.text === "ownedClassNames"
    ) {
      found = true;
      return;
    }
    if (!found) ts.forEachChild(candidate, visit);
  }
  visit(node);
  return found;
}

function collectClassValues(node, output, inspectFunctionCalls) {
  if (ts.isStringLiteralLike(node)) {
    output.push(node.text);
    return;
  }
  if (ts.isTemplateExpression(node)) {
    output.push(templatePattern(node));
    return;
  }
  if (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node)) {
    collectClassValues(node.expression, output, inspectFunctionCalls);
    return;
  }
  if (ts.isConditionalExpression(node)) {
    collectClassValues(node.whenTrue, output, inspectFunctionCalls);
    collectClassValues(node.whenFalse, output, inspectFunctionCalls);
    return;
  }
  if (ts.isArrayLiteralExpression(node)) {
    for (const element of node.elements) collectClassValues(element, output, inspectFunctionCalls);
    return;
  }
  if (ts.isCallExpression(node)) {
    if (
      ts.isPropertyAccessExpression(node.expression)
      && ["filter", "join"].includes(node.expression.name.text)
    ) {
      collectClassValues(node.expression.expression, output, inspectFunctionCalls);
      return;
    }
    if (inspectFunctionCalls) {
      for (const argument of node.arguments) {
        if (!ts.isArrowFunction(argument) && !ts.isFunctionExpression(argument)) {
          collectClassValues(argument, output, inspectFunctionCalls);
        }
      }
    }
    return;
  }
  if (ts.isBinaryExpression(node)) {
    if (
      node.operatorToken.kind === ts.SyntaxKind.PlusToken
      || node.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken
      || node.operatorToken.kind === ts.SyntaxKind.BarBarToken
      || node.operatorToken.kind === ts.SyntaxKind.QuestionQuestionToken
    ) {
      collectClassValues(node.left, output, inspectFunctionCalls);
      collectClassValues(node.right, output, inspectFunctionCalls);
    }
  }
}

function templatePattern(node) {
  let pattern = node.head.text;
  for (const span of node.templateSpans) pattern += "${}" + span.literal.text;
  return pattern;
}

function validateOwnedValue(value, ownerClasses, approvedUtilities, sourceFile, node, output) {
  for (const token of literalTokens(value)) {
    if (token === "${}" || approvedUtilities.has(token) || ownerClasses.has(token)) continue;
    if (token.includes("${}")) {
      const pattern = new RegExp(
        `^${token
          .split("${}")
          .map((part) => escapeRegExp(part))
          .join("[-A-Za-z0-9_]+")}$`,
        "u",
      );
      if ([...ownerClasses, ...approvedUtilities].some((className) => pattern.test(className))) {
        continue;
      }
    }
    output.push(
      `${location(sourceFile, node)} uses unowned semantic class ${JSON.stringify(token)} from ${JSON.stringify(value)}.`,
    );
  }
}

function literalTokens(value) {
  return value.split(/\s+/u).filter(Boolean);
}

function selectorClasses(source) {
  const withoutComments = source.replace(/\/\*[\s\S]*?\*\//gu, "");
  return new Set(
    [...withoutComments.matchAll(/\.([A-Za-z_][A-Za-z0-9_-]*)/gu)].map((match) => match[1]),
  );
}

function location(sourceFile, node) {
  const point = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
  return `${sourceFile.fileName}:${point.line + 1}:${point.character + 1}`;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

function isTestSource(path) {
  const normalized = path.replaceAll("\\", "/");
  return /(?:^|\/)(?:__tests__|test)(?:\/|$)/u.test(normalized)
    || /\.(?:test|spec|stories)\.tsx$/u.test(normalized);
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
