#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";

const path = "src/screens/flashcards/DeckLibraryWorkspace.tsx";
let source = await readFile(path, "utf8");

if (!source.includes("!settings.confirmDeletions ||")) {
  const functionStart = source.indexOf("  async function removeFolder(");
  const functionEnd = source.indexOf("\n  async function moveDeck(", functionStart);
  if (functionStart < 0 || functionEnd < 0) {
    throw new Error("Could not locate the folder deletion function.");
  }

  const functionSource = source.slice(functionStart, functionEnd);
  const dialogStart = functionSource.indexOf("    const accepted = await confirm({");
  const dialogEnd = functionSource.indexOf("    if (!accepted) return;", dialogStart);
  if (dialogStart < 0 || dialogEnd < 0) {
    throw new Error("Could not locate the folder deletion confirmation block.");
  }

  const originalBlock = functionSource.slice(dialogStart, dialogEnd);
  const confirmationCall = originalBlock
    .replace("    const accepted = await confirm({", "      (await confirm({")
    .replace(/\n    \}\);\s*$/u, "\n      }));\n");
  const replacement = `    const accepted =\n      !settings.confirmDeletions ||\n${confirmationCall}`;
  const repairedFunction = `${functionSource.slice(0, dialogStart)}${replacement}${functionSource.slice(dialogEnd)}`;
  source = `${source.slice(0, functionStart)}${repairedFunction}${source.slice(functionEnd)}`;
}

await writeFile(path, source, "utf8");
