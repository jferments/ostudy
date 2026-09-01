#!/usr/bin/env node

import { access, readFile, readdir } from "node:fs/promises";
import { extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const defaultRoot = fileURLToPath(new URL("..", import.meta.url));
const root = resolve(parseRoot(process.argv.slice(2)) ?? defaultRoot);
const contractPath = join(root, "scripts", "route-smoke-contract.json");
const failures = [];

let contract;
try {
  contract = JSON.parse(await readFile(contractPath, "utf8"));
} catch (error) {
  fail(`Could not read scripts/route-smoke-contract.json: ${error.message}`);
  finish();
}

validateContractShape(contract);
if (failures.length > 0) finish();

const sources = {};
for (const [name, path] of Object.entries(contract.authoritativeSources)) {
  sources[name] = await readRequired(path, `authoritative ${name} source`);
}

const protectedWorkflowById = new Map(
  contract.protectedWorkflows.map((workflow) => [workflow.id, workflow]),
);

if (sources.applicationSections !== null) {
  compareOrderedValues(
    "application sections",
    contract.applicationSections.map((entry) => entry.id),
    parseApplicationSections(sources.applicationSections),
  );
}
if (sources.applicationWorkflows !== null) {
  compareOrderedValues(
    "application workflows",
    contract.applicationWorkflows.map((entry) => entry.id),
    parseApplicationWorkflows(sources.applicationWorkflows),
  );
}
if (sources.checkpointGateway !== null) {
  compareOrderedValues(
    "checkpoint destinations",
    contract.checkpointDestinations,
    parseCheckpointSections(sources.checkpointGateway),
  );
}
if (sources.applicationShell !== null) {
  compareOrderedValues(
    "rendered App shell destinations",
    contract.applicationSections
      .filter((entry) => entry.role === "primary-destination")
      .map((entry) => entry.id),
    parseRenderedAppSections(sources.applicationShell),
  );
  compareOrderedValues(
    "rendered App shell workflows",
    contract.applicationWorkflows.map((entry) => entry.id),
    parseRenderedAppWorkflows(sources.applicationShell),
  );
}
if (sources.primaryNavigation !== null) {
  compareOrderedRecords(
    "primary navigation destinations",
    contract.primaryDestinations.map((entry) => ({
      id: entry.section,
      label: entry.label,
    })),
    parseNavigationItems(sources.primaryNavigation, "ITEMS", "section"),
  );
}
if (sources.compactNavigation !== null) {
  const visibleDestinations = parseNavigationItems(
    sources.compactNavigation,
    "ITEMS",
    "section",
  ).map((entry) => entry.id);
  const compactPrimary = parseStringArray(
    sources.compactNavigation,
    "COMPACT_PRIMARY_SECTIONS",
  );
  const compactPrimarySet = new Set(compactPrimary);
  compareOrderedValues(
    "compact primary destinations",
    contract.compactNavigation.primary,
    compactPrimary,
  );
  compareOrderedValues(
    "compact More destinations",
    contract.compactNavigation.more,
    visibleDestinations.filter((section) => !compactPrimarySet.has(section)),
  );
}
if (sources.flashcardsRoutes !== null) {
  compareOrderedValues(
    "Flashcards route workspaces",
    contract.flashcardsWorkspaces.map((entry) => entry.id),
    parseStringArray(sources.flashcardsRoutes, "flashcardsWorkspaceNames"),
  );
}
if (sources.flashcardsTabs !== null) {
  compareOrderedRecords(
    "Flashcards workspace tabs",
    contract.flashcardsWorkspaces.map((entry) => ({ id: entry.id, label: entry.label })),
    parseNavigationItems(sources.flashcardsTabs, "WORKSPACES", "value"),
  );
}

const nativeSmokeEntrypoints =
  sources.nativeSmokeRunner === null
    ? new Set()
    : new Set(parseNativeSmokeEntrypoints(sources.nativeSmokeRunner));
const androidScheduledCases =
  sources.androidSmokeRunner === null
    ? new Set()
    : new Set(parseAndroidScheduledCases(sources.androidSmokeRunner));
const smokeExecution = { nativeSmokeEntrypoints, androidScheduledCases };

await validateSmokeEvidence(
  contract.primaryDestinations.filter(
    (entry) => !isProtectedWorkflowReference(entry.smoke),
  ),
  "primary destination",
  "section",
  smokeExecution,
);
await validateSmokeEvidence(
  contract.flashcardsWorkspaces,
  "Flashcards workspace",
  "id",
  smokeExecution,
);
await validateSmokeEvidence(
  contract.protectedWorkflows,
  "protected workflow",
  "id",
  smokeExecution,
);
await validateProductIndependence();

finish();

function validateContractShape(value) {
  if (!isRecord(value)) {
    fail("The route/smoke contract must be a JSON object.");
    return;
  }
  if (value.schemaVersion !== 1) {
    fail(`schemaVersion must be 1; found ${JSON.stringify(value.schemaVersion)}.`);
  }
  if (value.contractRole !== "expected-inventory-only") {
    fail(
      'contractRole must be "expected-inventory-only" so the manifest cannot be mistaken for application routing configuration.',
    );
  }

  const sourceKeys = [
    "applicationSections",
    "applicationWorkflows",
    "applicationShell",
    "checkpointGateway",
    "primaryNavigation",
    "compactNavigation",
    "flashcardsRoutes",
    "flashcardsTabs",
    "nativeSmokeRunner",
    "androidSmokeRunner",
  ];
  if (!isRecord(value.authoritativeSources)) {
    fail("authoritativeSources must be an object.");
  } else {
    for (const key of sourceKeys) {
      if (!isRelativePath(value.authoritativeSources[key])) {
        fail(`authoritativeSources.${key} must be a repository-relative path.`);
      }
    }
  }

  if (!Array.isArray(value.applicationSections)) {
    fail("applicationSections must be an array.");
  }
  if (!Array.isArray(value.applicationWorkflows)) {
    fail("applicationWorkflows must be an array.");
  }
  if (
    !Array.isArray(value.checkpointDestinations) ||
    value.checkpointDestinations.some((section) => !isIdentifier(section))
  ) {
    fail("checkpointDestinations must be an array of destination identifiers.");
  }
  if (!Array.isArray(value.primaryDestinations)) {
    fail("primaryDestinations must be an array.");
  }
  if (!Array.isArray(value.protectedWorkflows)) {
    fail("protectedWorkflows must be an array.");
  }
  if (!Array.isArray(value.flashcardsWorkspaces)) {
    fail("flashcardsWorkspaces must be an array.");
  }
  if (!isRecord(value.compactNavigation)) {
    fail("compactNavigation must be an object.");
  }
  if (failures.length > 0) return;

  validateApplicationSections(value.applicationSections);
  validateApplicationWorkflows(value.applicationWorkflows);
  validateProtectedWorkflowCatalog(value.protectedWorkflows);
  const protectedWorkflows = new Map(
    value.protectedWorkflows
      .filter((entry) => isRecord(entry) && isIdentifier(entry.id))
      .map((entry) => [entry.id, entry]),
  );
  validateDestinationCatalog(
    value.primaryDestinations,
    "primary destination",
    "section",
    protectedWorkflows,
    true,
  );
  validateDestinationCatalog(
    value.flashcardsWorkspaces,
    "Flashcards workspace",
    "id",
    protectedWorkflows,
    false,
  );

  const applicationIds = value.applicationSections.map((entry) => entry.id);
  reportDuplicates("application section", applicationIds);
  const workflowIds = value.applicationWorkflows.map((entry) => entry.id);
  reportDuplicates("application workflow", workflowIds);
  const primaryIds = value.primaryDestinations.map((entry) => entry.section);
  reportDuplicates("primary destination", primaryIds);
  const workspaceIds = value.flashcardsWorkspaces.map((entry) => entry.id);
  reportDuplicates("Flashcards workspace", workspaceIds);
  const protectedWorkflowIds = value.protectedWorkflows.map((entry) => entry.id);
  reportDuplicates("protected workflow", protectedWorkflowIds);
  reportDuplicates("checkpoint destination", value.checkpointDestinations);

  const declaredPrimaryIds = value.applicationSections
    .filter((entry) => entry.role === "primary-destination")
    .map((entry) => entry.id);
  compareSets("primary destination taxonomy", declaredPrimaryIds, primaryIds);
  compareOrderedValues(
    "checkpoint destination taxonomy",
    primaryIds,
    value.checkpointDestinations,
  );

  validateProtectedWorkflowCoverage(
    value.applicationWorkflows,
    value.protectedWorkflows,
  );
  validateCompactNavigation(value.compactNavigation, value.primaryDestinations);

  const primarySet = new Set(primaryIds);
  const workspaceSet = new Set(workspaceIds);
  for (const entry of value.applicationSections) {
    if (entry.role !== "compatibility-alias") continue;
    const [section, workspace, ...remainder] = entry.target.split("/");
    if (remainder.length > 0 || !primarySet.has(section) || (workspace && !workspaceSet.has(workspace))) {
      fail(
        `application section ${entry.id} has invalid compatibility target ${JSON.stringify(entry.target)}.`,
      );
    }
  }
}

function validateCompactNavigation(compactNavigation, primaryDestinations) {
  const groups = ["primary", "more"];
  for (const group of groups) {
    if (
      !Array.isArray(compactNavigation[group]) ||
      compactNavigation[group].some((section) => !isIdentifier(section))
    ) {
      fail(`compactNavigation.${group} must be an array of destination identifiers.`);
      return;
    }
    reportDuplicates(`compact ${group} destination`, compactNavigation[group]);
  }

  const visible = primaryDestinations.map((entry) => entry.section);
  const compactPrimary = compactNavigation.primary;
  const compactMore = compactNavigation.more;
  reportDuplicates("compact destination", [...compactPrimary, ...compactMore]);
  compareSets("compact navigation coverage", visible, [...compactPrimary, ...compactMore]);

  for (const [label, group] of [
    ["primary", compactPrimary],
    ["More", compactMore],
  ]) {
    const groupSet = new Set(group);
    const expectedOrder = visible.filter((section) => groupSet.has(section));
    compareOrderedValues(`compact ${label} destination order`, expectedOrder, group);
  }
}

function validateApplicationSections(entries) {
  for (const [index, entry] of entries.entries()) {
    const label = `applicationSections[${index}]`;
    if (!isRecord(entry) || !isIdentifier(entry.id)) {
      fail(`${label}.id must be a non-empty route identifier.`);
      continue;
    }
    if (!["primary-destination", "compatibility-alias"].includes(entry.role)) {
      fail(
        `${label}.role must be primary-destination or compatibility-alias.`,
      );
    }
    if (entry.role === "compatibility-alias" && !isIdentifier(entry.target, true)) {
      fail(`${label}.target must name the primary destination and optional workspace.`);
    }
    if (entry.role !== "compatibility-alias" && "target" in entry) {
      fail(`${label} is not an alias and must not declare an alias target.`);
    }
  }
}

function validateApplicationWorkflows(entries) {
  for (const [index, entry] of entries.entries()) {
    const label = `applicationWorkflows[${index}]`;
    if (!isRecord(entry) || !isIdentifier(entry.id)) {
      fail(`${label}.id must be a non-empty workflow identifier.`);
      continue;
    }
    if (!isIdentifier(entry.protectedWorkflow)) {
      fail(`${label}.protectedWorkflow must name its protected smoke workflow.`);
    }
  }
}

function validateProtectedWorkflowCatalog(entries) {
  for (const [index, entry] of entries.entries()) {
    const label = `protected workflow ${isRecord(entry) ? entry.id : index}`;
    if (!isRecord(entry) || !isIdentifier(entry.id)) {
      fail(`protectedWorkflows[${index}].id must be a non-empty workflow identifier.`);
      continue;
    }
    if (!isIdentifier(entry.workflow)) {
      fail(`${label}.workflow must be a non-empty application workflow identifier.`);
    }
    if (typeof entry.label !== "string" || entry.label.trim() === "") {
      fail(`${label} must have a non-empty label.`);
    }
    validateSmokeShape(entry.smoke, label);
  }
}

function validateDestinationCatalog(
  entries,
  catalog,
  idKey,
  protectedWorkflows,
  allowProtectedWorkflow,
) {
  for (const [index, entry] of entries.entries()) {
    const label = `${catalog} ${isRecord(entry) ? entry[idKey] : index}`;
    if (!isRecord(entry) || !isIdentifier(entry[idKey])) {
      fail(`${catalog}[${index}].${idKey} must be a non-empty route identifier.`);
      continue;
    }
    if (typeof entry.label !== "string" || entry.label.trim() === "") {
      fail(`${label} must have a non-empty label.`);
    }
    if (isProtectedWorkflowReference(entry.smoke)) {
      if (!allowProtectedWorkflow) {
        fail(`${label} cannot reference a protected workflow.`);
        continue;
      }
      const workflow = protectedWorkflows.get(entry.smoke.protectedWorkflow);
      if (!workflow) {
        fail(
          `${label} references undeclared protected workflow ${JSON.stringify(entry.smoke.protectedWorkflow)}.`,
        );
      } else if (workflow.workflow !== entry[idKey]) {
        fail(
          `${label} references protected workflow ${workflow.id} for workflow ${workflow.workflow}.`,
        );
      }
      continue;
    }
    validateSmokeShape(entry.smoke, label);
  }
}

function validateProtectedWorkflowCoverage(applicationWorkflows, protectedWorkflows) {
  const protectedById = new Map(
    protectedWorkflows.map((workflow) => [workflow.id, workflow]),
  );
  const applicationById = new Map(
    applicationWorkflows.map((workflow) => [workflow.id, workflow]),
  );

  for (const applicationWorkflow of applicationWorkflows) {
    const protectedWorkflow = protectedById.get(applicationWorkflow.protectedWorkflow);
    if (!protectedWorkflow) {
      fail(`internal workflow ${applicationWorkflow.id} is missing protected smoke coverage.`);
    } else if (protectedWorkflow.workflow !== applicationWorkflow.id) {
      fail(
        `internal workflow ${applicationWorkflow.id} references protected workflow ${protectedWorkflow.id} for ${protectedWorkflow.workflow}.`,
      );
    }
  }

  for (const protectedWorkflow of protectedWorkflows) {
    if (!applicationById.has(protectedWorkflow.workflow)) {
      fail(
        `protected workflow ${protectedWorkflow.id} references unknown application workflow ${protectedWorkflow.workflow}.`,
      );
    }
  }
}

function validateSmokeShape(smoke, label) {
  if (!isRecord(smoke) || !["route-entry", "presence"].includes(smoke.coverage)) {
    fail(`${label} smoke.coverage must be route-entry or presence.`);
    return;
  }
  if (!Array.isArray(smoke.evidence) || smoke.evidence.length === 0) {
    fail(`${label} must declare at least one smoke evidence record.`);
    return;
  }
  for (const [index, evidence] of smoke.evidence.entries()) {
    const evidenceLabel = `${label} evidence[${index}]`;
    if (!isRecord(evidence)) {
      fail(`${evidenceLabel} must be an object.`);
      continue;
    }
    if (!["native-webdriver", "android-emulator"].includes(evidence.runner)) {
      fail(`${evidenceLabel}.runner must be native-webdriver or android-emulator.`);
    }
    if (!isRelativePath(evidence.path) || !isSmokePath(evidence.runner, evidence.path)) {
      fail(`${evidenceLabel}.path must belong to the declared executable smoke runner.`);
    }
    validateEvidenceExecutionShape(evidence, evidenceLabel);
    if (smoke.coverage === "route-entry") {
      validateFragments(evidence.entryFragments, `${evidenceLabel}.entryFragments`);
      validateFragments(evidence.assertionFragments, `${evidenceLabel}.assertionFragments`);
      if ("presenceFragments" in evidence) {
        fail(`${evidenceLabel} is route-entry evidence and must not use presenceFragments.`);
      }
    } else {
      validateFragments(evidence.presenceFragments, `${evidenceLabel}.presenceFragments`);
      if ("entryFragments" in evidence || "assertionFragments" in evidence) {
        fail(`${evidenceLabel} is presence evidence and must not claim route entry/assertion fragments.`);
      }
    }
  }
}

function validateEvidenceExecutionShape(evidence, label) {
  if (evidence.runner === "native-webdriver") {
    if (
      !isRelativePath(evidence.entrypoint) ||
      !isSmokePath("native-webdriver", evidence.entrypoint)
    ) {
      fail(`${label}.entrypoint must name a native WebDriver script.`);
      return;
    }
    if ("instrumentationCase" in evidence) {
      fail(`${label} is native WebDriver evidence and must not declare instrumentationCase.`);
    }
    if (evidence.entrypoint === evidence.path) {
      if ("executionChain" in evidence) {
        fail(`${label} is direct runner evidence and must not declare executionChain.`);
      }
      return;
    }
    if (!Array.isArray(evidence.executionChain) || evidence.executionChain.length === 0) {
      fail(`${label}.executionChain must connect its runner entrypoint to its evidence path.`);
      return;
    }
    let expectedFrom = evidence.entrypoint;
    for (const [index, edge] of evidence.executionChain.entries()) {
      const edgeLabel = `${label}.executionChain[${index}]`;
      if (!isRecord(edge)) {
        fail(`${edgeLabel} must be an object.`);
        continue;
      }
      if (
        !isRelativePath(edge.from) ||
        !isSmokePath("native-webdriver", edge.from) ||
        !isRelativePath(edge.to) ||
        !isSmokePath("native-webdriver", edge.to)
      ) {
        fail(`${edgeLabel} must connect repository-relative native WebDriver scripts.`);
      }
      if (edge.from !== expectedFrom) {
        fail(`${edgeLabel}.from must continue from ${JSON.stringify(expectedFrom)}.`);
      }
      validateFragments(edge.fragments, `${edgeLabel}.fragments`);
      expectedFrom = edge.to;
    }
    if (expectedFrom !== evidence.path) {
      fail(`${label}.executionChain must terminate at ${JSON.stringify(evidence.path)}.`);
    }
    return;
  }

  if (evidence.runner === "android-emulator") {
    if ("entrypoint" in evidence || "executionChain" in evidence) {
      fail(`${label} is Android evidence and must not declare native execution metadata.`);
    }
    if (
      !isRecord(evidence.instrumentationCase) ||
      !isCodeIdentifier(evidence.instrumentationCase.class) ||
      !isCodeIdentifier(evidence.instrumentationCase.method)
    ) {
      fail(`${label}.instrumentationCase must declare a Kotlin class and method.`);
    }
  }
}

function validateFragments(fragments, label) {
  if (
    !Array.isArray(fragments) ||
    fragments.length === 0 ||
    fragments.some((fragment) => typeof fragment !== "string" || fragment === "")
  ) {
    fail(`${label} must be a non-empty array of non-empty strings.`);
  }
}

async function validateSmokeEvidence(entries, catalog, idKey, execution) {
  const cache = new Map();
  for (const entry of entries) {
    const label = `${catalog} ${entry[idKey]}`;
    for (const evidence of entry.smoke.evidence) {
      let source = cache.get(evidence.path);
      if (source === undefined) {
        source = await readRequired(evidence.path, `${label} smoke evidence`);
        cache.set(evidence.path, source);
      }
      if (source === null) continue;
      await validateEvidenceExecution(evidence, label, source, cache, execution);
      const groups =
        entry.smoke.coverage === "route-entry"
          ? [
              ["entry", evidence.entryFragments],
              ["assertion", evidence.assertionFragments],
            ]
          : [["presence", evidence.presenceFragments]];
      for (const [kind, fragments] of groups) {
        for (const fragment of fragments) {
          if (!source.includes(fragment)) {
            fail(
              `${label} ${entry.smoke.coverage} evidence ${evidence.path} is missing required ${kind} fragment: ${JSON.stringify(fragment)}.`,
            );
          }
        }
      }
    }
  }
}

async function validateEvidenceExecution(evidence, label, evidenceSource, cache, execution) {
  if (evidence.runner === "native-webdriver") {
    if (!execution.nativeSmokeEntrypoints.has(evidence.entrypoint)) {
      fail(
        `${label} native smoke entrypoint ${evidence.entrypoint} is not enumerated by scripts/smoke-native.sh.`,
      );
    }
    for (const edge of evidence.executionChain ?? []) {
      let source = cache.get(edge.from);
      if (source === undefined) {
        source = await readRequired(edge.from, `${label} execution-chain source`);
        cache.set(edge.from, source);
      }
      if (source === null) continue;
      for (const fragment of edge.fragments) {
        if (!source.includes(fragment)) {
          fail(
            `${label} execution chain ${edge.from} -> ${edge.to} is missing required delegation fragment: ${JSON.stringify(fragment)}.`,
          );
        }
      }
    }
    return;
  }

  if (evidence.runner === "android-emulator") {
    const instrumentationCase = evidence.instrumentationCase;
    const caseKey = `${instrumentationCase.class}#${instrumentationCase.method}`;
    if (!execution.androidScheduledCases.has(caseKey)) {
      fail(
        `${label} instrumentation case ${caseKey} is not scheduled by scripts/android-emulator-smoke.sh.`,
      );
    }
    const methodPattern = new RegExp(
      `\\bfun\\s+${escapeRegExp(instrumentationCase.method)}\\s*\\(`,
      "u",
    );
    if (!methodPattern.test(evidenceSource)) {
      fail(
        `${label} evidence ${evidence.path} does not define instrumentation method ${instrumentationCase.method}.`,
      );
    }
  }
}

async function validateProductIndependence() {
  const productRoots = [
    "src",
    "src-tauri/src",
    "src-tauri/gen/android/app/src/main",
  ];
  const supportedExtensions = new Set([".js", ".jsx", ".ts", ".tsx", ".rs", ".kt"]);
  for (const directory of productRoots) {
    const absoluteDirectory = join(root, directory);
    if (!(await exists(absoluteDirectory))) continue;
    for (const path of await collectFiles(absoluteDirectory)) {
      if (!supportedExtensions.has(extname(path))) continue;
      const source = await readFile(path, "utf8");
      if (source.includes("route-smoke-contract.json")) {
        fail(
          `${relative(root, path)} depends on scripts/route-smoke-contract.json; application routing source must remain authoritative.`,
        );
      }
    }
  }
}

function parseApplicationSections(source) {
  const match = source.match(/export\s+type\s+AppSection\s*=([\s\S]*?);/u);
  if (!match) {
    fail("Could not parse export type AppSection from its authoritative source.");
    return [];
  }
  return [...match[1].matchAll(/"([^"]+)"/gu)].map((entry) => entry[1]);
}

function parseApplicationWorkflows(source) {
  const match = source.match(/export\s+type\s+AppWorkflow\s*=([\s\S]*?);/u);
  if (!match) {
    fail("Could not parse export type AppWorkflow from its authoritative source.");
    return [];
  }
  return [...match[1].matchAll(/"([^"]+)"/gu)].map((entry) => entry[1]);
}

function parseCheckpointSections(source) {
  const match = source.match(/export\s+type\s+CheckpointSection\s*=([\s\S]*?);/u);
  if (!match) {
    fail("Could not parse export type CheckpointSection from its authoritative source.");
    return [];
  }
  return [...match[1].matchAll(/"([^"]+)"/gu)].map((entry) => entry[1]);
}

function parseRenderedAppSections(source) {
  const sections = [
    ...source.matchAll(
      /(?:\{|:\s*)controller\.section\s*===\s*"([^"]+)"\s*\?\s*\(/gu,
    ),
  ].map((entry) => entry[1]);
  if (sections.length === 0) {
    fail("Could not parse rendered destination branches from src/App.tsx.");
  }
  return sections;
}

function parseRenderedAppWorkflows(source) {
  const workflows = [
    ...source.matchAll(/\{controller\.activeWorkflow\s*===\s*"([^"]+)"\s*\?\s*\(/gu),
  ].map((entry) => entry[1]);
  if (workflows.length === 0) {
    fail("Could not parse rendered workflow branches from src/App.tsx.");
  }
  return workflows;
}

function parseNativeSmokeEntrypoints(source) {
  const pattern =
    /^[ \t]*run_smoke[ \t]+"[^"\r\n]+"[ \t]*\\[ \t]*\r?\n[ \t]*"\$ROOT_DIR\/([^"\r\n]+)"[ \t]*$/gmu;
  return [...source.matchAll(pattern)].map((entry) => entry[1]);
}

function parseAndroidScheduledCases(source) {
  const pattern =
    /^[ \t]*(?:(?:if|elif)[ \t]+![ \t]+)?(?:record_case|run_case)[ \t]+([A-Za-z_][A-Za-z0-9_]*)(?:[ \t]+([A-Za-z_][A-Za-z0-9_]*))?/gmu;
  return [...source.matchAll(pattern)].map((entry) => {
    const method = entry[1];
    const className = entry[2] ?? "AndroidSmokeTest";
    return `${className}#${method}`;
  });
}

function parseStringArray(source, name) {
  const pattern = new RegExp(`(?:export\\s+)?const\\s+${name}\\s*=\\s*\\[([\\s\\S]*?)\\]\\s+as\\s+const`, "u");
  const match = source.match(pattern);
  if (!match) {
    fail(`Could not parse ${name} from its authoritative source.`);
    return [];
  }
  return [...match[1].matchAll(/"([^"]+)"/gu)].map((entry) => entry[1]);
}

function parseNavigationItems(source, name, idKey) {
  const pattern = new RegExp(`const\\s+${name}\\b[\\s\\S]*?=\\s*\\[([\\s\\S]*?)\\];`, "u");
  const match = source.match(pattern);
  if (!match) {
    fail(`Could not parse ${name} from its authoritative source.`);
    return [];
  }
  const itemPattern = new RegExp(
    `\\{\\s*${idKey}:\\s*"([^"]+)"[\\s\\S]*?label:\\s*"([^"]+)"[\\s\\S]*?\\}`,
    "gu",
  );
  return [...match[1].matchAll(itemPattern)].map((entry) => ({
    id: entry[1],
    label: entry[2],
  }));
}

function compareOrderedValues(label, expected, actual) {
  if (JSON.stringify(expected) !== JSON.stringify(actual)) {
    fail(`${label} drifted: expected ${JSON.stringify(expected)}, found ${JSON.stringify(actual)}.`);
  }
}

function compareOrderedRecords(label, expected, actual) {
  if (JSON.stringify(expected) !== JSON.stringify(actual)) {
    fail(`${label} drifted: expected ${JSON.stringify(expected)}, found ${JSON.stringify(actual)}.`);
  }
}

function compareSets(label, expected, actual) {
  const expectedSet = new Set(expected);
  const actualSet = new Set(actual);
  const missing = expected.filter((entry) => !actualSet.has(entry));
  const extra = actual.filter((entry) => !expectedSet.has(entry));
  if (missing.length > 0) fail(`${label} is missing: ${missing.join(", ")}.`);
  if (extra.length > 0) fail(`${label} has undeclared entries: ${extra.join(", ")}.`);
}

function reportDuplicates(label, values) {
  const seen = new Set();
  for (const value of values) {
    if (seen.has(value)) fail(`Duplicate ${label}: ${value}.`);
    seen.add(value);
  }
}

async function readRequired(path, label) {
  try {
    return await readFile(join(root, path), "utf8");
  } catch (error) {
    fail(`Could not read ${label} ${path}: ${error.message}`);
    return null;
  }
}

function parseRoot(argumentsList) {
  let selected = null;
  for (let index = 0; index < argumentsList.length; index += 1) {
    const argument = argumentsList[index];
    if (argument !== "--root") throw new Error(`Unknown argument: ${argument}`);
    selected = argumentsList[index + 1];
    if (!selected) throw new Error("--root requires a directory");
    index += 1;
  }
  return selected;
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function isProtectedWorkflowReference(value) {
  return (
    isRecord(value) &&
    Object.keys(value).length === 1 &&
    typeof value.protectedWorkflow === "string"
  );
}

function isCodeIdentifier(value) {
  return typeof value === "string" && /^[A-Za-z_][A-Za-z0-9_]*$/u.test(value);
}

function isIdentifier(value, allowSlash = false) {
  const pattern = allowSlash ? /^[a-z][a-z0-9-]*(?:\/[a-z][a-z0-9-]*)?$/u : /^[a-z][a-z0-9-]*$/u;
  return typeof value === "string" && pattern.test(value);
}

function isRelativePath(value) {
  return (
    typeof value === "string" &&
    value !== "" &&
    !value.startsWith("/") &&
    !value.split("/").includes("..")
  );
}

function isSmokePath(runner, path) {
  if (typeof path !== "string") return false;
  if (runner === "native-webdriver") return path.startsWith("tests/webdriver/");
  if (runner === "android-emulator") return path.includes("/src/androidTest/");
  return false;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await collectFiles(path)));
    else if (entry.isFile()) files.push(path);
  }
  return files;
}

async function exists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

function fail(message) {
  failures.push(message);
}

function finish() {
  if (failures.length > 0) {
    console.error("Route/smoke contract verification failed:");
    for (const failure of failures) console.error(`  - ${failure}`);
    process.exit(1);
  }

  const smokeEntries = [...contract.primaryDestinations, ...contract.flashcardsWorkspaces].map(
    (entry) =>
      isProtectedWorkflowReference(entry.smoke)
        ? protectedWorkflowById.get(entry.smoke.protectedWorkflow)
        : entry,
  );
  const routeEntryCount = smokeEntries.filter(
    (entry) => entry.smoke.coverage === "route-entry",
  ).length;
  const presenceCount = smokeEntries.length - routeEntryCount;
  console.log(
    `OK   Expected route/smoke inventory matches ${contract.applicationSections.length} application sections, ` +
      `${contract.applicationWorkflows.length} application workflow, ` +
      `${contract.checkpointDestinations.length} checkpoint destinations, ` +
      `${contract.primaryDestinations.length} primary destinations, and ` +
      `${contract.flashcardsWorkspaces.length} Flashcards workspaces ` +
      `(${routeEntryCount} route-entry; ${presenceCount} presence-only), with ` +
      `${contract.protectedWorkflows.length} protected workflow and ` +
      `${contract.compactNavigation.primary.length} compact primary / ` +
      `${contract.compactNavigation.more.length} More destinations.`,
  );
}
