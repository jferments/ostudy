#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const manifest = await read("plugins/ostudy-platform/android/src/main/AndroidManifest.xml");
const accessoryFilter = await read(
  "plugins/ostudy-platform/android/src/main/res/xml/ostudy_usb_accessory_filter.xml",
);

const failures = [];
const productionManifest = stripXmlComments(manifest);
const productionFilter = stripXmlComments(accessoryFilter);

expectContains(
  productionManifest,
  '<activity android:name="io.github.jferments.ostudy.MainActivity">',
  "oStudy MainActivity USB-accessory overlay",
);
expectContains(
  productionManifest,
  '<action android:name="android.hardware.usb.action.USB_ACCESSORY_ATTACHED" />',
  "Android Open Accessory attachment intent filter",
);
expectContains(
  productionManifest,
  'android:name="android.hardware.usb.action.USB_ACCESSORY_ATTACHED"',
  "Android Open Accessory metadata key",
);
expectContains(
  productionManifest,
  'android:resource="@xml/ostudy_usb_accessory_filter"',
  "Android Open Accessory identity-filter metadata",
);

const attachmentMentions =
  productionManifest.match(/android\.hardware\.usb\.action\.USB_ACCESSORY_ATTACHED/gu)?.length ?? 0;
if (attachmentMentions < 2) {
  failures.push(
    "The production manifest must declare USB_ACCESSORY_ATTACHED in both the intent filter and metadata; comments do not count.",
  );
}

expectContains(
  productionFilter,
  'manufacturer="oStudy"',
  "oStudy AOA manufacturer identity",
);
expectContains(
  productionFilter,
  'model="oStudy USB Sync"',
  "oStudy AOA model identity",
);

if (failures.length > 0) {
  console.error("Android USB accessory-manifest verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "OK   Android declares a real (non-comment) AOA accessory handler and exact oStudy accessory identity.",
);

async function read(relativePath) {
  return readFile(new URL(relativePath, `file://${root}/`), "utf8");
}

function stripXmlComments(source) {
  return source.replace(/<!--[\s\S]*?-->/gu, "");
}

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}
