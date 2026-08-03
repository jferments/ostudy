import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const root = join(scriptDirectory, "..");
const env = new Map(
  (await readFile(join(root, "versions.env"), "utf8"))
    .split(/\r?\n/u)
    .filter((line) => line && !line.startsWith("#"))
    .map((line) => {
      const index = line.indexOf("=");
      return [line.slice(0, index), line.slice(index + 1)];
    }),
);
const manifest = JSON.parse(await readFile(join(root, "package.json"), "utf8"));

const expected = new Map([
  ["@tauri-apps/api", env.get("TAURI_API_VERSION")],
  ["@tauri-apps/cli", env.get("TAURI_CLI_VERSION")],
  ["@tauri-apps/plugin-dialog", env.get("TAURI_DIALOG_VERSION")],
  ["@tauri-apps/plugin-opener", env.get("TAURI_OPENER_VERSION")],
  ["react", env.get("REACT_VERSION")],
  ["react-dom", env.get("REACT_VERSION")],
  ["chart.js", env.get("CHART_JS_VERSION")],
  ["react-chartjs-2", env.get("REACT_CHARTJS_2_VERSION")],
  ["@types/react", env.get("REACT_TYPES_VERSION")],
  ["@types/react-dom", env.get("REACT_DOM_TYPES_VERSION")],
  ["@vitejs/plugin-react", env.get("VITE_REACT_PLUGIN_VERSION")],
  ["typescript", env.get("TYPESCRIPT_VERSION")],
  ["vite", env.get("VITE_VERSION")],
  ["esbuild", env.get("ESBUILD_VERSION")],
  ["vitest", env.get("VITEST_VERSION")],
  ["@testing-library/react", env.get("TESTING_LIBRARY_REACT_VERSION")],
  ["@testing-library/dom", env.get("TESTING_LIBRARY_DOM_VERSION")],
  ["jsdom", env.get("JSDOM_VERSION")],
]);
for (const name of Object.keys({
  ...manifest.dependencies,
  ...manifest.devDependencies,
}).filter((name) => name.startsWith("@tiptap/"))) {
  expected.set(name, env.get("TIPTAP_VERSION"));
}

const failures = [];
for (const [name, version] of expected) {
  const packageFile = join(root, "node_modules", name, "package.json");
  try {
    const found = JSON.parse(await readFile(packageFile, "utf8")).version;
    if (found !== version) failures.push(`${name}: expected ${version}, found ${found}`);
    else console.log(`OK   ${name}@${found}`);
  } catch (error) {
    failures.push(`${name}: could not inspect installed package (${String(error)})`);
  }
}

try {
  const data = JSON.parse(
    execFileSync("pnpm", ["list", "--json", "--depth", "Infinity"], {
      cwd: root,
      encoding: "utf8",
    }),
  );
  const seen = new Map();
  function walk(node) {
    if (!node || typeof node !== "object") return;
    for (const [name, value] of Object.entries(node.dependencies ?? {})) {
      if (name.startsWith("@tiptap/")) {
        const version = value?.version ?? "unknown";
        if (!seen.has(name)) seen.set(name, new Set());
        seen.get(name).add(version);
      }
      walk(value);
    }
  }
  for (const item of data) walk(item);
  for (const [name, versions] of seen) {
    if (versions.size !== 1 || !versions.has(env.get("TIPTAP_VERSION"))) {
      failures.push(`${name}: mixed or unexpected graph ${[...versions].join(", ")}`);
    }
  }
} catch (error) {
  failures.push(`Could not inspect complete pnpm graph: ${String(error)}`);
}

if (failures.length > 0) {
  console.error("\nJavaScript dependency verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log("OK   Exact JavaScript versions and the single-version Tiptap graph are valid.");
