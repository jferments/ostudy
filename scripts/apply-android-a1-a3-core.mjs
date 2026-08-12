#!/usr/bin/env node

import { readFile, writeFile, mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFile(join(root, path), "utf8");
async function write(path, content) {
  await mkdir(dirname(join(root, path)), { recursive: true });
  await writeFile(join(root, path), content, "utf8");
}
function replaceExact(text, before, after, label) {
  if (!text.includes(before)) throw new Error(`${label}: expected source not found`);
  return text.replace(before, after);
}
function replaceBetween(text, startMarker, endMarker, replacement, label) {
  const start = text.indexOf(startMarker);
  const end = text.indexOf(endMarker, start + startMarker.length);
  if (start < 0 || end < 0) throw new Error(`${label}: range not found`);
  return text.slice(0, start) + replacement + text.slice(end);
}

// App dependency and platform-specific bundle resources.
let cargo = await read("src-tauri/Cargo.toml");
cargo = replaceExact(
  cargo,
  'tauri-plugin-opener = "=2.5.4"\n',
  'tauri-plugin-opener = "=2.5.4"\ntauri-plugin-ostudy-platform = { path = "../plugins/ostudy-platform" }\n',
  "register private platform plugin dependency",
);
await write("src-tauri/Cargo.toml", cargo);

let baseConfig = JSON.parse(await read("src-tauri/tauri.conf.json"));
delete baseConfig.bundle.resources;
baseConfig.bundle.fileAssociations = [
  { ext: ["studydeck"], mimeType: "application/vnd.ostudy.studydeck", name: "ostudy deck" },
  { ext: ["ostudybackup"], mimeType: "application/vnd.ostudy.backup", name: "ostudy backup" },
  { ext: ["json"], mimeType: "application/json", name: "ostudy generated flashcards" },
];
await write("src-tauri/tauri.conf.json", `${JSON.stringify(baseConfig, null, 2)}\n`);
await write(
  "src-tauri/tauri.linux.conf.json",
  `${JSON.stringify({ bundle: { resources: { "resources/native/pdfium/*": "native/pdfium/" } } }, null, 2)}\n`,
);
await write(
  "src-tauri/tauri.android.conf.json",
  `${JSON.stringify({
    app: {
      windows: [{ label: "main", title: "ostudy", fullscreen: false, resizable: true }],
    },
    bundle: {
      active: true,
      targets: ["apk", "aab"],
      android: { minSdkVersion: 28, versionCode: 1 },
      resources: null,
    },
  }, null, 2)}\n`,
);

// Tauri capability for private plugin event listening.
let capability = JSON.parse(await read("src-tauri/capabilities/main.json"));
if (!capability.permissions.includes("ostudy-platform:default")) {
  capability.permissions.push("ostudy-platform:default");
}
await write("src-tauri/capabilities/main.json", `${JSON.stringify(capability, null, 2)}\n`);
await write(
  "plugins/ostudy-platform/permissions/default.toml",
  `"$schema" = "schemas/schema.json"\n\n[default]\ndescription = "Allows the trusted ostudy WebView to listen for native Android platform events."\npermissions = []\n`,
);

// Extend platform contracts for URI streaming and system sharing.
let platform = await read("src-tauri/src/platform/mod.rs");
platform = replaceExact(
  platform,
  `#[cfg(not(any(target_os = "android", target_os = "ios")))]\nmod desktop;\n#[cfg(any(target_os = "android", target_os = "ios"))]\nmod mobile_stub;`,
  `#[cfg(target_os = "android")]\nmod android;\n#[cfg(not(any(target_os = "android", target_os = "ios")))]\nmod desktop;\n#[cfg(any(target_os = "android", target_os = "ios"))]\nmod mobile_stub;`,
  "register Android platform module",
);
platform = replaceBetween(
  platform,
  `#[derive(Clone, Debug)]\npub struct ExternalSource`,
  `#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]\n#[serde(rename_all = "snake_case")]\npub enum LifecyclePhase`,
  `#[derive(Clone, Debug)]
pub struct ExternalImportTarget {
    pub path: PathBuf,
    pub maximum_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct ExternalAcquireResult {
    pub display_name: String,
    pub reported_media_type: Option<String>,
    pub copied_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct ExternalExportSource {
    pub path: PathBuf,
    pub display_name: String,
    pub media_type: String,
    pub maximum_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct ExternalPublishResult {
    pub display_name: String,
    pub copied_bytes: u64,
    pub destination_kind: String,
}

pub type PlatformFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait ExternalFilePort: Send + Sync {
    fn acquire_import(
        &self,
        app: AppHandle,
        request: ExternalFileRequest,
        target: ExternalImportTarget,
    ) -> PlatformFuture<Result<Option<ExternalAcquireResult>, CommandError>>;

    fn publish_export(
        &self,
        app: AppHandle,
        request: ExternalFileRequest,
        source: ExternalExportSource,
    ) -> PlatformFuture<Result<Option<ExternalPublishResult>, CommandError>>;
}

#[derive(Clone, Debug)]
pub struct ShareSource {
    pub title: String,
    pub path: PathBuf,
    pub display_name: String,
    pub media_type: String,
}

#[derive(Clone, Debug)]
pub struct ShareLaunchResult {
    pub launched: bool,
    pub destination_kind: String,
}

pub trait SystemSharePort: Send + Sync {
    fn share_file(
        &self,
        app: AppHandle,
        source: ShareSource,
    ) -> PlatformFuture<Result<ShareLaunchResult, CommandError>>;
}

#[derive(Default)]
pub struct UnavailableSystemSharePort;

impl SystemSharePort for UnavailableSystemSharePort {
    fn share_file(
        &self,
        _app: AppHandle,
        _source: ShareSource,
    ) -> PlatformFuture<Result<ShareLaunchResult, CommandError>> {
        Box::pin(async {
            Err(CommandError::new(
                "system_share_unavailable",
                "This build does not provide a native system share sheet.",
            ))
        })
    }
}

`,
  "replace external file platform contract",
);
platform = replaceExact(
  platform,
  `pub struct ExternalFileRequest {\n    pub title: String,\n    pub suggested_filename: Option<String>,\n    pub filters: Vec<FileFilter>,\n}`,
  `pub struct ExternalFileRequest {\n    pub title: String,\n    pub suggested_filename: Option<String>,\n    pub filters: Vec<FileFilter>,\n    pub mime_types: Vec<String>,\n}`,
  "add Android MIME filters",
);
platform = replaceExact(
  platform,
  `    resources: Arc<dyn ResourceProbe>,\n}`,
  `    resources: Arc<dyn ResourceProbe>,\n    system_share: Arc<dyn SystemSharePort>,\n}`,
  "add system share runtime field",
);
platform = replaceExact(
  platform,
  `    pub fn current() -> Self {\n        #[cfg(not(any(target_os = "android", target_os = "ios")))]\n        {\n            desktop::runtime()\n        }\n        #[cfg(any(target_os = "android", target_os = "ios"))]\n        {\n            mobile_stub::runtime()\n        }\n    }`,
  `    pub fn current() -> Self {\n        #[cfg(not(any(target_os = "android", target_os = "ios")))]\n        {\n            desktop::runtime()\n        }\n        #[cfg(any(target_os = "android", target_os = "ios"))]\n        {\n            mobile_stub::runtime()\n        }\n    }\n\n    pub fn for_app(app: AppHandle) -> Self {\n        #[cfg(target_os = "android")]\n        {\n            android::runtime(app)\n        }\n        #[cfg(not(target_os = "android"))]\n        {\n            let _ = app;\n            Self::current()\n        }\n    }`,
  "add app-aware Android runtime factory",
);
platform = replaceExact(
  platform,
  `        time_zone: Arc<dyn TimeZoneProvider>,\n        resources: Arc<dyn ResourceProbe>,\n    ) -> Self {`,
  `        time_zone: Arc<dyn TimeZoneProvider>,\n        resources: Arc<dyn ResourceProbe>,\n        system_share: Arc<dyn SystemSharePort>,\n    ) -> Self {`,
  "extend runtime constructor arguments",
);
platform = replaceExact(
  platform,
  `            time_zone,\n            resources,\n        }`,
  `            time_zone,\n            resources,\n            system_share,\n        }`,
  "store system share port",
);
platform = replaceExact(
  platform,
  `    pub fn secure_secret_store(&self) -> Arc<dyn SecureSecretStore> {\n        Arc::clone(&self.secure_secrets)\n    }`,
  `    pub fn secure_secret_store(&self) -> Arc<dyn SecureSecretStore> {\n        Arc::clone(&self.secure_secrets)\n    }\n\n    pub fn system_share(&self) -> Arc<dyn SystemSharePort> {\n        Arc::clone(&self.system_share)\n    }`,
  "expose system share port",
);
platform = replaceExact(
  platform,
  `#[tauri::command]\npub fn get_platform_capabilities(\n    runtime: State<'_, PlatformRuntime>,\n) -> PlatformCapabilitySnapshot {\n    runtime.capabilities()\n}`,
  `#[tauri::command]\npub fn get_platform_capabilities(\n    runtime: State<'_, PlatformRuntime>,\n) -> PlatformCapabilitySnapshot {\n    runtime.capabilities()\n}\n\n#[tauri::command]\npub fn set_platform_back_interception(\n    app: AppHandle,\n    enabled: bool,\n) -> Result<(), CommandError> {\n    #[cfg(target_os = "android")]\n    {\n        use tauri_plugin_ostudy_platform::OstudyPlatformExt;\n        app.ostudy_platform()\n            .set_back_interception(enabled)\n            .map_err(|error| {\n                CommandError::new(\n                    "android_back_interception_failed",\n                    format!("Android Back interception could not be updated: {error}"),\n                )\n            })\n    }\n    #[cfg(not(target_os = "android"))]\n    {\n        let _ = (app, enabled);\n        Ok(())\n    }\n}`,
  "add Android Back command",
);
await write("src-tauri/src/platform/mod.rs", platform);

let desktop = await read("src-tauri/src/platform/desktop/mod.rs");
desktop = replaceExact(
  desktop,
  `        Arc::new(resources::SysinfoResourceProbe),\n    )`,
  `        Arc::new(resources::SysinfoResourceProbe),\n        Arc::new(crate::platform::UnavailableSystemSharePort),\n    )`,
  "add desktop unavailable share port",
);
await write("src-tauri/src/platform/desktop/mod.rs", desktop);

let mobileStub = await read("src-tauri/src/platform/mobile_stub.rs");
mobileStub = replaceExact(
  mobileStub,
  `        Arc::new(ConservativeMobileResourceProbe),\n    )`,
  `        Arc::new(ConservativeMobileResourceProbe),\n        Arc::new(crate::platform::UnavailableSystemSharePort),\n    )`,
  "add mobile fallback share port",
);
mobileStub = replaceBetween(
  mobileStub,
  `struct UnavailableExternalFilePort;`,
  `struct MobileSystemClock;`,
  `struct UnavailableExternalFilePort;

impl ExternalFilePort for UnavailableExternalFilePort {
    fn acquire_import(
        &self,
        _app: AppHandle,
        _request: ExternalFileRequest,
        _target: crate::platform::ExternalImportTarget,
    ) -> PlatformFuture<Result<Option<crate::platform::ExternalAcquireResult>, CommandError>> {
        Box::pin(async {
            Err(CommandError::new(
                "external_file_adapter_unavailable",
                "This mobile build does not include its native file-selection adapter.",
            ))
        })
    }

    fn publish_export(
        &self,
        _app: AppHandle,
        _request: ExternalFileRequest,
        _source: crate::platform::ExternalExportSource,
    ) -> PlatformFuture<Result<Option<crate::platform::ExternalPublishResult>, CommandError>> {
        Box::pin(async {
            Err(CommandError::new(
                "external_file_adapter_unavailable",
                "This mobile build does not include its native export adapter.",
            ))
        })
    }
}

`,
  "update fallback external file port",
);
mobileStub = mobileStub.replace(
  `        CapabilityProvider, ExternalDestination, ExternalFilePort, ExternalFileRequest,\n        ExternalSource, NativeServiceCapabilities, PlatformCapabilitySnapshot, PlatformFuture,`,
  `        CapabilityProvider, ExternalFilePort, ExternalFileRequest, NativeServiceCapabilities,\n        PlatformCapabilitySnapshot, PlatformFuture,`,
);
await write("src-tauri/src/platform/mobile_stub.rs", mobileStub);

// Update external-file use cases to let the adapter stream directly to/from managed files.
let external = await read("src-tauri/src/external_files.rs");
external = replaceExact(
  external,
  `    platform::{ExternalFileRequest, FileFilter, PlatformRuntime},`,
  `    platform::{\n        ExternalExportSource, ExternalFileRequest, ExternalImportTarget, FileFilter,\n        PlatformRuntime, ShareSource,\n    },`,
  "import streaming platform types",
);
external = external.replaceAll(
  `                filters: vec![FileFilter {`,
  `                mime_types: Vec::new(),\n                filters: vec![FileFilter {`,
);
external = replaceExact(
  external,
  `    let Some(source) = runtime\n        .external_files()\n        .select_import(app.clone(), input.purpose.request())\n        .await?\n    else {\n        return Ok(None);\n    };\n    let source_path = fs::canonicalize(&source.path).map_err(CommandError::io)?;\n    if !source_path.is_file() {\n        return Err(CommandError::new(\n            "external_file_missing",\n            "The selected file is no longer available.",\n        ));\n    }\n    let display_name = validate_display_name(&source.display_name)?;\n    let partial_relative = AppPaths::staged_import_partial_relative_path(&input.operation_id);\n    let partial_path = paths.resolve_relative(&partial_relative);\n    create_parent(&partial_path)?;\n    remove_file_if_exists(&partial_path)?;\n    let (byte_size, sha256) =\n        copy_bounded_with_hash(&source_path, &partial_path, input.purpose.maximum_bytes())?;`,
  `    let partial_relative = AppPaths::staged_import_partial_relative_path(&input.operation_id);\n    let partial_path = paths.resolve_relative(&partial_relative);\n    create_parent(&partial_path)?;\n    remove_file_if_exists(&partial_path)?;\n    let acquired = runtime\n        .external_files()\n        .acquire_import(\n            app.clone(),\n            input.purpose.request(),\n            ExternalImportTarget {\n                path: partial_path.clone(),\n                maximum_bytes: input.purpose.maximum_bytes(),\n            },\n        )\n        .await;\n    let acquired = match acquired {\n        Ok(Some(value)) => value,\n        Ok(None) => {\n            remove_file_if_exists(&partial_path)?;\n            return Ok(None);\n        }\n        Err(error) => {\n            let _ = remove_file_if_exists(&partial_path);\n            return Err(error);\n        }\n    };\n    let display_name = validate_display_name(&acquired.display_name)?;\n    let (byte_size, sha256) =\n        hash_bounded_file(&partial_path, input.purpose.maximum_bytes())?;\n    if u64::try_from(byte_size).unwrap_or(u64::MAX) != acquired.copied_bytes {\n        remove_file_if_exists(&partial_path)?;\n        return Err(CommandError::new(\n            "external_file_copy_mismatch",\n            "The selected file changed while Android or the desktop file provider copied it.",\n        ));\n    }`,
  "stream selected import into managed partial file",
);
external = replaceExact(
  external,
  `    let request = ExternalFileRequest {\n        title: "Publish ostudy export".to_string(),\n        suggested_filename: Some(stored.dto.suggested_filename.clone()),\n        filters: vec![FileFilter {`,
  `    let request = ExternalFileRequest {\n        title: "Publish ostudy export".to_string(),\n        suggested_filename: Some(stored.dto.suggested_filename.clone()),\n        mime_types: vec![stored.dto.media_type.clone()],\n        filters: vec![FileFilter {`,
  "add export MIME type",
);
external = replaceExact(
  external,
  `    let Some(destination) = runtime\n        .external_files()\n        .select_export(app.clone(), request)\n        .await?\n    else {\n        return Ok(None);\n    };\n    let source = paths.resolve_relative(&stored.relative_path);\n    verify_file(&source, stored.dto.byte_size, &stored.dto.sha256)?;\n    publish_atomic(&source, &destination.path)?;\n    let completed_at = unix_time_millis()?;`,
  `    let source = paths.resolve_relative(&stored.relative_path);\n    verify_file(&source, stored.dto.byte_size, &stored.dto.sha256)?;\n    let Some(publication) = runtime\n        .external_files()\n        .publish_export(\n            app.clone(),\n            request,\n            ExternalExportSource {\n                path: source,\n                display_name: stored.dto.suggested_filename.clone(),\n                media_type: stored.dto.media_type.clone(),\n                maximum_bytes: u64::try_from(stored.dto.byte_size).unwrap_or(u64::MAX),\n            },\n        )\n        .await?\n    else {\n        return Ok(None);\n    };\n    if i64::try_from(publication.copied_bytes).unwrap_or(i64::MAX) != stored.dto.byte_size {\n        return Err(CommandError::new(\n            "external_export_copy_mismatch",\n            "The published export does not match the prepared artifact size.",\n        ));\n    }\n    let completed_at = unix_time_millis()?;`,
  "publish through platform adapter",
);
external = external.replaceAll("destination.display_name", "publication.display_name");
external = external.replaceAll('destination_kind: "desktop_file".to_string()', "destination_kind: publication.destination_kind");
external = replaceExact(
  external,
  `pub struct PublishReceiptDto {\n    pub prepared_export_id: String,\n    pub display_name: String,\n    pub destination_kind: String,\n    pub completed_at: i64,\n}`,
  `pub struct PublishReceiptDto {\n    pub prepared_export_id: String,\n    pub display_name: String,\n    pub destination_kind: String,\n    pub completed_at: i64,\n}\n\n#[derive(Clone, Debug, Serialize)]\n#[serde(rename_all = "camelCase")]\npub struct ShareLaunchDto {\n    pub prepared_export_id: String,\n    pub launched: bool,\n    pub destination_kind: String,\n    pub launched_at: i64,\n}`,
  "add share result DTO",
);
const shareCommand = `
#[tauri::command]
pub async fn share_prepared_export(
    app: AppHandle,
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: PublishPreparedExportInput,
) -> Result<ShareLaunchDto, CommandError> {
    validate_entity_id(&input.prepared_export_id, "prepared export")?;
    let stored = database.with_connection(|connection| {
        load_prepared_export(connection, paths.inner(), &input.prepared_export_id)
    })?;
    let source = paths.resolve_relative(&stored.relative_path);
    verify_file(&source, stored.dto.byte_size, &stored.dto.sha256)?;
    let result = runtime
        .system_share()
        .share_file(
            app,
            ShareSource {
                title: "Share ostudy export".to_string(),
                path: source,
                display_name: stored.dto.suggested_filename,
                media_type: stored.dto.media_type,
            },
        )
        .await?;
    Ok(ShareLaunchDto {
        prepared_export_id: input.prepared_export_id,
        launched: result.launched,
        destination_kind: result.destination_kind,
        launched_at: unix_time_millis()?,
    })
}

`;
external = replaceExact(
  external,
  `#[tauri::command]\npub fn release_prepared_export(`,
  `${shareCommand}#[tauri::command]\npub fn release_prepared_export(`,
  "add prepared export share command",
);
external = replaceBetween(
  external,
  `fn copy_bounded_with_hash(`,
  `fn hash_file(`,
  `fn hash_bounded_file(
    path: &Path,
    maximum_bytes: u64,
) -> Result<(i64, String), CommandError> {
    let metadata = fs::metadata(path).map_err(CommandError::io)?;
    if metadata.len() == 0 || metadata.len() > maximum_bytes {
        return Err(CommandError::new(
            "external_file_size_invalid",
            "The selected file is empty or exceeds the supported size limit.",
        ));
    }
    hash_file(path)
}

`,
  "replace copy-and-hash helper",
);
external = replaceBetween(
  external,
  `fn publish_atomic(`,
  `fn remove_unknown_transfer_files(`,
  ``,
  "remove desktop-only publication helper",
);
await write("src-tauri/src/external_files.rs", external);

// Register the private plugin and Android-aware runtime.
let lib = await read("src-tauri/src/lib.rs");
lib = replaceExact(lib, `mod util;`, `mod ui_checkpoint;\nmod util;`, "register UI checkpoint module");
lib = replaceExact(
  lib,
  `.plugin(tauri_plugin_dialog::init())`,
  `.plugin(tauri_plugin_ostudy_platform::init())\n        .plugin(tauri_plugin_dialog::init())`,
  "register private Android plugin",
);
lib = replaceExact(
  lib,
  `            let platform_runtime = platform::PlatformRuntime::current();`,
  `            let platform_runtime = platform::PlatformRuntime::for_app(app.handle().clone());`,
  "construct Android-aware platform runtime",
);
lib = replaceExact(
  lib,
  `            platform::get_platform_capabilities,`,
  `            platform::get_platform_capabilities,\n            platform::set_platform_back_interception,\n            ui_checkpoint::get_ui_checkpoint,\n            ui_checkpoint::save_ui_checkpoint,\n            ui_checkpoint::clear_ui_checkpoint,`,
  "register Android restoration commands",
);
lib = replaceExact(
  lib,
  `            external_files::publish_prepared_export,`,
  `            external_files::publish_prepared_export,\n            external_files::share_prepared_export,`,
  "register Android sharing command",
);
await write("src-tauri/src/lib.rs", lib);

// Android Gradle application configuration.
await write(
  "src-tauri/gen/android/app/build.gradle.kts",
  `import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) propFile.inputStream().use { load(it) }
}

android {
    compileSdk = 36
    namespace = "io.github.jferments.ostudy"

    defaultConfig {
        applicationId = "io.github.jferments.ostudy"
        minSdk = 28
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "0.1.0")
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        manifestPlaceholders["usesCleartextTraffic"] = "false"
    }

    buildTypes {
        getByName("debug") {
            applicationIdSuffix = ".dev"
            versionNameSuffix = "-dev"
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {
                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            isMinifyEnabled = true
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    buildFeatures { buildConfig = true }
    packaging { jniLibs.useLegacyPackaging = false }
    testOptions { animationsDisabled = true }
}

rust { rootDirRel = "../../../" }

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.9.2")

    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test:runner:1.6.2")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.6.1")
    androidTestImplementation("androidx.test.espresso:espresso-web:3.6.1")
    androidTestImplementation("androidx.test.espresso:espresso-intents:3.6.1")
    androidTestImplementation("androidx.test.uiautomator:uiautomator:2.3.0")
}

apply(from = "tauri.build.gradle.kts")
`,
);

await write(
  "src-tauri/gen/android/app/src/main/AndroidManifest.xml",
  `<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />

    <application
        android:allowBackup="false"
        android:dataExtractionRules="@xml/data_extraction_rules"
        android:fullBackupContent="@xml/backup_rules"
        android:icon="@mipmap/ic_launcher"
        android:label="@string/app_name"
        android:networkSecurityConfig="@xml/network_security_config"
        android:roundIcon="@mipmap/ic_launcher_round"
        android:supportsRtl="true"
        android:theme="@style/AppTheme"
        android:usesCleartextTraffic="\${usesCleartextTraffic}">

        <activity
            android:name=".MainActivity"
            android:configChanges="orientation|screenSize|smallestScreenSize|screenLayout|uiMode|keyboard|keyboardHidden|navigation|fontScale|density"
            android:exported="true"
            android:launchMode="singleTask"
            android:resizeableActivity="true">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
            <intent-filter>
                <action android:name="android.intent.action.SEND" />
                <category android:name="android.intent.category.DEFAULT" />
                <data android:mimeType="application/vnd.ostudy.studydeck" />
                <data android:mimeType="application/vnd.ostudy.backup" />
                <data android:mimeType="application/json" />
                <data android:mimeType="application/octet-stream" />
            </intent-filter>
        </activity>

        <provider
            android:name="androidx.core.content.FileProvider"
            android:authorities="\${applicationId}.fileprovider"
            android:exported="false"
            android:grantUriPermissions="true">
            <meta-data
                android:name="android.support.FILE_PROVIDER_PATHS"
                android:resource="@xml/file_paths" />
        </provider>
    </application>
</manifest>
`,
);
await write(
  "src-tauri/gen/android/app/src/main/res/xml/file_paths.xml",
  `<?xml version="1.0" encoding="utf-8"?>\n<paths xmlns:android="http://schemas.android.com/apk/res/android">\n    <files-path name="prepared_exports" path="transfers/exports/" />\n</paths>\n`,
);
await write(
  "src-tauri/gen/android/app/src/main/res/xml/network_security_config.xml",
  `<?xml version="1.0" encoding="utf-8"?>\n<network-security-config>\n    <base-config cleartextTrafficPermitted="false">\n        <trust-anchors><certificates src="system" /></trust-anchors>\n    </base-config>\n    <debug-overrides>\n        <trust-anchors><certificates src="user" /><certificates src="system" /></trust-anchors>\n    </debug-overrides>\n</network-security-config>\n`,
);
await write(
  "src-tauri/gen/android/app/src/main/res/xml/backup_rules.xml",
  `<?xml version="1.0" encoding="utf-8"?>\n<full-backup-content>\n    <exclude domain="root" path="." />\n    <exclude domain="file" path="." />\n    <exclude domain="database" path="." />\n    <exclude domain="sharedpref" path="." />\n    <exclude domain="external" path="." />\n</full-backup-content>\n`,
);
await write(
  "src-tauri/gen/android/app/src/main/res/xml/data_extraction_rules.xml",
  `<?xml version="1.0" encoding="utf-8"?>\n<data-extraction-rules>\n    <cloud-backup disableIfNoEncryptionCapabilities="true">\n        <exclude domain="root" path="." />\n        <exclude domain="file" path="." />\n        <exclude domain="database" path="." />\n        <exclude domain="sharedpref" path="." />\n        <exclude domain="external" path="." />\n    </cloud-backup>\n    <device-transfer>\n        <exclude domain="root" path="." />\n        <exclude domain="file" path="." />\n        <exclude domain="database" path="." />\n        <exclude domain="sharedpref" path="." />\n        <exclude domain="external" path="." />\n    </device-transfer>\n</data-extraction-rules>\n`,
);

// Correct Android FileProvider authority and foreground lifecycle emission.
let kotlin = await read(
  "plugins/ostudy-platform/android/src/main/java/io/github/jferments/ostudy/platform/OstudyPlatformPlugin.kt",
);
kotlin = kotlin.replaceAll('"${activity.packageName}.files"', '"${activity.packageName}.fileprovider"');
kotlin = replaceExact(
  kotlin,
  `    override fun onResume(owner: LifecycleOwner) {\n        trigger("lifecycle", JSObject().put("phase", "resuming"))\n        publishEnvironment()\n    }`,
  `    override fun onResume(owner: LifecycleOwner) {\n        trigger("lifecycle", JSObject().put("phase", "resuming"))\n        trigger("lifecycle", JSObject().put("phase", "foreground"))\n        publishEnvironment()\n    }`,
  "emit Android foreground lifecycle",
);
await write(
  "plugins/ostudy-platform/android/src/main/java/io/github/jferments/ostudy/platform/OstudyPlatformPlugin.kt",
  kotlin,
);

console.log("Applied Android A1-A3 core runtime, transfer, sharing, and build integration.");
