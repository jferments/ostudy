import { readFileSync, statSync } from "node:fs";
import { pathToFileURL } from "node:url";

const MAX_LOG_BYTES = 2 * 1024 * 1024;
const MAX_DIFF_BYTES = 512 * 1024;

/** Report existing check-only diagnostics. Never invoke a formatter or edit source. */
export function rustfmtDiagnostics(input, maximumBytes = MAX_DIFF_BYTES) {
  const text = input.replace(/\x1b\[[0-9;]*m/g, "");
  if (!text.includes("STATIC CHECK FAILED during: Rust formatting")) return "";
  const lines = text.split("\n");
  const hunks = [];
  let current = null;
  for (const line of lines) {
    if (/^Diff in .+\.rs:\d+:$/.test(line)) {
      if (current) hunks.push(current.join("\n") + "\n");
      current = [line];
    } else if (current && (/^[ +\-]/.test(line) || line === "")) {
      current.push(line);
    } else if (current) {
      hunks.push(current.join("\n") + "\n");
      current = null;
    }
  }
  if (current) hunks.push(current.join("\n") + "\n");
  const uniqueHunks = [];
  const seenHunks = new Set();
  for (const hunk of hunks) {
    if (seenHunks.has(hunk)) continue;
    seenHunks.add(hunk);
    uniqueHunks.push(hunk);
  }
  if (uniqueHunks.length === 0) return "Rust formatting failed; no complete diff headers were captured.\n";
  const selected = [];
  let bytes = 0;
  for (const hunk of uniqueHunks) {
    const size = Buffer.byteLength(hunk);
    if (bytes + size > maximumBytes) break;
    selected.push(hunk);
    bytes += size;
  }
  const omitted = uniqueHunks.length - selected.length;
  return [
    "--- read-only Rust formatting diagnostics ---",
    `Complete hunks: ${selected.length}/${uniqueHunks.length}; bytes: ${bytes}; omitted hunks: ${omitted}.`,
    ...selected,
    omitted ? "INCOMPLETE: the diagnostic byte limit was reached; no partial hunk was emitted." : "All captured Rust formatting hunks are included.",
    "--- end read-only Rust formatting diagnostics ---\n",
  ].join("\n");
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const path = process.argv[2];
  if (!path) throw new Error("Usage: report-rustfmt-diffs.mjs <captured-log>");
  if (statSync(path).size > MAX_LOG_BYTES) {
    process.stderr.write("Rust formatting diagnostics exceed the bounded log-read limit; the original log is retained.\n");
  } else {
    process.stdout.write(rustfmtDiagnostics(readFileSync(path, "utf8")));
  }
}
