import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { ESLint } from "eslint";
import ts from "typescript";

const root = fileURLToPath(new URL("../..", import.meta.url));
const engine = new ESLint({ cwd: root });
const read = (path) => readFileSync(join(root, path), "utf8");
const configured = ts.readConfigFile(join(root, "tsconfig.app.json"), ts.sys.readFile);
assert.equal(configured.error, undefined);
const parsed = ts.parseJsonConfigFileContent(configured.config, ts.sys, root);
assert.deepEqual(parsed.errors, []);

async function lint(source) {
  const [result] = await engine.lintText(source, {
    filePath: join(root, "src/__hook_contract__.tsx"),
  });
  assert.ok(result);
  return result.messages;
}

function typeDiagnostics(source) {
  const file = join(root, "src/__indexed_access_contract__.ts");
  const options = {
    ...parsed.options,
    noEmit: true,
    incremental: false,
    composite: false,
    tsBuildInfoFile: undefined,
    types: [],
  };
  const host = ts.createCompilerHost(options);
  const originalSourceFile = host.getSourceFile.bind(host);
  host.getSourceFile = (path, languageVersion, onError, shouldCreateNewSourceFile) =>
    path === file
      ? ts.createSourceFile(path, source, languageVersion, true)
      : originalSourceFile(path, languageVersion, onError, shouldCreateNewSourceFile);
  const program = ts.createProgram([file], options, host);
  return ts.getPreEmitDiagnostics(program);
}

test("production and tests enforce only the two requested hook correctness rules", async () => {
  for (const path of ["src/App.tsx", "src/platform/PlatformProvider.test.tsx"]) {
    const config = await engine.calculateConfigForFile(join(root, path));
    assert.ok(config, `${path} must not be ignored`);
    assert.deepEqual(Object.keys(config.rules).sort(), [
      "react-hooks/exhaustive-deps",
      "react-hooks/rules-of-hooks",
    ]);
    for (const rule of Object.values(config.rules)) assert.equal(rule[0], 2);
    assert.equal(config.linterOptions.reportUnusedDisableDirectives, 2);
  }
});

test("a conditional hook fails the real lint configuration", async () => {
  const messages = await lint(`
    import { useEffect } from "react";
    export function Component({ enabled }) {
      if (enabled) useEffect(() => {}, []);
      return null;
    }
  `);
  assert.ok(messages.some((message) => message.ruleId === "react-hooks/rules-of-hooks"));
});

test("missing effect dependencies fail and explicit dependencies pass", async () => {
  const missing = await lint(`
    import { useEffect } from "react";
    export function Component({ value, onChange }) {
      useEffect(() => onChange(value), []);
      return null;
    }
  `);
  assert.ok(missing.some((message) => message.ruleId === "react-hooks/exhaustive-deps"));
  const valid = await lint(`
    import { useEffect } from "react";
    export function Component({ value, onChange }) {
      useEffect(() => onChange(value), [value, onChange]);
      return null;
    }
  `);
  assert.deepEqual(valid, []);
});

test("obsolete suppressions fail instead of accumulating silently", async () => {
  const messages = await lint(`
    // eslint-disable-next-line react-hooks/exhaustive-deps
    export function Component() { return null; }
  `);
  assert.ok(messages.some((message) => message.severity === 2 && message.message.includes("Unused eslint-disable")));
});

test("unchecked indexed access is enabled for all application and test source", () => {
  assert.equal(parsed.options.strict, true);
  assert.equal(parsed.options.noUncheckedIndexedAccess, true);
  assert.deepEqual(configured.config.include, ["src"]);
  assert.equal(configured.config.exclude, undefined);
  assert.ok(typeDiagnostics(`
    const values: readonly string[] = [];
    export const count = values[0].length;
  `).some((diagnostic) => diagnostic.code === 2532));
  assert.deepEqual(typeDiagnostics(`
    const values: readonly string[] = [];
    const first = values[0];
    export const count = first === undefined ? 0 : first.length;
  `), []);
});

test("the maintained static gate executes hook lint without fixes or a warning budget", () => {
  const pkg = JSON.parse(read("package.json"));
  assert.equal(pkg.scripts.lint, "eslint src --max-warnings=0");
  const gate = read("scripts/check-static.sh");
  assert.ok(gate.includes("pnpm run lint"));
  assert.ok(gate.includes("frontend-correctness.node-test.mjs"));
});
