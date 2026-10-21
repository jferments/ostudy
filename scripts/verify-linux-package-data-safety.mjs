#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");
const failures = [];

const [baseText, devText, linuxText, release, packageCheck, workflow, linuxDocs] = await Promise.all([
  read("src-tauri/tauri.conf.json"),
  read("src-tauri/tauri.dev.conf.json"),
  read("src-tauri/tauri.linux.conf.json"),
  read("scripts/release.sh"),
  read("scripts/check-linux-package.sh"),
  read(".github/workflows/check.yml"),
  read("docs/installation/linux.md"),
]);
const base = JSON.parse(baseText);
const dev = JSON.parse(devText);
const linux = JSON.parse(linuxText);

const productionId = "io.github.jferments.ostudy";
const developmentId = "io.github.jferments.ostudy.dev";

function fail(message) { failures.push(message); }
function requireText(label, source, needle) {
  if (!source.includes(needle)) fail(`${label} missing ${JSON.stringify(needle)}`);
}

if (base.identifier !== productionId) fail("production Tauri identifier changed");
if (dev.identifier !== developmentId) fail("development Tauri identifier changed");
for (const target of ["deb", "rpm"]) {
  if (!Array.isArray(linux.bundle?.targets) || !linux.bundle.targets.includes(target)) {
    fail(`Linux bundle must include ${target} packaging`);
  }
}

const packageDestinations = [
  ...Object.keys(linux.bundle?.linux?.deb?.files ?? {}),
  ...Object.keys(linux.bundle?.linux?.rpm?.files ?? {}),
  ...Object.values(linux.bundle?.resources ?? {}),
].map(String);
for (const destination of packageDestinations) {
  const normalized = destination.toLowerCase();
  for (const forbidden of ["/home/", "/root/", ".local/share", "$xdg_data_home"]) {
    if (normalized.includes(forbidden)) {
      fail(`Linux package-owned destination targets user/application data: ${destination}`);
    }
  }
}
for (const [label, format] of [["Debian", "deb"], ["RPM", "rpm"]]) {
  if (!Object.keys(linux.bundle?.linux?.[format]?.files ?? {}).includes("/usr/lib/udev/rules.d/70-ostudy-android-usb.rules")) {
    fail(`${label} package must retain the reviewed Android USB udev rule`);
  }
}

requireText("release production build", release, "pnpm tauri build --config src-tauri/tauri.linux.conf.json --bundles deb,rpm,appimage");
requireText("release actual Cargo bundle root", release, 'BUNDLE_ROOT="$CARGO_TARGET_DIR/release/bundle"');
requireText("package check production build", packageCheck, "pnpm tauri build --config src-tauri/tauri.linux.conf.json --bundles deb,rpm --ci -- --locked");
requireText("package check actual Cargo bundle root", packageCheck, 'BUNDLE_ROOT="$CARGO_TARGET_DIR/release/bundle"');
requireText("package check Debian inspection", packageCheck, '"$SCRIPT_DIR/verify-deb-package.sh" "${DEB_FILES[0]}"');
requireText("package check RPM inspection", packageCheck, '"$SCRIPT_DIR/verify-rpm-package.py" "${RPM_FILES[0]}"');
requireText("release RPM artifact inspection", release, '"$SCRIPT_DIR/verify-rpm-package.py" "${RPM_FILES[0]}"');
requireText("terminal workflow package step", workflow, "Qualify production Debian package");
requireText("terminal workflow package command", workflow, "bash ./scripts/check-linux-package.sh");
requireText("terminal workflow package evidence", workflow, 'linux-package=${LINUX_PACKAGE_STATUS:-skipped}');
requireText("release Debian artifact inspection", release, '"$SCRIPT_DIR/verify-deb-package.sh" "${DEB_FILES[0]}"');
requireText("Debian verifier dependency check", await read("scripts/verify-deb-package.sh"), "libusb-1\\.0-0");
requireText("Debian verifier udev extraction check", await read("scripts/verify-deb-package.sh"), "PACKAGED_UDEV=");
requireText("Debian verifier PDFium extraction check", await read("scripts/verify-deb-package.sh"), "PACKAGED_PDFIUM=");
requireText("RPM verifier dependency check", await read("scripts/verify-rpm-package.py"), "LIBUSB_REQUIREMENT");
requireText("RPM verifier user-data check", await read("scripts/verify-rpm-package.py"), "USER_PATH");
if (release.includes("tauri.dev.conf.json")) fail("release path must not use the development Tauri overlay");
if (packageCheck.includes("tauri.dev.conf.json")) fail("package qualification must not use the development Tauri overlay");
for (const forbidden of ["XDG_DATA_HOME", ".local/share/io.github.jferments.ostudy", "rm -rf -- $HOME", "rm -rf $HOME"]) {
  if (release.includes(forbidden)) fail(`release.sh must not manipulate learner data: ${forbidden}`);
}

requireText("Linux uninstall policy", linuxDocs, "normal package-manager removal");
requireText("Linux learner-data preservation", linuxDocs, "does not own the learner library");
requireText("Linux purge separation", linuxDocs, "explicit purge-user-data");

if (failures.length > 0) {
  console.error("Linux package data-safety verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log("OK   Linux production packaging is separated from user-owned learner data and development identity.");
