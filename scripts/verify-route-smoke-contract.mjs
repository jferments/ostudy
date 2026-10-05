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

const navigation = contract.navigation;
const sectionIds = contract.applicationSections.map((entry) => entry.id);
const workflowIds = contract.applicationWorkflows.map((entry) => entry.id);

if (sources.applicationSections !== null) {
  compareOrderedValues(
    "application sections",
    sectionIds,
    parseUnion(sources.applicationSections, "AppSection"),
  );
}
if (sources.applicationWorkflows !== null) {
  compareOrderedValues(
    "application workflows",
    workflowIds,
    parseUnion(sources.applicationWorkflows, "AppWorkflow"),
  );
}
if (sources.checkpointGateway !== null) {
  compareOrderedValues(
    "checkpoint sections",
    contract.checkpointSections,
    parseUnion(sources.checkpointGateway, "CheckpointSection"),
  );
}
if (sources.applicationShell !== null) {
  compareOrderedValues(
    "rendered App shell sections",
    sectionIds,
    parseRenderedAppSections(sources.applicationShell),
  );
  compareOrderedValues(
    "rendered App shell workflows",
    workflowIds,
    parseRenderedAppWorkflows(sources.applicationShell),
  );
  compareOrderedRecords(
    "sections opened by navigation destinations",
    expectedDestinationSections(),
    parseDestinationSections(sources.applicationShell),
  );
  validateHeaderActionWiring(sources.applicationShell);
}
if (sources.mainNavigation !== null) {
  compareOrderedRecords(
    "main navigation destinations",
    navigation.destinations.map(({ id, label }) => ({ id, label })),
    parseNavigationItems(sources.mainNavigation, "ITEMS", "destination"),
  );
}
if (sources.destinationMapping !== null) {
  compareOrderedValues(
    "navigation destination type",
    navigation.destinations.map((entry) => entry.id),
    parseUnion(sources.destinationMapping, "AppDestination"),
  );
  const librarySections = parseStringArray(sources.destinationMapping, "LIBRARY_SECTIONS");
  compareOrderedValues(
    "Library views",
    navigation.library.views.map((entry) => entry.id),
    librarySections,
  );
  validateHighlightedDestinations(sources.destinationMapping, librarySections);
}
if (sources.libraryViews !== null) {
  compareOrderedRecords(
    "Library view labels",
    navigation.library.views.map(({ id, label }) => ({ id, label })),
    parseLibraryViewLabels(sources.libraryViews),
  );
}
if (sources.headerActions !== null) {
  compareOrderedRecords(
    "header actions",
    navigation.headerActions.map(({ id, label }) => ({ id, label })),
    parseHeaderActions(sources.headerActions),
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

await validateSmokeEvidence(contract.applicationSections, "section", smokeExecution);
await validateSmokeEvidence(contract.flashcardsWorkspaces, "Flashcards workspace", smokeExecution);
await validateSmokeEvidence(contract.protectedWorkflows, "protected workflow", smokeExecution);
await validateProductIndependence();

finish();

function validateContractShape(value) {
  if (!isRecord(value)) {
    fail("The route/smoke contract must be a JSON object.");
    return;
  }
  if (value.schemaVersion !== 2) {
    fail(`schemaVersion must be 2; found ${JSON.stringify(value.schemaVersion)}.`);
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
    "mainNavigation",
    "destinationMapping",
    "libraryViews",
    "headerActions",
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
    !Array.isArray(value.checkpointSections) ||
    value.checkpointSections.some((section) => !isIdentifier(section))
  ) {
    fail("checkpointSections must be an array of section identifiers.");
  }
  if (!Array.isArray(value.protectedWorkflows)) {
    fail("protectedWorkflows must be an array.");
  }
  if (!Array.isArray(value.flashcardsWorkspaces)) {
    fail("flashcardsWorkspaces must be an array.");
  }
  validateNavigationShape(value.navigation);
  if (failures.length > 0) return;

  const destinations = new Map(value.navigation.destinations.map((entry) => [entry.id, entry]));
  const views = new Map(value.navigation.library.views.map((entry) => [entry.id, entry]));
  const actions = new Map(value.navigation.headerActions.map((entry) => [entry.id, entry]));

  validateApplicationSections(value.applicationSections, value.navigation, destinations, views, actions);
  validateApplicationWorkflows(value.applicationWorkflows, destinations);
  validateProtectedWorkflowCatalog(value.protectedWorkflows);
  validateCatalog(value.flashcardsWorkspaces, "Flashcards workspace");
  if (failures.length > 0) return;

  const sectionIdList = value.applicationSections.map((entry) => entry.id);
  reportDuplicates("application section", sectionIdList);
  reportDuplicates("application workflow", value.applicationWorkflows.map((entry) => entry.id));
  reportDuplicates("Flashcards workspace", value.flashcardsWorkspaces.map((entry) => entry.id));
  reportDuplicates("protected workflow", value.protectedWorkflows.map((entry) => entry.id));
  reportDuplicates("checkpoint section", value.checkpointSections);

  // Every section can be saved and restored by the navigation checkpoint.
  compareOrderedValues("checkpoint section taxonomy", sectionIdList, value.checkpointSections);

  // Every destination opens something, and each Library view is one section.
  for (const destination of value.navigation.destinations) {
    const placed = value.applicationSections.filter(
      (entry) => entry.placement.destination === destination.id,
    );
    if (placed.length === 0) {
      fail(`navigation destination ${destination.id} opens no section.`);
    }
  }
  compareSets(
    "Library view coverage",
    value.navigation.library.views.map((entry) => entry.id),
    value.applicationSections
      .filter((entry) => typeof entry.placement.libraryView === "string")
      .map((entry) => entry.placement.libraryView),
  );

  validateProtectedWorkflowCoverage(value.applicationWorkflows, value.protectedWorkflows);
}

function validateNavigationShape(value) {
  if (!isRecord(value)) {
    fail("navigation must be an object.");
    return;
  }
  validateLabeledIdentifiers(value.destinations, "navigation.destinations");
  validateLabeledIdentifiers(value.headerActions, "navigation.headerActions");
  if (!isRecord(value.library)) {
    fail("navigation.library must be an object.");
    return;
  }
  validateLabeledIdentifiers(value.library.views, "navigation.library.views");
  if (failures.length > 0) return;
  if (!value.destinations.some((entry) => entry.id === value.library.destination)) {
    fail(
      `navigation.library.destination ${JSON.stringify(value.library.destination)} is not a navigation destination.`,
    );
  }
  reportDuplicates("navigation destination", value.destinations.map((entry) => entry.id));
  reportDuplicates("header action", value.headerActions.map((entry) => entry.id));
  reportDuplicates("Library view", value.library.views.map((entry) => entry.id));
}

function validateLabeledIdentifiers(entries, label) {
  if (!Array.isArray(entries) || entries.length === 0) {
    fail(`${label} must be a non-empty array.`);
    return;
  }
  for (const [index, entry] of entries.entries()) {
    if (!isRecord(entry) || !isIdentifier(entry.id)) {
      fail(`${label}[${index}].id must be a non-empty identifier.`);
      continue;
    }
    if (typeof entry.label !== "string" || entry.label.trim() === "") {
      fail(`${label}[${index}] must have a non-empty label.`);
    }
  }
}

function validateApplicationSections(entries, navigationShape, destinations, views, actions) {
  for (const [index, entry] of entries.entries()) {
    if (!isRecord(entry) || !isIdentifier(entry.id)) {
      fail(`applicationSections[${index}].id must be a non-empty route identifier.`);
      continue;
    }
    const label = `section ${entry.id}`;
    if (typeof entry.label !== "string" || entry.label.trim() === "") {
      fail(`${label} must have a non-empty label.`);
    }
    validateSmokeShape(entry.smoke, label);

    const placement = entry.placement;
    const hasDestination = isRecord(placement) && "destination" in placement;
    const hasAction = isRecord(placement) && "headerAction" in placement;
    if (hasDestination === hasAction) {
      fail(`${label} must name the destination or header action that opens it.`);
      continue;
    }
    if (hasAction) {
      const action = actions.get(placement.headerAction);
      if (!action) {
        fail(`${label} names unknown header action ${JSON.stringify(placement.headerAction)}.`);
      } else if (action.label !== entry.label) {
        fail(`${label} is labeled ${JSON.stringify(entry.label)} but its header action says ${JSON.stringify(action.label)}.`);
      }
      if ("libraryView" in placement) {
        fail(`${label} opens from the header and cannot also be a Library view.`);
      }
      continue;
    }

    const destination = destinations.get(placement.destination);
    if (!destination) {
      fail(`${label} names unknown navigation destination ${JSON.stringify(placement.destination)}.`);
      continue;
    }
    if (placement.destination === navigationShape.library.destination) {
      const view = views.get(placement.libraryView);
      if (!view) {
        fail(`${label} opens in Library and must name one of its views.`);
      } else if (placement.libraryView !== entry.id) {
        // Library views keep their section ids so saved checkpoints still name them.
        fail(`${label} must be the Library view with its own id, not ${placement.libraryView}.`);
      } else if (view.label !== entry.label) {
        fail(`${label} is labeled ${JSON.stringify(entry.label)} but its Library view says ${JSON.stringify(view.label)}.`);
      }
    } else if ("libraryView" in placement) {
      fail(`${label} opens from ${placement.destination}, which has no Library views.`);
    } else if (destination.label !== entry.label) {
      fail(`${label} is labeled ${JSON.stringify(entry.label)} but its destination says ${JSON.stringify(destination.label)}.`);
    }
  }
}

function validateApplicationWorkflows(entries, destinations) {
  for (const [index, entry] of entries.entries()) {
    const label = `applicationWorkflows[${index}]`;
    if (!isRecord(entry) || !isIdentifier(entry.id)) {
      fail(`${label}.id must be a non-empty workflow identifier.`);
      continue;
    }
    if (!isIdentifier(entry.protectedWorkflow)) {
      fail(`${label}.protectedWorkflow must name its protected smoke workflow.`);
    }
    if (!destinations.has(entry.destination)) {
      fail(`${label}.destination must name the navigation destination it runs inside.`);
    }
  }
}

function validateProtectedWorkflowCatalog(entries) {
  for (const [index, entry] of entries.entries()) {
    if (!isRecord(entry) || !isIdentifier(entry.id)) {
      fail(`protectedWorkflows[${index}].id must be a non-empty workflow identifier.`);
      continue;
    }
    const label = `protected workflow ${entry.id}`;
    if (!isIdentifier(entry.workflow)) {
      fail(`${label}.workflow must be a non-empty application workflow identifier.`);
    }
    if (typeof entry.label !== "string" || entry.label.trim() === "") {
      fail(`${label} must have a non-empty label.`);
    }
    validateSmokeShape(entry.smoke, label);
  }
}

function validateCatalog(entries, catalog) {
  for (const [index, entry] of entries.entries()) {
    if (!isRecord(entry) || !isIdentifier(entry.id)) {
      fail(`${catalog}[${index}].id must be a non-empty route identifier.`);
      continue;
    }
    const label = `${catalog} ${entry.id}`;
    if (typeof entry.label !== "string" || entry.label.trim() === "") {
      fail(`${label} must have a non-empty label.`);
    }
    validateSmokeShape(entry.smoke, label);
  }
}

function validateProtectedWorkflowCoverage(applicationWorkflows, protectedWorkflows) {
  const protectedById = new Map(protectedWorkflows.map((workflow) => [workflow.id, workflow]));
  const applicationById = new Map(applicationWorkflows.map((workflow) => [workflow.id, workflow]));

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
    if (!isRelativePath(evidence.entrypoint) || !isSmokePath("native-webdriver", evidence.entrypoint)) {
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

/**
 * Each navigation destination opens the one section it hosts. A destination
 * hosting several sections, Library, reopens the last one the learner used.
 */
function expectedDestinationSections() {
  return navigation.destinations.map((destination) => {
    const placed = contract.applicationSections.filter(
      (entry) => entry.placement.destination === destination.id,
    );
    return { id: destination.id, section: placed.length === 1 ? placed[0].id : null };
  });
}

function validateHeaderActionWiring(source) {
  for (const entry of contract.applicationSections) {
    const action = entry.placement.headerAction;
    if (typeof action !== "string") continue;
    const handler = `on${action[0].toUpperCase()}${action.slice(1)}`;
    const pattern = new RegExp(
      `\\b${escapeRegExp(handler)}=\\{\\(\\)\\s*=>\\s*navigate\\("${escapeRegExp(entry.id)}"\\)\\}`,
      "u",
    );
    if (!pattern.test(source)) {
      fail(`header action ${action} does not open section ${entry.id} in src/App.tsx.`);
    }
  }
}

function validateHighlightedDestinations(source, librarySections) {
  const body = source.match(
    /export\s+function\s+destinationOf\s*\([\s\S]*?\)\s*:[^{]*\{([\s\S]*?)\n\}/u,
  )?.[1];
  if (body === undefined) {
    fail("Could not parse destinationOf from its authoritative source.");
    return;
  }
  const highlightedWorkflows = new Map(
    [...body.matchAll(/if\s*\(\s*activeWorkflow\s*===\s*"([^"]+)"\s*\)\s*return\s+"([^"]+)"\s*;/gu)].map(
      (match) => [match[1], match[2]],
    ),
  );
  const highlightedSections = new Map();
  const library = body.match(/if\s*\(\s*isLibrarySection\(section\)\s*\)\s*return\s+"([^"]+)"\s*;/u);
  if (library) {
    for (const section of librarySections) highlightedSections.set(section, library[1]);
  }
  for (const match of body.matchAll(/case\s+"([^"]+)"\s*:\s*return\s+"([^"]+)"\s*;/gu)) {
    highlightedSections.set(match[1], match[2]);
  }
  const fallback = /default\s*:\s*return\s+null\s*;/u.test(body) ? null : undefined;

  for (const entry of contract.applicationSections) {
    const expected = entry.placement.destination ?? null;
    const found = highlightedSections.has(entry.id) ? highlightedSections.get(entry.id) : fallback;
    if (found === undefined) {
      fail(`destinationOf does not say which destination section ${entry.id} highlights.`);
    } else if (found !== expected) {
      fail(
        `highlighted destination for section ${entry.id} drifted: expected ${JSON.stringify(expected)}, found ${JSON.stringify(found)}.`,
      );
    }
  }
  for (const entry of contract.applicationWorkflows) {
    const found = highlightedWorkflows.get(entry.id);
    if (found !== entry.destination) {
      fail(
        `highlighted destination for workflow ${entry.id} drifted: expected ${JSON.stringify(entry.destination)}, found ${JSON.stringify(found ?? null)}.`,
      );
    }
  }
}

async function validateSmokeEvidence(entries, catalog, execution) {
  const cache = new Map();
  for (const entry of entries) {
    const label = `${catalog} ${entry.id}`;
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
      fail(`${label} instrumentation case ${caseKey} is not scheduled by scripts/android-emulator-smoke.sh.`);
    }
    const methodPattern = new RegExp(`\\bfun\\s+${escapeRegExp(instrumentationCase.method)}\\s*\\(`, "u");
    if (!methodPattern.test(evidenceSource)) {
      fail(`${label} evidence ${evidence.path} does not define instrumentation method ${instrumentationCase.method}.`);
    }
  }
}

async function validateProductIndependence() {
  const productRoots = ["src", "src-tauri/src", "src-tauri/gen/android/app/src/main"];
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

function parseUnion(source, name) {
  const match = source.match(new RegExp(`export\\s+type\\s+${name}\\s*=([\\s\\S]*?);`, "u"));
  if (!match) {
    fail(`Could not parse export type ${name} from its authoritative source.`);
    return [];
  }
  return [...match[1].matchAll(/"([^"]+)"/gu)].map((entry) => entry[1]);
}

function parseRenderedAppSections(source) {
  const sections = [
    ...source.matchAll(/(?:\{|:\s*)controller\.section\s*===\s*"([^"]+)"\s*\?\s*\(/gu),
  ].map((entry) => entry[1]);
  if (sections.length === 0) {
    fail("Could not parse rendered section branches from src/App.tsx.");
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

function parseDestinationSections(source) {
  const match = source.match(/const\s+DESTINATION_SECTIONS\b[^=]*=\s*\{([\s\S]*?)\};/u);
  if (!match) {
    fail("Could not parse DESTINATION_SECTIONS from src/App.tsx.");
    return [];
  }
  return [...match[1].matchAll(/^\s*"?([a-z][a-z0-9-]*)"?\s*:\s*(?:null|"([^"]+)")\s*,?\s*$/gmu)].map(
    (entry) => ({ id: entry[1], section: entry[2] ?? null }),
  );
}

function parseLibraryViewLabels(source) {
  const match = source.match(/const\s+VIEWS\b[^=]*=\s*\{([\s\S]*?)\n\};/u);
  if (!match) {
    fail("Could not parse VIEWS from the Library frame.");
    return [];
  }
  return [...match[1].matchAll(/^\s*"?([a-z][a-z0-9-]*)"?\s*:\s*\{\s*label:\s*"([^"]+)"/gmu)].map(
    (entry) => ({ id: entry[1], label: entry[2] }),
  );
}

function parseHeaderActions(source) {
  const actions = [];
  for (const element of source.matchAll(/<IconButton\b([\s\S]*?)\/>/gu)) {
    const attributes = element[1];
    const label = attributes.match(/(?<![-\w])label="([^"]+)"/u)?.[1];
    const id = attributes.match(/data-testid="app-([^"]+)"/u)?.[1];
    if (label === undefined || id === undefined) {
      fail("Every header IconButton needs a label and an app-* test ID.");
      continue;
    }
    actions.push({ id, label });
  }
  if (actions.length === 0) {
    fail("Could not parse header actions from AppHeaderBar.");
  }
  return actions;
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

function isCodeIdentifier(value) {
  return typeof value === "string" && /^[A-Za-z_][A-Za-z0-9_]*$/u.test(value);
}

function isIdentifier(value) {
  return typeof value === "string" && /^[a-z][a-z0-9-]*$/u.test(value);
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

  const smokeEntries = [...contract.applicationSections, ...contract.flashcardsWorkspaces];
  const routeEntryCount = smokeEntries.filter((entry) => entry.smoke.coverage === "route-entry").length;
  const presenceCount = smokeEntries.length - routeEntryCount;
  console.log(
    `OK   Expected route/smoke inventory matches ${contract.applicationSections.length} application sections, ` +
      `${contract.applicationWorkflows.length} application workflow, ` +
      `${contract.checkpointSections.length} checkpoint sections, ` +
      `${contract.navigation.destinations.length} navigation destinations, ` +
      `${contract.navigation.library.views.length} Library views, ` +
      `${contract.navigation.headerActions.length} header actions, and ` +
      `${contract.flashcardsWorkspaces.length} Flashcards workspaces ` +
      `(${routeEntryCount} route-entry; ${presenceCount} presence-only), with ` +
      `${contract.protectedWorkflows.length} protected workflow.`,
  );
}
