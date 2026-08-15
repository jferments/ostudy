#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const failures = [];
const required = [
  "src-tauri/tauri.android.conf.json",
  "src-tauri/gen/android/gradlew",
  "src-tauri/gen/android/gradle/wrapper/gradle-wrapper.properties",
  "src-tauri/gen/android/gradle.properties",
  "src-tauri/gen/android/app/build.gradle.kts",
  "src-tauri/gen/android/app/src/main/AndroidManifest.xml",
  "src-tauri/gen/android/app/src/main/java/io/github/jferments/ostudy/MainActivity.kt",
  "src-tauri/gen/android/app/src/main/res/xml/file_paths.xml",
  "src-tauri/gen/android/app/src/main/res/xml/network_security_config.xml",
  "src-tauri/gen/android/app/src/main/res/xml/backup_rules.xml",
  "src-tauri/gen/android/app/src/main/res/xml/data_extraction_rules.xml",
  "src-tauri/gen/android/app/src/androidTest/AndroidManifest.xml",
  "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidSmokeTest.kt",
  "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidDocumentsProviderSmokeTest.kt",
  "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/OstudyTestDocumentsProvider.kt",
  "plugins/ostudy-platform/Cargo.toml",
  "plugins/ostudy-platform/build.rs",
  "plugins/ostudy-platform/permissions/default.toml",
  "plugins/ostudy-platform/android/build.gradle.kts",
  "plugins/ostudy-platform/android/src/main/java/OstudyPlatformPlugin.kt",
  "plugins/ostudy-platform/android/src/main/java/InvokeArrayResult.kt",
  "src/components/IncomingShareCoordinator.tsx",
  "src/platform/PlatformProvider.tsx",
  "src/platform/backInterception.ts",
  "src/platform/backInterception.test.ts",
  "src/testing/androidTestBridge.ts",
  "src-tauri/src/platform/android.rs",
  "src-tauri/src/ui_checkpoint.rs",
  "src-tauri/src/practice_resume.rs",
  "scripts/qualification-gates.sh",
  "scripts/android-qualification-gates.sh",
  "scripts/android-env.sh",
  "scripts/android-doctor.sh",
  "scripts/android-runner-doctor.sh",
  "scripts/android-build.sh",
  "scripts/android-gradle-contracts.sh",
  "scripts/android-gradle-postbuild.sh",
  "scripts/android-smoke-frontend-preflight.sh",
  "scripts/verify-android-smoke-frontend.mjs",
  "scripts/verify-build-cleanliness.sh",
  "scripts/summarize-android-smoke.mjs",
  "scripts/android-test.sh",
  "scripts/android-dev.sh",
  "scripts/android-avd.sh",
  "scripts/android-emulator-smoke.sh",
  "scripts/verify-android-package.sh",
];
for (const path of required) {
  if (!existsSync(resolve(root, path))) failures.push(`Missing Android project file: ${path}`);
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
  "ANDROID_COMMANDLINE_TOOLS_BUILD",
  "ANDROID_COMMANDLINE_TOOLS_SHA256",
  "ANDROID_PHONE_SYSTEM_IMAGE",
  "ANDROID_PHONE_AVD",
  "ANDROID_MIN_SYSTEM_IMAGE",
  "ANDROID_MIN_AVD",
]) {
  if (!env.has(key) || env.get(key) === "") failures.push(`versions.env is missing ${key}`);
}
if (!env.get("ANDROID_MIN_SYSTEM_IMAGE")?.includes(`android-${env.get("ANDROID_MIN_SDK")};`)) {
  failures.push("ANDROID_MIN_SYSTEM_IMAGE must exercise the declared ANDROID_MIN_SDK");
}

let trackedLocalProperties = false;
try {
  execFileSync("git", ["ls-files", "--error-unmatch", "src-tauri/gen/android/local.properties"], {
    cwd: root,
    stdio: "ignore",
  });
  trackedLocalProperties = true;
} catch {
  trackedLocalProperties = false;
}
if (trackedLocalProperties) {
  failures.push("src-tauri/gen/android/local.properties must remain untracked and machine-local");
}

const gradle = await read("src-tauri/gen/android/app/build.gradle.kts");
expectContains(gradle, `compileSdk = ${env.get("ANDROID_COMPILE_SDK")}`, "compileSdk pin");
expectContains(gradle, `minSdk = ${env.get("ANDROID_MIN_SDK")}`, "minSdk pin");
expectContains(gradle, `targetSdk = ${env.get("ANDROID_TARGET_SDK")}`, "targetSdk pin");
expectContains(gradle, 'applicationId = "io.github.jferments.ostudy"', "application identity");
expectContains(
  gradle,
  'testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"',
  "instrumentation runner",
);

const mainActivity = await read(
  "src-tauri/gen/android/app/src/main/java/io/github/jferments/ostudy/MainActivity.kt",
);
expectContains(
  mainActivity,
  "override val handleBackNavigation: Boolean = false",
  "exclusive ostudy Android Back ownership over Wry browser-history Back",
);

const rootGradle = await read("src-tauri/gen/android/build.gradle.kts");
expectContains(
  rootGradle,
  `classpath("com.android.tools.build:gradle:${env.get("ANDROID_GRADLE_PLUGIN_VERSION")}")`,
  "Android Gradle Plugin pin",
);
expectContains(
  rootGradle,
  `classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:${env.get("ANDROID_KOTLIN_VERSION")}")`,
  "Kotlin Gradle plugin pin",
);
for (const artifact of ["kotlin-stdlib", "kotlin-stdlib-common", "kotlin-stdlib-jdk7", "kotlin-stdlib-jdk8"]) {
  expectContains(
    rootGradle,
    `"org.jetbrains.kotlin:${artifact}:${env.get("ANDROID_KOTLIN_VERSION")}"`,
    `${artifact} compatibility pin`,
  );
}
for (const artifact of ["kotlinx-coroutines-android", "kotlinx-coroutines-core-jvm"]) {
  expectContains(
    rootGradle,
    `"org.jetbrains.kotlinx:${artifact}:${env.get("ANDROID_COROUTINES_VERSION")}"`,
    `${artifact} compatibility pin`,
  );
}
expectContains(rootGradle, "maxParallelForks", "parallel Android JVM test configuration");

const pluginGradle = await read("plugins/ostudy-platform/android/build.gradle.kts");
expectContains(
  pluginGradle,
  `implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:${env.get("ANDROID_COROUTINES_VERSION")}")`,
  "platform plugin coroutine declaration",
);

const platformPermissions = await read("plugins/ostudy-platform/permissions/default.toml");
for (const permission of ["allow-register-listener", "allow-remove-listener"]) {
  expectContains(platformPermissions, `"${permission}"`, `platform event permission ${permission}`);
}
const platformBuild = await read("plugins/ostudy-platform/build.rs");
for (const command of ["register_listener", "remove_listener"]) {
  expectContains(platformBuild, `"${command}"`, `generated listener permission command ${command}`);
}

const gradleProperties = await read("src-tauri/gen/android/gradle.properties");
expectContains(gradleProperties, "org.gradle.parallel=true", "Gradle project parallelism");
expectContains(gradleProperties, "org.gradle.caching=true", "Gradle build cache");

const androidEnv = await read("scripts/android-env.sh");
for (const token of ["OSTUDY_ANDROID_JOBS", "OSTUDY_GRADLE_WORKERS", "CARGO_BUILD_JOBS", "VITEST_MAX_WORKERS"]) {
  expectContains(androidEnv, token, `Android parallelism control ${token}`);
}
if (/^set\s+-[^\n]*e/mu.test(androidEnv)) {
  failures.push("android-env.sh must not enable errexit when sourced into an interactive shell");
}

const androidBuild = await read("scripts/android-build.sh");
expectContains(
  androidBuild,
  'verify-android-smoke-frontend.mjs" present',
  "smoke build frontend bridge verifier",
);
expectContains(
  androidBuild,
  'verify-android-smoke-frontend.mjs" absent',
  "normal build frontend bridge exclusion verifier",
);
expectContains(androidBuild, "manifest_backup", "tracked Android manifest restoration");

const packageVerifier = await read("scripts/verify-android-package.sh");
if (packageVerifier.includes("bridge_required") || packageVerifier.includes("VITE_OSTUDY_ANDROID_TEST_BRIDGE")) {
  failures.push(
    "verify-android-package.sh must verify observable package properties rather than infer smoke bridge presence",
  );
}
expectContains(packageVerifier, '[[ "$MODE" == release ]]', "release-only smoke marker defense");

const backInterception = await read("src/platform/backInterception.ts");
expectContains(
  backInterception,
  "while (this.applied !== this.desired)",
  "serialized Android Back interception convergence loop",
);
const platformProvider = await read("src/platform/PlatformProvider.tsx");
expectContains(platformProvider, "new BackInterceptionSynchronizer()", "Android Back interception synchronizer");
expectContains(platformProvider, "dataset.androidBackInterception", "acknowledged Android Back state");
expectContains(platformProvider, 'root.dataset.androidBackListener = "ready";', "Android Back listener readiness");
expectContains(
  platformProvider,
  'root.dataset.androidBackListener = "ready";\n        const backStack = subscribePlatformBackStack',
  "listener-before-interception Android Back ordering",
);
for (const diagnostic of [
  "Unable to register the Android Back listener:",
  "Unable to register the Android insets listener:",
  "Unable to register the Android network listener:",
]) {
  expectContains(platformProvider, diagnostic, `Android plugin listener diagnostic ${diagnostic}`);
}
const incomingShareCoordinator = await read("src/components/IncomingShareCoordinator.tsx");
expectContains(
  incomingShareCoordinator,
  "Unable to register the Android incoming-share listener:",
  "incoming-share plugin listener diagnostic",
);

const gradleContracts = await read("scripts/android-gradle-contracts.sh");
for (const token of [
  "tauri.settings.gradle",
  "libostudy_lib.so",
  ":tauri-plugin-ostudy-platform:dependencies",
  ":tauri-plugin-ostudy-platform:compileDebugKotlin",
  "ANDROID_KOTLIN_VERSION",
  "ANDROID_COROUTINES_VERSION",
]) {
  expectContains(gradleContracts, token, `Tauri-first Gradle contract ${token}`);
}

const postbuild = await read("scripts/android-gradle-postbuild.sh");
for (const token of [
  "tauri.settings.gradle",
  "-PabiList=x86_64",
  "-ParchList=x86_64",
  "-PtargetList=x86_64",
  "-x :app:rustBuildUniversalDebug",
  "-x :app:rustBuildX86_64Debug",
  ":tauri-plugin-ostudy-platform:testDebugUnitTest",
  ":tauri-plugin-ostudy-platform:lintDebug",
  ":app:testUniversalDebugUnitTest",
  ":app:lintUniversalDebug",
  ":app:assembleUniversalDebugAndroidTest",
]) {
  expectContains(postbuild, token, `Android Gradle postbuild invariant ${token}`);
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
  expectContains(androidTest, token, `Android qualification stage ${token}`);
}
const x86BuildIndex = androidTest.indexOf("ostudy_android_gate_build_x86_smoke");
const gradleIndex = androidTest.indexOf("ostudy_android_gate_gradle_contracts");
if (x86BuildIndex < 0 || gradleIndex <= x86BuildIndex) {
  failures.push("android-test.sh must initialize Tauri Android state before direct Gradle qualification");
}
if (androidTest.includes(":app:rustBuild")) {
  failures.push("android-test.sh must not invoke generated Tauri rustBuild tasks directly");
}

const emulatorSmoke = await read("scripts/android-emulator-smoke.sh");
for (const token of [
  "--app-apk",
  "--test-apk",
  "summarize-android-smoke.mjs",
  "android-smoke-results.tsv",
  "dump_case_logcat",
  'shell am force-stop "$APP_ID"',
  "-e waitForActivitiesToComplete false",
  "h_documentProviderRoundTripStreamsImportsAndExport",
]) {
  expectContains(emulatorSmoke, token, `Android emulator smoke invariant ${token}`);
}
if (emulatorSmoke.includes(":app:rustBuild")) {
  failures.push("android-emulator-smoke.sh must not invoke generated Tauri rustBuild tasks directly");
}

const androidSmokeTest = await read(
  "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidSmokeTest.kt",
);
for (const token of [
  "fun a0_appLaunchesAndTestBridgeIsReady()",
  "waitForAndroidBackReady(webView)",
  "dataset.androidBackInterception === 'true'",
  "dataset.overlayActive !== 'true'",
  "waitForWebView(scenario)",
]) {
  expectContains(androidSmokeTest, token, `Android primary smoke contract ${token}`);
}
if (androidSmokeTest.includes("ActivityScenario.launch(MainActivity::class.java).use")) {
  failures.push(
    "AndroidSmokeTest must not close the Tauri Activity from inside instrumentation; shell process cleanup owns Activity destruction",
  );
}

const providerManifest = await read("src-tauri/gen/android/app/src/androidTest/AndroidManifest.xml");
expectContains(providerManifest, "android.content.action.DOCUMENTS_PROVIDER", "test DocumentsProvider intent");
expectContains(providerManifest, "io.github.jferments.ostudy.dev.test.documents", "test DocumentsProvider authority");
const provider = await read(
  "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/OstudyTestDocumentsProvider.kt",
);
for (const token of ["extends", "ostudy test fixtures", "android-fixture.ostudy.json", "android-fixture.png", "android-fixture.pdf"]) {
  if (token === "extends") continue;
  expectContains(provider, token, `DocumentsProvider fixture ${token}`);
}
expectContains(provider, "class OstudyTestDocumentsProvider : DocumentsProvider()", "instrumentation DocumentsProvider class");
const providerSmoke = await read(
  "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidDocumentsProviderSmokeTest.kt",
);
for (const token of [
  "h_documentProviderRoundTripStreamsImportsAndExport",
  'purpose = "study_content"',
  'purpose = "image"',
  'purpose = "pdf_document"',
  "startDeckExportPicker",
  "exportedFiles",
]) {
  expectContains(providerSmoke, token, `system picker success smoke ${token}`);
}

const bridge = await read("src/testing/androidTestBridge.ts");
expectContains(bridge, "interface AndroidFileOperationResult", "structured Android file operation result");
expectContains(bridge, "detectedKind", "successful import detection result");
expectContains(bridge, "destinationKind", "successful export destination result");

const avd = await read("scripts/android-avd.sh");
expectContains(avd, "--system-image", "selectable emulator system image");
expectContains(avd, "connected_emulator_name", "requested AVD identity check");
const installDeps = await read("scripts/install-android-deps.sh");
expectContains(installDeps, "--with-min-emulator", "minimum-SDK emulator dependency mode");
const runnerDoctor = await read("scripts/android-runner-doctor.sh");
expectContains(runnerDoctor, "--system-image", "runner doctor image selection");

const arrayAdapter = await read("plugins/ostudy-platform/android/src/main/java/InvokeArrayResult.kt");
expectContains(arrayAdapter, 'envelope.put("items", result)', "Android array result envelope");

const wrapper = await read("src-tauri/gen/android/gradle/wrapper/gradle-wrapper.properties");
expectContains(wrapper, `gradle-${env.get("ANDROID_GRADLE_VERSION")}-bin.zip`, "Gradle wrapper pin");

const manifest = await read("src-tauri/gen/android/app/src/main/AndroidManifest.xml");
for (const requiredText of [
  "android.permission.INTERNET",
  "android.permission.ACCESS_NETWORK_STATE",
  'android:allowBackup="false"',
  'android:resizeableActivity="true"',
  "androidx.core.content.FileProvider",
  "${applicationId}.fileprovider",
  "android.intent.action.SEND",
]) {
  expectContains(manifest, requiredText, `Android manifest entry ${requiredText}`);
}
for (const forbidden of [
  "READ_EXTERNAL_STORAGE",
  "WRITE_EXTERNAL_STORAGE",
  "MANAGE_EXTERNAL_STORAGE",
  "REQUEST_INSTALL_PACKAGES",
]) {
  if (manifest.includes(forbidden)) failures.push(`Android manifest contains forbidden permission ${forbidden}`);
}

const fileProvider = await read("src-tauri/gen/android/app/src/main/res/xml/file_paths.xml");
if (fileProvider.includes("external-path") || fileProvider.includes('path="."')) {
  failures.push("Android FileProvider must expose only the app-private prepared-export directory");
}
expectContains(fileProvider, 'path="transfers/exports/"', "prepared-export FileProvider scope");

const baseConfig = JSON.parse(await read("src-tauri/tauri.conf.json"));
if (baseConfig.bundle?.resources) {
  failures.push("Desktop PDFium resources must not remain in the shared Tauri bundle configuration");
}
const linuxConfig = JSON.parse(await read("src-tauri/tauri.linux.conf.json"));
if (!linuxConfig.bundle?.resources?.["resources/native/pdfium/*"]) {
  failures.push("Linux overlay must retain the managed PDFium bundle resource");
}
const androidConfig = JSON.parse(await read("src-tauri/tauri.android.conf.json"));
if (androidConfig.bundle?.android?.minSdkVersion !== Number(env.get("ANDROID_MIN_SDK"))) {
  failures.push("Tauri Android minSdkVersion does not match versions.env");
}
if (androidConfig.bundle?.android?.debugApplicationIdSuffix !== ".dev") {
  failures.push("Tauri Android debugApplicationIdSuffix must be .dev");
}
if (androidConfig.bundle?.resources != null) {
  failures.push("Android Tauri configuration must not bundle PDFium resources during the current mobile boundary");
}

const packageJson = JSON.parse(await read("package.json"));
for (const command of [
  "android:doctor",
  "android:runner:doctor",
  "android:dev",
  "android:build:debug",
  "android:build:aab",
  "android:avd",
  "android:smoke",
]) {
  if (!packageJson.scripts?.[command]) failures.push(`package.json is missing ${command}`);
}

const cargo = await read("src-tauri/Cargo.toml");
expectContains(cargo, "tauri-plugin-ostudy-platform", "private platform plugin dependency");

if (failures.length > 0) {
  console.error("Android project verification failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Android project: Tauri-first build ownership, bridge observability, Back synchronization, system-picker streaming, arm64/x86 qualification, Gradle Lint, minimum-SDK image, structured smoke evidence, package policy, and toolchain pins are canonical.",
);

function expectContains(text, value, label) {
  if (!text.includes(value)) failures.push(`Missing ${label}`);
}
async function read(path) {
  return readFile(resolve(root, path), "utf8");
}
function parseEnv(text) {
  const values = new Map();
  for (const line of text.split(/\r?\n/u)) {
    const trimmed = line.trim();
    if (trimmed === "" || trimmed.startsWith("#")) continue;
    const index = trimmed.indexOf("=");
    if (index < 1) continue;
    const key = trimmed.slice(0, index);
    let value = trimmed.slice(index + 1).trim();
    if (
      (value.startsWith("'") && value.endsWith("'")) ||
      (value.startsWith('"') && value.endsWith('"'))
    ) {
      value = value.slice(1, -1);
    }
    values.set(key, value);
  }
  return values;
}
