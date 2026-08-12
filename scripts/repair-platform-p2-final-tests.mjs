#!/usr/bin/env node

import { rm } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const repairPath = join(root, "scripts", "repair-platform-p2-green-tests.mjs");

await import(new URL("./repair-platform-p2-green-tests.mjs", import.meta.url));
await rm(repairPath, { force: true });

console.log("Executed deterministic final P0-P2 test repairs.");
