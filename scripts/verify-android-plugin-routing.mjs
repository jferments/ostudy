#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const mobile = await read("plugins/ostudy-platform/src/mobile.rs");
const pluginRoot = await read("plugins/ostudy-platform/src/lib.rs");
const compositionRoot = await read("src-tauri/src/lib.rs");
const emulatorSmoke = await read("scripts/android-emulator-smoke.sh");
const routingInstrumentation = await read(
  "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidPlatformRoutingTest.kt",
);

const routes = [
  {
    initializer: "init",
    constructor: "init",
    builder: "ostudy-platform",
    className: "OstudyPlatformPlugin",
    kotlin: "plugins/ostudy-platform/android/src/main/java/OstudyPlatformPlugin.kt",
    commands: [
      "getEnvironment",
      "timeZoneSnapshot",
      "localDateAt",
      "firstValidInstant",
      "acquireImport",
      "publishExport",
      "shareFile",
      "setBackInterception",
      "secretAvailability",
      "readSecret",
      "writeSecret",
      "deleteSecret",
      "listIncomingShares",
      "takeIncomingShare",
      "dismissIncomingShare",
    ],
  },
  {
    initializer: "init_documents",
    constructor: "init_documents",
    builder: "ostudy-documents",
    className: "OstudyDocumentPlugin",
    kotlin: "plugins/ostudy-platform/android/src/main/java/OstudyDocumentPlugin.kt",
    commands: [
      "documentCapabilities",
      "openPdf",
      "renderPdfPage",
      "closePdf",
      "recognizeImage",
      "beginBackgroundExecution",
      "updateBackgroundExecution",
      "endBackgroundExecution",
    ],
  },
  {
    initializer: "init_qr",
    constructor: "init_qr",
    builder: "ostudy-qr",
    className: "OstudyQrPlugin",
    kotlin: "plugins/ostudy-platform/android/src/main/java/OstudyQrPlugin.kt",
    commands: ["scanQrCode"],
  },
];

const failures = [];
const builderNames = new Set(routes.map((route) => route.builder));
if (builderNames.size !== routes.length) {
  failures.push("Every Android native plugin class must have a distinct Tauri Builder name.");
}

let registrationCount = 0;
for (const route of routes) {
  const initializer = functionBlock(mobile, route.initializer);
  const constructor = functionBlock(pluginRoot, route.constructor);
  const kotlin = await read(route.kotlin);
  const registrations = initializer.match(/register_android_plugin\s*\(/gu) ?? [];
  registrationCount += registrations.length;

  if (registrations.length !== 1) {
    failures.push(
      `${route.initializer} must register exactly one Android plugin class; found ${registrations.length}. ` +
        "Tauri routes PluginHandle calls by the Rust Builder name, so registering several classes through one PluginApi aliases them onto one route.",
    );
  }
  expectContains(initializer, `"${route.className}"`, `${route.initializer} native class`);
  expectContains(constructor, `Builder::new("${route.builder}")`, `${route.constructor} Tauri Builder name`);
  expectContains(
    compositionRoot,
    `tauri_plugin_ostudy_platform::${route.constructor}()`,
    `${route.constructor} composition-root registration`,
  );

  for (const command of route.commands) {
    expectMatches(
      mobile,
      new RegExp(`run_mobile_plugin\\s*\\(\\s*"${escapeRegExp(command)}"`, "u"),
      `Rust mobile call ${command}`,
    );
    expectMatches(
      kotlin,
      new RegExp(`fun\\s+${escapeRegExp(command)}\\s*\\(`, "u"),
      `${route.className} command ${command}`,
    );
  }
}

if (registrationCount !== routes.length) {
  failures.push(
    `Expected ${routes.length} Android plugin registrations across distinct initializers, found ${registrationCount}.`,
  );
}

for (const [leftIndex, left] of routes.entries()) {
  for (const right of routes.slice(leftIndex + 1)) {
    if (left.builder === right.builder) {
      failures.push(`${left.className} and ${right.className} share Builder name ${left.builder}.`);
    }
  }
}

expectContains(
  routingInstrumentation,
  "fun rustPlatformSnapshotReachesDocumentAndQrNativeRoutes()",
  "runtime Rust-to-native plugin instrumentation regression test",
);
expectContains(
  routingInstrumentation,
  'assertEquals("available", health.getString("nativePdfRender"))',
  "runtime native PDF route assertion",
);
expectContains(
  routingInstrumentation,
  'assertEquals("available", health.getString("ocr"))',
  "runtime ML Kit route assertion",
);
expectContains(
  emulatorSmoke,
  "run_case rustPlatformSnapshotReachesDocumentAndQrNativeRoutes AndroidPlatformRoutingTest",
  "runtime native plugin routing smoke invocation",
);

if (failures.length > 0) {
  console.error("Android native-plugin routing contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "OK   Android native plugin routes are distinct, command-complete, and guarded by runtime instrumentation.",
);

async function read(relativePath) {
  return readFile(join(root, relativePath), "utf8");
}

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}

function expectMatches(source, pattern, label) {
  if (!pattern.test(source)) failures.push(`Missing ${label}: ${pattern}`);
}

function functionBlock(source, name) {
  const match = new RegExp(`\\bfn\\s+${escapeRegExp(name)}\\b`, "u").exec(source);
  if (!match) {
    failures.push(`Missing Rust function ${name}.`);
    return "";
  }
  const open = source.indexOf("{", match.index);
  if (open < 0) {
    failures.push(`Rust function ${name} has no body.`);
    return "";
  }
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(match.index, index + 1);
    }
  }
  failures.push(`Rust function ${name} has an unterminated body.`);
  return source.slice(match.index);
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}
