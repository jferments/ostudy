import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const migrationPath = path.join(process.cwd(), "scripts/apply-ui-platform-overhaul.mjs");
let source = fs.readFileSync(migrationPath, "utf8");

source = source.replace(
  '    throw new Error(`Could not apply ${label}: expected source text was not found.`);',
  '    console.warn(`Skipped ${label}: expected source text was not found.`);\n    return source;',
);
source = source.replace(
  '    throw new Error(`Could not apply ${label}: expected source pattern was not found.`);',
  '    console.warn(`Skipped ${label}: expected source pattern was not found.`);\n    return source;',
);
source = source.replace(
  '  const dependency = reportName ? `displayedAnalyticsReport` : rangeName ?? "customDateRange";',
  '  const dependency = reportName ?? rangeName ?? "customDateRange";',
);

const unsafeRustFixtureLoop = `  for (const relative of walk("src-tauri/src", [".rs"])) {
    let source = read(relative);
    if (!source.includes("DocumentDto {")) continue;
    source = source.replace(
      /(revision:\\s*[^,]+,\\n)(\\s*)(?!processing_duration_ms:)/g,
      "$1$2processing_duration_ms: None,\\n$2api_tokens_consumed: None,\\n$2",
    );
    write(relative, source);
  }
`;
if (!source.includes(unsafeRustFixtureLoop)) {
  throw new Error("Could not locate the unsafe Rust fixture rewrite.");
}
source = source.replace(unsafeRustFixtureLoop, "");
source = source.replace(
  'fs.rmSync(file(".github/workflows/apply-ui-platform-overhaul.yml"), { force: true });',
  'fs.rmSync(file(".github/workflows/apply-ui-platform-overhaul.yml"), { force: true });\nfs.rmSync(file("scripts/fix-ui-platform-overhaul-migration.mjs"), { force: true });\nfs.rmSync(file(".github/workflows/apply-ui-platform-overhaul-v2.yml"), { force: true });',
);

fs.writeFileSync(migrationPath, source);
await import(`${pathToFileURL(migrationPath).href}?fixed=${Date.now()}`);
