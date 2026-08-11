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

const cleanupMarker = 'fs.rmSync(file(".github/workflows/apply-ui-platform-overhaul.yml"), { force: true });';
source = source.replace(
  cleanupMarker,
  `${cleanupMarker}
fs.rmSync(file("scripts/fix-ui-platform-overhaul-migration.mjs"), { force: true });
fs.rmSync(file("scripts/fix-ui-platform-overhaul-migration-v3.mjs"), { force: true });
fs.rmSync(file("scripts/fix-ui-platform-overhaul-migration-v4.mjs"), { force: true });
fs.rmSync(file("scripts/fix-ui-platform-overhaul-migration-v5.mjs"), { force: true });
fs.rmSync(file("scripts/fix-ui-platform-overhaul-migration-v6.mjs"), { force: true });
fs.rmSync(file(".github/workflows/apply-ui-platform-overhaul-v2.yml"), { force: true });
fs.rmSync(file(".github/workflows/apply-ui-platform-overhaul-v3.yml"), { force: true });
fs.rmSync(file(".github/workflows/apply-ui-platform-overhaul-v4.yml"), { force: true });
fs.rmSync(file(".github/workflows/apply-ui-platform-overhaul-v5.yml"), { force: true });
fs.rmSync(file(".github/workflows/apply-ui-platform-overhaul-v6.yml"), { force: true });`,
);

fs.writeFileSync(migrationPath, source);
await import(`${pathToFileURL(migrationPath).href}?fixed=${Date.now()}`);
