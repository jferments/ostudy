#!/usr/bin/env node

import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { localFontSources } from "./font-sources.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const outputDirectory = join(root, "src", "assets", "fonts");
const maximumFontBytes = 5 * 1024 * 1024;
const requestTimeoutMillis = 30_000;
const browserUserAgent =
  "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 " +
  "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

await mkdir(outputDirectory, { recursive: true });

let installed = 0;
let reused = 0;
for (const font of localFontSources) {
  const destination = join(outputDirectory, font.filename);
  if (await isValidWoff2(destination)) {
    reused += 1;
    continue;
  }

  const family = font.family.replaceAll(" ", "+");
  const stylesheetUrl =
    `https://fonts.googleapis.com/css2?family=${family}:wght@${font.weightRange}&display=swap`;
  const stylesheet = await fetchText(stylesheetUrl, "Google Fonts stylesheet");
  const assetUrl = extractLatinWoff2(stylesheet, font.family);
  const parsed = new URL(assetUrl);
  if (parsed.protocol !== "https:" || parsed.hostname !== "fonts.gstatic.com") {
    throw new Error(
      `Google Fonts returned an unexpected asset host for ${font.family}: ${parsed.hostname}`,
    );
  }

  const response = await fetch(assetUrl, {
    headers: { "User-Agent": browserUserAgent },
    signal: AbortSignal.timeout(requestTimeoutMillis),
  });
  if (!response.ok) {
    throw new Error(
      `Google Fonts asset download failed for ${font.family}: HTTP ${response.status}.`,
    );
  }
  const declaredLength = Number(response.headers.get("content-length") ?? 0);
  if (declaredLength > maximumFontBytes) {
    throw new Error(`${font.family} exceeds the ${maximumFontBytes}-byte font safety limit.`);
  }

  const bytes = Buffer.from(await response.arrayBuffer());
  if (bytes.length === 0 || bytes.length > maximumFontBytes || !hasWoff2Magic(bytes)) {
    throw new Error(`Google Fonts returned an invalid WOFF2 payload for ${font.family}.`);
  }

  const temporary = `${destination}.tmp-${process.pid}`;
  try {
    await writeFile(temporary, bytes, { mode: 0o644 });
    await rename(temporary, destination);
  } finally {
    await rm(temporary, { force: true });
  }
  installed += 1;
  console.log(`Installed ${font.family} from Google Fonts -> ${font.filename}`);
}

console.log(
  `OK   Local font cache is ready (${installed} downloaded, ${reused} already present).`,
);

async function fetchText(url, label) {
  const response = await fetch(url, {
    headers: {
      Accept: "text/css,*/*;q=0.1",
      "User-Agent": browserUserAgent,
    },
    signal: AbortSignal.timeout(requestTimeoutMillis),
  });
  if (!response.ok) throw new Error(`${label} request failed: HTTP ${response.status}.`);
  return response.text();
}

function extractLatinWoff2(stylesheet, family) {
  const blocks = [
    ...stylesheet.matchAll(/(?:\/\*\s*([^*]+?)\s*\*\/\s*)?@font-face\s*\{([^}]+)\}/gu),
  ];
  if (blocks.length === 0) {
    throw new Error(`Google Fonts returned no @font-face rules for ${family}.`);
  }
  const block =
    blocks.find((match) => match[1]?.trim().toLowerCase() === "latin") ??
    blocks.find((match) => /unicode-range\s*:[^;]*(?:U\+0000-00FF|U\+0020-007E)/iu.test(match[2])) ??
    blocks.at(-1);
  const url = block?.[2].match(
    /src\s*:\s*url\((?:["']?)(https:\/\/fonts\.gstatic\.com\/[^)'"\s]+)(?:["']?)\)\s*format\((?:["'])woff2(?:["'])\)/iu,
  )?.[1];
  if (!url) throw new Error(`Google Fonts returned no Latin WOFF2 asset for ${family}.`);
  return url;
}

async function isValidWoff2(path) {
  try {
    const bytes = await readFile(path);
    return bytes.length > 0 && bytes.length <= maximumFontBytes && hasWoff2Magic(bytes);
  } catch {
    return false;
  }
}

function hasWoff2Magic(bytes) {
  return bytes.length >= 4 && bytes.subarray(0, 4).toString("ascii") === "wOF2";
}
