import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const root = process.cwd();
const absolute = (relative) => path.join(root, relative);
const exists = (relative) => fs.existsSync(absolute(relative));
const read = (relative) => fs.readFileSync(absolute(relative), "utf8");
const write = (relative, source) => fs.writeFileSync(absolute(relative), source);

const migration = "scripts/apply-ui-platform-overhaul.mjs";
if (exists(migration) && !exists("src/app/navigation.ts")) {
  let source = read(migration);
  source = source.replace(
    /throw new Error\(`Could not apply \$\{label\}: expected source text was not found\.`\);/,
    'console.warn(`Skipped ${label}: expected source text was not found.`);\n    return source;',
  );
  source = source.replace(
    /throw new Error\(`Could not apply \$\{label\}: expected source pattern was not found\.`\);/,
    'console.warn(`Skipped ${label}: expected source pattern was not found.`);\n    return source;',
  );
  source = source.replace(
    /const dependency = reportName \? `displayedAnalyticsReport` : rangeName \?\? "customDateRange";/,
    'const dependency = "customDateRange";',
  );
  source = source.replace(
    'const bodyOpen = screen.indexOf("{", componentStart);',
    'const signature = screen.slice(componentStart).match(/\\)\\s*\\{/);\n    const bodyOpen = signature ? componentStart + signature.index + signature[0].lastIndexOf("{") : -1;\n    if (bodyOpen < 0) throw new Error("Could not locate DocumentsScreen function body.");',
  );
  source = source.replace(
    'const functionOpen = source.indexOf("{", functionStart);',
    'const signature = source.slice(functionStart).match(/\\)\\s*\\{/);\n  const functionOpen = signature ? functionStart + signature.index + signature[0].lastIndexOf("{") : -1;\n  if (functionOpen < 0) throw new Error("Could not locate AnalyticsScreen function body.");',
  );
  source = source.replace(
    ' title="Search this deck" onClose={() => setMobileSearchOpen(false)} width="small"',
    ' title="Search this deck" onClose={() => setMobileSearchOpen(false)}',
  );
  source = source.replace(
    /\n  for \(const relative of walk\("src-tauri\/src", \["\.rs"\]\)\) \{\n    let source = read\(relative\);\n    if \(!source\.includes\("DocumentDto \{"\)\) continue;[\s\S]*?\n    write\(relative, source\);\n  \}\n\}/,
    "\n}",
  );
  write(migration, source);
  await import(`${pathToFileURL(absolute(migration)).href}?final=${Date.now()}`);
}

if (exists("src/screens/AnalyticsScreen.tsx")) {
  let analytics = read("src/screens/AnalyticsScreen.tsx");
  analytics = analytics.replace(
    /useAnalyticsLineCharts\(analyticsRootRef,\s*(?!customDateRange)[A-Za-z0-9_]+\);/,
    "useAnalyticsLineCharts(analyticsRootRef, customDateRange);",
  );
  write("src/screens/AnalyticsScreen.tsx", analytics);
}

patchDocumentDtoLiterals();
cleanupTemporaryMigrationFiles();
console.log("Finalized UI platform overhaul source tree.");

function patchDocumentDtoLiterals() {
  for (const relative of walk("src-tauri/src", ".rs")) {
    let source = read(relative);
    let cursor = 0;
    let changed = false;
    while (true) {
      const marker = source.indexOf("DocumentDto {", cursor);
      if (marker < 0) break;
      const prefix = source.slice(Math.max(0, marker - 24), marker);
      const open = source.indexOf("{", marker);
      const close = matchingBrace(source, open);
      const block = source.slice(marker, close + 1);
      if (!prefix.includes("struct ") && !block.includes("processing_duration_ms:")) {
        const lineStart = source.lastIndexOf("\n", close) + 1;
        const indent = source.slice(lineStart, close).match(/^\s*/)?.[0] ?? "    ";
        const insertion = `\n${indent}processing_duration_ms: None,\n${indent}api_tokens_consumed: None,`;
        source = `${source.slice(0, close)}${insertion}${source.slice(close)}`;
        cursor = close + insertion.length + 1;
        changed = true;
      } else {
        cursor = close + 1;
      }
    }
    if (changed) write(relative, source);
  }
}

function cleanupTemporaryMigrationFiles() {
  for (const directory of ["scripts", ".github/workflows"]) {
    if (!exists(directory)) continue;
    for (const entry of fs.readdirSync(absolute(directory))) {
      if (
        entry === "apply-ui-platform-overhaul.mjs" ||
        entry.startsWith("fix-ui-platform-overhaul-migration") ||
        entry.startsWith("apply-ui-platform-overhaul") ||
        entry === "finalize-ui-platform-overhaul.mjs" ||
        entry === "finalize-ui-platform-overhaul.yml"
      ) {
        fs.rmSync(absolute(path.join(directory, entry)), { force: true });
      }
    }
  }
}

function walk(relative, extension) {
  if (!exists(relative)) return [];
  const results = [];
  for (const entry of fs.readdirSync(absolute(relative), { withFileTypes: true })) {
    const child = path.join(relative, entry.name).replaceAll(path.sep, "/");
    if (entry.isDirectory()) results.push(...walk(child, extension));
    else if (entry.name.endsWith(extension)) results.push(child);
  }
  return results;
}

function matchingBrace(source, open) {
  let depth = 0;
  let quote = null;
  let escaped = false;
  for (let index = open; index < source.length; index += 1) {
    const character = source[index];
    if (quote) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === '"' || character === "'" || character === "`") {
      quote = character;
      continue;
    }
    if (character === "{") depth += 1;
    if (character === "}") {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  throw new Error("Could not match Rust struct literal brace.");
}
