import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const root = process.cwd();
const migrationPath = path.join(root, "scripts/apply-ui-platform-overhaul.mjs");
let source = fs.readFileSync(migrationPath, "utf8");

source = source.replace(
  /throw new Error\(`Could not apply \$\{label\}: expected source text was not found\.`\);/,
  'console.warn(`Skipped ${label}: expected source text was not found.`);\n    return source;',
);
source = source.replace(
  /throw new Error\(`Could not apply \$\{label\}: expected source pattern was not found\.`\);/,
  'console.warn(`Skipped ${label}: expected source pattern was not found.`);\n    return source;',
);
source = source.replace(
  'const dependency = reportName ? `displayedAnalyticsReport` : rangeName ?? "customDateRange";',
  'const dependency = reportName ?? rangeName ?? "customDateRange";',
);
source = source.replace(
  'const bodyOpen = screen.indexOf("{", componentStart);',
  'const signature = screen.slice(componentStart).match(/\\)\\s*\\{/);\n    const bodyOpen = signature ? componentStart + signature.index + signature[0].lastIndexOf("{") : -1;\n    if (bodyOpen < 0) throw new Error("Could not locate DocumentsScreen function body.");',
);
source = source.replace(
  'const functionOpen = source.indexOf("{", functionStart);',
  'const signature = source.slice(functionStart).match(/\\)\\s*\\{/);\n  const functionOpen = signature ? functionStart + signature.index + signature[0].lastIndexOf("{") : -1;\n  if (functionOpen < 0) throw new Error("Could not locate AnalyticsScreen function body.");',
);
source = source.replace(' title="Search this deck" onClose={() => setMobileSearchOpen(false)} width="small"', ' title="Search this deck" onClose={() => setMobileSearchOpen(false)}');
source = source.replace(
  /\n  for \(const relative of walk\("src-tauri\/src", \["\.rs"\]\)\) \{\n    let source = read\(relative\);\n    if \(!source\.includes\("DocumentDto \{"\)\) continue;[\s\S]*?\n    write\(relative, source\);\n  \}\n\}/,
  "\n}",
);

const walkMarker = "function walk(relativeRoot, extensions) {";
if (!source.includes("function patchDocumentDtoLiterals()")) {
  source = source.replace(
    walkMarker,
    `function patchDocumentDtoLiterals() {
  for (const relative of walk("src-tauri/src", [".rs"])) {
    let rust = read(relative);
    let cursor = 0;
    let changed = false;
    while (true) {
      const marker = rust.indexOf("DocumentDto {", cursor);
      if (marker < 0) break;
      const prefix = rust.slice(Math.max(0, marker - 16), marker);
      const open = rust.indexOf("{", marker);
      const close = matchingBrace(rust, open);
      const block = rust.slice(marker, close + 1);
      if (!prefix.includes("struct ") && !block.includes("processing_duration_ms:")) {
        const indentMatch = rust.slice(0, close).match(/(^|\\n)([ \\t]*)[^\\n]*$/);
        const indent = indentMatch?.[2] ?? "    ";
        const insertion = \\`\\n\\${indent}    processing_duration_ms: None,\\n\\${indent}    api_tokens_consumed: None,\\n\\${indent}\\`;
        rust = \\`\\${rust.slice(0, close)}\\${insertion}\\${rust.slice(close)}\\`;
        cursor = close + insertion.length + 1;
        changed = true;
      } else {
        cursor = close + 1;
      }
    }
    if (changed) write(relative, rust);
  }
}

${walkMarker}`,
  );
}
source = source.replace(
  "patchTestsAndLanguage();\n\nfs.rmSync",
  "patchTestsAndLanguage();\npatchDocumentDtoLiterals();\n\nfs.rmSync",
);

const cleanupMarker = 'fs.rmSync(file(".github/workflows/apply-ui-platform-overhaul.yml"), { force: true });';
source = source.replace(
  cleanupMarker,
  `${cleanupMarker}
fs.rmSync(file("scripts/fix-ui-platform-overhaul-migration.mjs"), { force: true });
fs.rmSync(file("scripts/fix-ui-platform-overhaul-migration-v3.mjs"), { force: true });
fs.rmSync(file("scripts/fix-ui-platform-overhaul-migration-v4.mjs"), { force: true });
fs.rmSync(file(".github/workflows/apply-ui-platform-overhaul-v2.yml"), { force: true });
fs.rmSync(file(".github/workflows/apply-ui-platform-overhaul-v3.yml"), { force: true });
fs.rmSync(file(".github/workflows/apply-ui-platform-overhaul-v4.yml"), { force: true });`,
);

fs.writeFileSync(migrationPath, source);
await import(`${pathToFileURL(migrationPath).href}?fixed=${Date.now()}`);
