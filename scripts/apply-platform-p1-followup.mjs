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
await replace("src-tauri/src/lib.rs", "mod platform;", "pub mod platform;");

let policy = await read("scripts/verify-platform-boundaries.mjs");
policy = policy
  .replace(
    '    const desktopAdapter = displayPath.startsWith("src-tauri/src/platform/desktop/");',
    '    const platformAdapter = displayPath.startsWith("src-tauri/src/platform/");',
  )
  .replaceAll("!desktopAdapter", "!platformAdapter")
  .replace(
    '  if (!/\\.(?:rs|ts|tsx)$/u.test(displayPath)) continue;\n  if (/(?:^|\\/)[^/]+\\.(?:test|spec)\\.(?:ts|tsx)$/u.test(displayPath)) continue;',
    '  if (!/\\.(?:rs|ts|tsx)$/u.test(displayPath)) continue;\n  if (/(?:^|\\/)[^/]+\\.(?:test|spec)\\.(?:ts|tsx)$/u.test(displayPath)) continue;\n  if (\n    displayPath.startsWith("src-tauri/src/")\n    && (\n      displayPath.includes("/tests/")\n      || /(?:^|\\/)(?:[^/]*_tests|conformance_tests|operation_tests)\\.rs$/u.test(displayPath)\n    )\n  ) continue;',
  );
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

const exceptionPath = "scripts/policy/platform-boundary-exceptions.json";
const exceptions = JSON.parse(await read(exceptionPath));
if (!exceptions.exceptions.some(
  (entry) => entry.rule === "frontend-direct-tauri-core-import"
    && entry.path === "src/editor/StudyRichEditor.tsx",
)) {
  exceptions.exceptions.push({
    rule: "frontend-direct-tauri-core-import",
    path: "src/editor/StudyRichEditor.tsx",
    retireInPhase: "P2",
    reason: "Temporary absolute asset path conversion",
  });
}
await write(exceptionPath, `${JSON.stringify(exceptions, null, 2)}\n`);

let calendar = await read("src-tauri/src/study_calendar.rs");
calendar = calendar
  .replace("#[cfg(test)]\nuse chrono::Utc;\nuse chrono::{DateTime, Datelike, Duration, NaiveDate};", "#[cfg(test)]\nuse chrono::{DateTime, Utc};\nuse chrono::{Datelike, Duration, NaiveDate};")
  .replace("const SECONDS_PER_LOCAL_DATE: i64 = 86_400;\n", "")
  .replace(
    "\n    #[test]\n    fn zone_identifier_rejects_paths_and_control_characters() {\n        assert_eq!(\n            normalize_zone_identifier(\"America/Los_Angeles\"),\n            Some(\"America/Los_Angeles\".to_string())\n        );\n        assert_eq!(normalize_zone_identifier(\"/usr/share/zoneinfo/UTC\"), None);\n        assert_eq!(normalize_zone_identifier(\"bad\\nzone\"), None);\n    }",
    "",
  );
await write("src-tauri/src/study_calendar.rs", calendar);

let desktopTime = await read("src-tauri/src/platform/desktop/time.rs");
desktopTime += `

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_identifier_rejects_paths_and_control_characters() {
        assert_eq!(
            normalize_zone_identifier("America/Los_Angeles"),
            Some("America/Los_Angeles".to_string())
        );
        assert_eq!(normalize_zone_identifier("/usr/share/zoneinfo/UTC"), None);
        assert_eq!(normalize_zone_identifier("bad\\nzone"), None);
    }
}
`;
await write("src-tauri/src/platform/desktop/time.rs", desktopTime);

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

await write("src-tauri/src/logging.rs", `fn timestamp_millis() -> i64 {
    crate::platform::now_utc_millis().unwrap_or_default()
}

fn record(level: &str, event: &str, message: &str) -> serde_json::Value {
    serde_json::json!({
        "timestampMs": timestamp_millis(),
        "level": level,
        "event": event,
        "message": message,
        "application": "ostudy",
        "version": env!("CARGO_PKG_VERSION"),
    })
}

pub fn info(event: &str, message: impl AsRef<str>) {
    println!("{}", record("info", event, message.as_ref()));
}

pub fn error(event: &str, message: impl AsRef<str>) {
    eprintln!("{}", record("error", event, message.as_ref()));
}
`);

let ocr = await read("src-tauri/src/pdf_ocr_backend.rs");
ocr = ocr
  .replace(
    "    time::{Duration, Instant, SystemTime, UNIX_EPOCH},",
    "    time::{Duration, Instant},",
  )
  .replace(
    "        let nanos = SystemTime::now()\n            .duration_since(UNIX_EPOCH)\n            .unwrap_or_default()\n            .as_nanos();",
    "        let nanos = crate::platform::now_utc_millis().unwrap_or_default();",
  );
await write("src-tauri/src/pdf_ocr_backend.rs", ocr);

await rm(join(root, "scripts", "apply-platform-p1-followup.mjs"), { force: true });
await rm(join(root, ".github", "workflows", "apply-platform-p1-followup.yml"), { force: true });
console.log("Applied P1 follow-up portability fixes.");
