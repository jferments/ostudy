#!/usr/bin/env node

import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { dirname, extname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const root = fileURLToPath(new URL("..", import.meta.url));
const mode = process.argv[2] ?? "--check";
if (!["--check", "--write", "--print"].includes(mode)) {
  console.error("usage: node scripts/generate-ipc-command-contracts.mjs [--check|--write|--print]");
  process.exit(2);
}

const rustRootPath = join(root, "src-tauri", "src", "lib.rs");
const generatedPath = join(root, "src", "ipc", "generated", "nativeCommands.ts");
const registered = await registeredCommands(rustRootPath);
const gateways = await gatewayContracts(join(root, "src", "ipc"));

const missing = [...registered].filter((command) => !gateways.has(command));
const extra = [...gateways.keys()].filter((command) => !registered.has(command));
if (missing.length > 0 || extra.length > 0) {
  console.error("Generated IPC contract inventory does not match the native registry.");
  if (missing.length > 0) console.error("Missing gateways: " + missing.join(", "));
  if (extra.length > 0) console.error("Unregistered gateway invokes: " + extra.join(", "));
  process.exit(1);
}

const output = render([...registered].sort(), gateways);
if (mode === "--print") {
  process.stdout.write(output);
  process.exit(0);
}
if (mode === "--write") {
  await mkdir(dirname(generatedPath), { recursive: true });
  await writeFile(generatedPath, output, "utf8");
  console.log(`Wrote ${relative(root, generatedPath)} (${registered.size} commands).`);
  process.exit(0);
}

let current = null;
try {
  current = await readFile(generatedPath, "utf8");
} catch (error) {
  if (error?.code !== "ENOENT") throw error;
}
if (current !== output) {
  const bytes = Buffer.from(output, "utf8");
  const gitHash = createHash("sha1")
    .update(Buffer.from(`blob ${bytes.length}\0`, "utf8"))
    .update(bytes)
    .digest("hex");
  console.error("Generated IPC command contracts are stale or missing.");
  console.error("Run: node scripts/generate-ipc-command-contracts.mjs --write");
  console.log(`IPC_COMMAND_CONTRACT_CANDIDATE_GIT_BLOB=${gitHash}`);
  console.log("IPC_COMMAND_CONTRACT_CANDIDATE_BEGIN");
  process.stdout.write(output);
  if (!output.endsWith("\n")) process.stdout.write("\n");
  console.log("IPC_COMMAND_CONTRACT_CANDIDATE_END");
  process.exit(1);
}
console.log(`OK   generated IPC command contracts: ${registered.size} commands.`);

async function registeredCommands(libPath) {
  const rustRoot = await readFile(libPath, "utf8");
  const rustFiles = (await collectFiles(join(root, "src-tauri", "src"))).filter((path) =>
    path.endsWith(".rs"),
  );
  const definitionsByFunction = new Map();
  const pattern =
    /#\[tauri::command(?<attribute>\s*\([^\]]*\))?\]\s*pub(?:\([^)]*\))?\s+(?:async\s+)?fn\s+(?<function>[a-z][a-z0-9_]*)/gu;
  for (const path of rustFiles) {
    const source = await readFile(path, "utf8");
    for (const match of source.matchAll(pattern)) {
      const functionName = match.groups?.function;
      if (!functionName) continue;
      const rename = match.groups?.attribute?.match(
        /\brename\s*=\s*"([a-z][a-z0-9_]*)"/u,
      )?.[1];
      const values = definitionsByFunction.get(functionName) ?? [];
      values.push(rename ?? functionName);
      definitionsByFunction.set(functionName, values);
    }
  }

  const handlerBody = rustRoot.match(/tauri::generate_handler!\s*\[([\s\S]*?)\]\s*\)/u)?.[1];
  if (!handlerBody) throw new Error("Could not locate the Tauri generate_handler registry.");
  const commands = new Set();
  for (const rawEntry of handlerBody.split(",")) {
    const entry = rawEntry.replace(/\/\/.*$/gmu, "").trim();
    if (entry === "") continue;
    const functionName = entry.split("::").at(-1)?.trim();
    const definitions = definitionsByFunction.get(functionName) ?? [];
    if (definitions.length !== 1) {
      throw new Error(`Registered command ${entry} resolved to ${definitions.length} native definitions.`);
    }
    const command = definitions[0];
    if (commands.has(command)) throw new Error(`Native command is registered twice: ${command}`);
    commands.add(command);
  }
  return commands;
}

async function gatewayContracts(ipcRoot) {
  const output = new Map();
  for (const path of await collectFiles(ipcRoot)) {
    if (!/\.[cm]?[jt]sx?$/u.test(path)) continue;
    const display = relative(root, path).replaceAll(sep, "/");
    if (
      display.includes("/generated/") ||
      /\.(?:test|spec)\.[cm]?[jt]sx?$/u.test(display) ||
      display.endsWith("src/ipc/transport.ts") ||
      display.endsWith("src/ipc/libraryTransport.ts")
    ) {
      continue;
    }
    const sourceText = await readFile(path, "utf8");
    const sourceFile = ts.createSourceFile(
      path,
      sourceText,
      ts.ScriptTarget.ES2022,
      true,
      extname(path).toLowerCase().includes("x") ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
    );
    visit(sourceFile, (node) => {
      if (
        !ts.isCallExpression(node) ||
        !ts.isIdentifier(node.expression) ||
        node.expression.text !== "invoke" ||
        node.arguments.length === 0 ||
        !ts.isStringLiteralLike(node.arguments[0])
      ) {
        return;
      }
      const command = node.arguments[0].text;
      const owner = exportedOwner(node);
      if (!owner) {
        throw new Error(`Native invoke ${command} in ${display} is not owned by an exported gateway function.`);
      }
      const modulePath = relative(
        join(root, "src", "ipc", "generated"),
        path.slice(0, -extname(path).length),
      ).replaceAll(sep, "/");
      const normalizedModule = modulePath.startsWith(".") ? modulePath : `./${modulePath}`;
      const contract = { module: normalizedModule, functionName: owner };
      const existing = output.get(command) ?? [];
      if (!existing.some(
        (candidate) =>
          candidate.module === contract.module && candidate.functionName === contract.functionName,
      )) {
        existing.push(contract);
        existing.sort((left, right) =>
          left.module.localeCompare(right.module) || left.functionName.localeCompare(right.functionName),
        );
      }
      output.set(command, existing);
    });
  }
  return output;
}

function exportedOwner(node) {
  let current = node.parent;
  while (current) {
    if (ts.isFunctionDeclaration(current) && current.name && isExported(current)) {
      return current.name.text;
    }
    if (ts.isArrowFunction(current) || ts.isFunctionExpression(current)) {
      const declaration = current.parent;
      const statement = declaration?.parent?.parent;
      if (
        ts.isVariableDeclaration(declaration) &&
        ts.isIdentifier(declaration.name) &&
        statement &&
        ts.isVariableStatement(statement) &&
        isExported(statement)
      ) {
        return declaration.name.text;
      }
    }
    current = current.parent;
  }
  return null;
}

function isExported(node) {
  return Boolean(node.modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword));
}

function render(commands, gateways) {
  let output =
    "/* Generated by scripts/generate-ipc-command-contracts.mjs. Do not edit by hand. */\n" +
    "/* Rust registration owns command membership; all exported gateway owners contribute application request/result types. */\n\n" +
    "export interface NativeCommandContractMap {\n";
  for (const command of commands) {
    const owners = gateways.get(command);
    if (!owners || owners.length === 0) {
      throw new Error(`Native command ${command} has no typed frontend gateway owner.`);
    }
    const types = owners.map(
      (gateway) => `typeof import(${JSON.stringify(gateway.module)}).${gateway.functionName}`,
    );
    output += `  ${JSON.stringify(command)}: {\n`;
    output += "    gateways: readonly [\n";
    for (const type of types) output += `      ${type},\n`;
    output += "    ];\n";
    output += `    request: ${types.map((type) => `Parameters<${type}>`).join(" | ")};\n`;
    output += `    result: ${types.map((type) => `Awaited<ReturnType<${type}>>`).join(" | ")};\n`;
    output += "  };\n";
  }
  output += "}\n\n";
  output += "export type NativeCommandName = keyof NativeCommandContractMap;\n\n";
  output += "export const NATIVE_COMMAND_NAMES = [\n";
  for (const command of commands) output += `  ${JSON.stringify(command)},\n`;
  output += "] as const satisfies readonly NativeCommandName[];\n";
  return output;
}

function visit(node, callback) {
  callback(node);
  node.forEachChild((child) => visit(child, callback));
}

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await collectFiles(path)));
    else if (entry.isFile()) files.push(resolve(path));
  }
  return files;
}
