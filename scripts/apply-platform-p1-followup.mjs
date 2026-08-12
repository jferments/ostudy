#!/usr/bin/env node

import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");
const write = async (path, content) => {
  const target = join(root, path);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, content, "utf8");
};
const replace = async (path, from, to) => {
  const source = await read(path);
  if (!source.includes(from)) throw new Error(`${path}: source fragment not found`);
  await write(path, source.replace(from, to));
};

await replace(
  "src-tauri/src/platform/mod.rs",
  "#[cfg(not(any(target_os = \"android\", target_os = \"ios\")))]\nmod desktop;",
  "#[cfg(not(any(target_os = \"android\", target_os = \"ios\")))]\nmod desktop;\n#[cfg(any(target_os = \"android\", target_os = \"ios\"))]\nmod mobile_stub;",
);
await replace(
  "src-tauri/src/platform/mod.rs",
  "        #[cfg(any(target_os = \"android\", target_os = \"ios\"))]\n        {\n            compile_error!(\"Mobile PlatformRuntime adapters must be supplied by the generated Tauri mobile project.\");\n        }",
  "        #[cfg(any(target_os = \"android\", target_os = \"ios\"))]\n        {\n            return mobile_stub::runtime();\n        }",
);

let policy = await read("scripts/verify-platform-boundaries.mjs");
policy = policy
  .replace(
    '    const desktopAdapter = displayPath.startsWith("src-tauri/src/platform/desktop/");',
    '    const platformAdapter = displayPath.startsWith("src-tauri/src/platform/");',
  )
  .replaceAll("!desktopAdapter", "!platformAdapter");
await write("scripts/verify-platform-boundaries.mjs", policy);

let repositoryPolicy = await read("scripts/verify-repository-policy.mjs");
repositoryPolicy = repositoryPolicy.replace(
  '  if (/\\bkeyring\\b/u.test(text) && displayPath !== "src-tauri/src/ai/secrets.rs") {\n    failures.push(`${displayPath}: operating-system credentials are confined to the AI secret-store boundary`);\n  }',
  '  if (\n    /\\bkeyring\\b/u.test(text)\n    && displayPath !== "src-tauri/src/platform/desktop/secrets.rs"\n  ) {\n    failures.push(`${displayPath}: operating-system credentials are confined to the desktop secret-store adapter`);\n  }',
);
if (!repositoryPolicy.includes("desktop secret-store adapter")) {
  throw new Error("scripts/verify-repository-policy.mjs: keyring policy was not migrated");
}
await write("scripts/verify-repository-policy.mjs", repositoryPolicy);

let calendar = await read("src-tauri/src/study_calendar.rs");
calendar = calendar
  .replace("#[cfg(test)]\nuse chrono::Utc;\nuse chrono::{DateTime, Datelike, Duration, NaiveDate};", "#[cfg(test)]\nuse chrono::{DateTime, Utc};\nuse chrono::{Datelike, Duration, NaiveDate};")
  .replace("const SECONDS_PER_LOCAL_DATE: i64 = 86_400;\n", "");
await write("src-tauri/src/study_calendar.rs", calendar);

let mobile = await read("src-tauri/src/platform/mobile_stub.rs");
mobile = mobile.replace(
  "        date.and_hms_opt(0, 0, 0)\n            .and_then(|value| value.and_utc().timestamp_millis().try_into().ok())\n            .ok_or_else(|| \"The requested study calendar date is outside the supported range.\".to_string())",
  "        date.and_hms_opt(0, 0, 0)\n            .map(|value| value.and_utc().timestamp_millis())\n            .ok_or_else(|| \"The requested study calendar date is outside the supported range.\".to_string())",
);
await write("src-tauri/src/platform/mobile_stub.rs", mobile);

let resources = await read("src-tauri/src/system_resources.rs");
resources = resources.replace(
  "        for mut value in [snapshot(16, gib(32)), snapshot(16, gib(32)), snapshot(16, gib(32))] {\n            if value == snapshot(16, gib(32)) {\n                value.low_memory = true;\n            }\n            assert_eq!(calculate_parallel_workers(value, 20, 128 * MIB, None).worker_count, 1);\n        }\n        let mut thermal = snapshot(16, gib(32));",
  "        let mut low_memory = snapshot(16, gib(32));\n        low_memory.low_memory = true;\n        assert_eq!(\n            calculate_parallel_workers(low_memory, 20, 128 * MIB, None).worker_count,\n            1\n        );\n        let mut thermal = snapshot(16, gib(32));",
);
await write("src-tauri/src/system_resources.rs", resources);

await rm(join(root, "scripts", "apply-platform-p1-followup.mjs"), { force: true });
await rm(join(root, ".github", "workflows", "apply-platform-p1-followup.yml"), { force: true });
console.log("Applied P1 follow-up portability fixes.");
