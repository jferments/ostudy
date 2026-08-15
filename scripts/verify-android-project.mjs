#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const failures = [];
const providerPath =
  "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/OstudyTestDocumentsProvider.java";
const retiredProviderPath = providerPath.replace(/\.java$/u, ".kt");

const requiredPaths = [
  "src-tauri/tauri.android.conf.json",
  "src-tauri/gen/android/gradlew",
  "src-tauri/gen/android/gradle/wrapper/gradle-wrapper.properties",
  "src-tauri/gen/android/gradle.properties",
  "src-tauri/gen/android/app/build.gradle.kts",
  "src-tauri/gen/android/app/src/main/AndroidManifest.xml",
  "src-tauri/gen/android/app/src/main/java/io/github/jferments/ostudy/MainActivity.kt",
  "src-tauri/gen/android/app/src/main/res/xml/file_paths.xml",
  "src-tauri/gen/android/app/src/androidTest/AndroidManifest.xml",
  "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidSmokeTest.kt",
  "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidDocumentsProviderSmokeTest.kt",
  providerPath,
  "plugins/ostudy-platform/build.rs",
  "plugins/ostudy-platform/permissions/default.toml",
  "plugins/ostudy-platform/android/build.gradle.kts",
  "plugins/ostudy-platform/android/src/main/java/OstudyPlatformPlugin.kt",
  "src/platform/PlatformProvider.tsx",
  "src/platform/backInterception.ts",
  "src/testing/androidTestBridge.ts",
  "scripts/android-env.sh",
  "scripts/android-runner-doctor.sh",
  "scripts/android-build.sh",
  "scripts/android-gradle-contracts.sh",
  "scripts/android-gradle-postbuild.sh",
  "scripts/android-test.sh",
  "scripts/android-avd.sh",
  "scripts/android-emulator-smoke.sh",
  "scripts/verify-android-package.sh",
  ".github/workflows/android-check.yml",
  ".github/workflows/android-package.yml",
];
for (const path of requiredPaths) {
  if (!existsSync(resolve(root, path))) failures.push(`Missing Android project file: ${path}`);
}
if (existsSync(resolve(root, retiredProviderPath))) {
  failures.push("The retired Kotlin test DocumentsProvider still exists beside the Java provider.");
}

const env = parseEnv(await read("versions.env"));
for (const key of [
  "ANDROID_JDK_MAJOR",
  "ANDROID_MIN_SDK",
  "ANDROID_COMPILE_SDK",
  "ANDROID_TARGET_SDK",
  "ANDROID_BUILD_TOOLS_VERSION",
  "ANDROID_NDK_VERSION",
  "ANDROID_GRADLE_PLUGIN_VERSION",
  "ANDROID_GRADLE_VERSION",
  "ANDROID_KOTLIN_VERSION",
  "ANDROID_COROUTINES_VERSION",
  "ANDROID_PHONE_SYSTEM_IMAGE",
  "ANDROID_PHONE_AVD",
  "ANDROID_MIN_SYSTEM_IMAGE",
  "ANDROID_MIN_AVD",
]) {
  if (!env.get(key)) failures.push(`versions.env is missing ${key}`);
}
if (!env.get("ANDROID_MIN_SYSTEM_IMAGE")?.includes(`android-${env.get("ANDROID_MIN_SDK")};`)) {
  failures.push("The minimum-SDK emulator image must match ANDROID_MIN_SDK.");
}

let localPropertiesTracked = false;
try {
  execFileSync("git", ["ls-files", "--error-unmatch", "src-tauri/gen/android/local.properties"], {
    cwd: root,
    stdio: "ignore",
  });
  localPropertiesTracked = true;
} catch {
  localPropertiesTracked = false;
}
if (localPropertiesTracked) failures.push("Android local.properties must remain machine-local.");

const appGradle = await read("src-tauri/gen/android/app/build.gradle.kts");
expect(appGradle, `compileSdk = ${env.get("ANDROID_COMPILE_SDK")}`, "compileSdk pin");
expect(appGradle, `minSdk = ${env.get("ANDROID_MIN_SDK")}`, "minSdk pin");
expect(appGradle, `targetSdk = ${env.get("ANDROID_TARGET_SDK")}`, "targetSdk pin");
expect(appGradle, 'applicationId = "io.github.jferments.ostudy"', "application identity");
expect(
  appGradle,
  'testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"',
  "instrumentation runner",
);

const rootGradle = await read("src-tauri/gen/android/build.gradle.kts");
expect(
  rootGradle,
  `classpath("com.android.tools.build:gradle:${env.get("ANDROID_GRADLE_PLUGIN_VERSION")}")`,
  "Android Gradle Plugin pin",
);
expect(
  rootGradle,
  `classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:${env.get("ANDROID_KOTLIN_VERSION")}")`,
  "Kotlin Gradle plugin pin",
);
for (const artifact of ["kotlin-stdlib", "kotlin-stdlib-common", "kotlin-stdlib-jdk7", "kotlin-stdlib-jdk8"]) {
  expect(rootGradle, `"org.jetbrains.kotlin:${artifact}:${env.get("ANDROID_KOTLIN_VERSION")}"`, `${artifact} pin`);
}
for (const artifact of ["kotlinx-coroutines-android", "kotlinx-coroutines-core-jvm"]) {
  expect(
    rootGradle,
    `"org.jetbrains.kotlinx:${artifact}:${env.get("ANDROID_COROUTINES_VERSION")}"`,
    `${artifact} pin`,
  );
}

const pluginGradle = await read("plugins/ostudy-platform/android/build.gradle.kts");
expect(
  pluginGradle,
  `kotlinx-coroutines-android:${env.get("ANDROID_COROUTINES_VERSION")}`,
  "platform coroutine compatibility pin",
);
const permissions = await read("plugins/ostudy-platform/permissions/default.toml");
expect(permissions, '"allow-register-listener"', "listener registration permission");
expect(permissions, '"allow-remove-listener"', "listener removal permission");

const mainActivity = await read(
  "src-tauri/gen/android/app/src/main/java/io/github/jferments/ostudy/MainActivity.kt",
);
expect(mainActivity, "override val handleBackNavigation: Boolean = false", "exclusive Android Back ownership");
const backInterception = await read("src/platform/backInterception.ts");
expect(backInterception, "while (this.applied !== this.desired)", "serialized Back convergence");
const platformProvider = await read("src/platform/PlatformProvider.tsx");
expect(platformProvider, "new BackInterceptionSynchronizer()", "Back interception synchronizer");
expect(platformProvider, 'root.dataset.androidBackListener = "ready";', "Back listener readiness");
expect(platformProvider, "dataset.androidBackInterception", "acknowledged Back state");

const androidBuild = await read("scripts/android-build.sh");
expect(androidBuild, 'verify-android-smoke-frontend.mjs" present', "smoke bridge inclusion check");
expect(androidBuild, 'verify-android-smoke-frontend.mjs" absent', "smoke bridge exclusion check");
expect(androidBuild, "manifest_backup", "generated-manifest restoration");

const gradleContracts = await read("scripts/android-gradle-contracts.sh");
for (const token of [
  "tauri.settings.gradle",
  "libostudy_lib.so",
  ":tauri-plugin-ostudy-platform:dependencies",
  ":tauri-plugin-ostudy-platform:compileDebugKotlin",
]) {
  expect(gradleContracts, token, `Tauri-first Gradle contract ${token}`);
}

const postbuild = await read("scripts/android-gradle-postbuild.sh");
for (const token of [
  "jvm-tests",
  "lint",
  "instrumentation-package",
  "test-packages",
  "-x :app:rustBuildUniversalDebug",
  "-x :app:rustBuildX86_64Debug",
  ":tauri-plugin-ostudy-platform:testDebugUnitTest",
  ":tauri-plugin-ostudy-platform:lintDebug",
  ":app:testUniversalDebugUnitTest",
  ":app:lintUniversalDebug",
  ":app:assembleUniversalDebugAndroidTest",
]) {
  expect(postbuild, token, `postbuild contract ${token}`);
}

const androidTest = await read("scripts/android-test.sh");
for (const token of [
  "ostudy_android_gate_smoke_frontend",
  "ostudy_android_gate_build_arm64_debug",
  "ostudy_android_gate_build_x86_smoke",
  "ostudy_android_gate_gradle_contracts",
  "ostudy_android_gate_test_packages",
  "ostudy_android_gate_build_cleanliness",
  "--app-apk",
  "--test-apk",
]) {
  expect(androidTest, token, `Android qualification stage ${token}`);
}
if (
  androidTest.indexOf("ostudy_android_gate_gradle_contracts") <=
  androidTest.indexOf("ostudy_android_gate_build_x86_smoke")
) {
  failures.push("Tauri x86 build must precede direct Gradle qualification.");
}
forbid(androidTest, ":app:rustBuild", "direct generated rustBuild invocation");

const diagnostic = await read("scripts/check-diagnostic.sh");
for (const token of ["android-jvm-tests", "android-lint", "android-instrumentation-package", "android-emulator-smoke"]) {
  expect(diagnostic, token, `independent diagnostic gate ${token}`);
}

const avd = await read("scripts/android-avd.sh");
for (const token of [
  "connected_emulator_serial_for_avd",
  'PID_FILE="$ROOT_DIR/.android-emulator-${safe_avd_name}.pid"',
  "wait_for_avd_disconnect",
]) {
  expect(avd, token, `isolated AVD lifecycle ${token}`);
}
const smokeRunner = await read("scripts/android-emulator-smoke.sh");
for (const token of [
  "serial_for_avd",
  'android-avd.sh stop --name "$AVD_NAME"',
  "summarize-android-smoke.mjs",
  "h_documentProviderRoundTripStreamsImportsAndExport",
  "--app-apk",
  "--test-apk",
]) {
  expect(smokeRunner, token, `emulator smoke contract ${token}`);
}

const providerManifest = await read("src-tauri/gen/android/app/src/androidTest/AndroidManifest.xml");
expect(providerManifest, "android.content.action.DOCUMENTS_PROVIDER", "test DocumentsProvider registration");
expect(providerManifest, "io.github.jferments.ostudy.dev.test.documents", "test provider authority");
const provider = await read(providerPath);
expect(
  provider,
  "public final class OstudyTestDocumentsProvider extends DocumentsProvider",
  "Java-only DocumentsProvider",
);
for (const fixture of ["android-fixture.ostudy.json", "android-fixture.png", "android-fixture.pdf"]) {
  expect(provider, fixture, `system-picker fixture ${fixture}`);
}
if (/\b(?:kotlin|androidx)\./u.test(provider)) {
  failures.push("Standalone test DocumentsProvider must not depend on Kotlin or AndroidX runtime classes.");
}
const providerSmoke = await read(
  "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidDocumentsProviderSmokeTest.kt",
);
for (const token of [
  'purpose = "study_content"',
  'purpose = "image"',
  'purpose = "pdf_document"',
  "startDeckExportPicker",
  'getString("destinationKind")',
  'getLong("byteSize")',
]) {
  expect(providerSmoke, token, `system-picker success assertion ${token}`);
}

const bridge = await read("src/testing/androidTestBridge.ts");
expect(bridge, "interface AndroidFileOperationResult", "structured test file-operation result");
expect(bridge, "detectedKind", "successful import result kind");
expect(bridge, "destinationKind", "successful export destination result");

const runnerDoctor = await read("scripts/android-runner-doctor.sh");
expect(runnerDoctor, "/dev/kvm", "KVM availability check");
expect(runnerDoctor, "--system-image", "selectable emulator image check");
const packageVerifier = await read("scripts/verify-android-package.sh");
forbid(packageVerifier, "VITE_OSTUDY_ANDROID_TEST_BRIDGE", "APK inference of frontend smoke-bridge presence");

const androidWorkflow = await read(".github/workflows/android-check.yml");
const packageWorkflow = await read(".github/workflows/android-package.yml");
expect(
  androidWorkflow,
  "runs-on: [self-hosted, linux, x64, ostudy-android]",
  "dedicated Android compile runner",
);
const kvmSelectors = androidWorkflow.match(/runs-on: \[self-hosted, linux, x64, ostudy-android, kvm\]/gu) ?? [];
if (kvmSelectors.length < 3) failures.push("Android diagnostic, emulator, and minimum-SDK jobs must require KVM.");
expect(
  packageWorkflow,
  "runs-on: [self-hosted, linux, x64, ostudy-android]",
  "dedicated Android package runner",
);
forbid(androidWorkflow, "runs-on: [self-hosted, linux, x64, ostudy, android", "Android routing to the VM runner");
forbid(androidWorkflow, "OSTUDY_CARGO_TARGET_DIR: $HOME/", "literal unexpanded HOME in workflow env");

const wrapper = await read("src-tauri/gen/android/gradle/wrapper/gradle-wrapper.properties");
expect(wrapper, `gradle-${env.get("ANDROID_GRADLE_VERSION")}-bin.zip`, "Gradle wrapper pin");
const manifest = await read("src-tauri/gen/android/app/src/main/AndroidManifest.xml");
for (const token of [
  "android.permission.INTERNET",
  "android.permission.ACCESS_NETWORK_STATE",
  'android:allowBackup="false"',
  "androidx.core.content.FileProvider",
  "android.intent.action.SEND",
]) {
  expect(manifest, token, `manifest contract ${token}`);
}
for (const forbidden of ["READ_EXTERNAL_STORAGE", "WRITE_EXTERNAL_STORAGE", "MANAGE_EXTERNAL_STORAGE", "REQUEST_INSTALL_PACKAGES"]) {
  forbid(manifest, forbidden, `forbidden Android permission ${forbidden}`);
}
const filePaths = await read("src-tauri/gen/android/app/src/main/res/xml/file_paths.xml");
expect(filePaths, 'path="transfers/exports/"', "prepared-export FileProvider scope");
forbid(filePaths, "external-path", "broad external FileProvider scope");
forbid(filePaths, 'path="."', "root FileProvider scope");

const androidConfig = JSON.parse(await read("src-tauri/tauri.android.conf.json"));
if (androidConfig.bundle?.android?.minSdkVersion !== Number(env.get("ANDROID_MIN_SDK"))) {
  failures.push("Tauri Android minSdkVersion does not match versions.env.");
}
if (androidConfig.bundle?.android?.debugApplicationIdSuffix !== ".dev") {
  failures.push("Tauri Android debug application suffix must remain .dev.");
}
if (androidConfig.bundle?.resources != null) failures.push("Android must not bundle the desktop PDFium resource tree.");

const pkg = JSON.parse(await read("package.json"));
for (const command of [
  "android:doctor",
  "android:runner:doctor",
  "android:dev",
  "android:build:debug",
  "android:build:aab",
  "android:avd",
  "android:smoke",
  "android:diagnostic",
  "android:smoke:min",
]) {
  if (!pkg.scripts?.[command]) failures.push(`package.json is missing ${command}`);
}

if (failures.length > 0) {
  console.error("Android project verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Android project: dedicated runner routing, Tauri-first builds, independent diagnostics, isolated AVD lifecycle, Java-only system-picker fixtures, Back/bridge observability, package policy, and toolchain pins are canonical.",
);

function expect(text, token, label) {
  if (!text.includes(token)) failures.push(`Missing ${label}`);
}
function forbid(text, token, label) {
  if (text.includes(token)) failures.push(`Found ${label}`);
}
async function read(path) {
  return readFile(resolve(root, path), "utf8");
}
function parseEnv(text) {
  const values = new Map();
  for (const line of text.split(/\r?\n/u)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const equals = trimmed.indexOf("=");
    if (equals < 1) continue;
    let value = trimmed.slice(equals + 1).trim();
    if (
      (value.startsWith("'") && value.endsWith("'")) ||
      (value.startsWith('"') && value.endsWith('"'))
    ) {
      value = value.slice(1, -1);
    }
    values.set(trimmed.slice(0, equals), value);
  }
  return values;
}
