#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// Normalize only the already-retired cleanup assumption. All source-contract
// replacements in the underlying applicator remain strict and fail closed.
const root = fileURLToPath(new URL("..", import.meta.url));
const applicatorPath = join(root, "scripts", "apply-platform-p2-atomic-native.mjs");
let source = await readFile(applicatorPath, "utf8");

const strictImportCleanup = `drafts = replaceExact(
  drafts,
  "use std::{collections::BTreeSet, path::PathBuf};",
  "use std::collections::BTreeSet;",
  "remove unused editor draft path import",
);`;
const tolerantImportCleanup = `drafts = drafts.replace(
  "use std::{collections::BTreeSet, path::PathBuf};",
  "use std::collections::BTreeSet;",
);`;
if (!source.includes(strictImportCleanup)) {
  throw new Error("The atomic applicator import-normalization block was not found.");
}
source = source.replace(strictImportCleanup, tolerantImportCleanup);
await writeFile(applicatorPath, source, "utf8");

await import(new URL("./apply-platform-p2-atomic-native.mjs", import.meta.url));

const profilesPath = join(root, "src-tauri", "src", "ai", "profiles.rs");
let profiles = await readFile(profilesPath, "utf8");
const profileFixture = `        request_timeout_ms: super::catalog::DEFAULT_TIMEOUT_MS,
    }
}`;
const migratedProfileFixture = `        request_timeout_ms: super::catalog::DEFAULT_TIMEOUT_MS,
        editor_draft: None,
    }
}`;
if (!profiles.includes(profileFixture)) {
  throw new Error("The AI profile test fixture was not found.");
}
profiles = profiles.replace(profileFixture, migratedProfileFixture);
await writeFile(profilesPath, profiles, "utf8");

const authoringTestsPath = join(root, "src-tauri", "src", "authoring", "tests.rs");
let authoringTests = await readFile(authoringTestsPath, "utf8");
const bulkFixture = `        ],
    };
    let prepared = input`;
const migratedBulkFixture = `        ],
        editor_draft: None,
    };
    let prepared = input`;
if (!authoringTests.includes(bulkFixture)) {
  throw new Error("The bulk-entry test fixture was not found.");
}
authoringTests = authoringTests.replace(bulkFixture, migratedBulkFixture);
await writeFile(authoringTestsPath, authoringTests, "utf8");
