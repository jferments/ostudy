#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const [
  nativeScanner,
  nativeUsb,
  nativeManifest,
  qrCommand,
  frontendIpc,
  platformIpc,
  usbCoordinator,
  compositionRoot,
  startup,
  settingsUi,
  settingsCss,
  syncCore,
  syncCancellation,
  syncNetwork,
  syncTransport,
  syncUsb,
  syncPackage,
  syncScope,
  syncMerge,
  aiProfileRepository,
  aiDefaults,
  aiProbeRepository,
  logging,
  linuxSetup,
  linuxBundle,
  udevRules,
  accessoryFilter,
  referenceTokens,
] = await Promise.all([
  read("plugins/ostudy-platform/android/src/main/java/OstudyQrPlugin.kt"),
  read("plugins/ostudy-platform/android/src/main/java/OstudyUsbPlugin.kt"),
  read("plugins/ostudy-platform/android/src/main/AndroidManifest.xml"),
  read("src-tauri/src/device_sync_qr_commands.rs"),
  read("src/ipc/deviceSync.ts"),
  read("src/ipc/platform.ts"),
  read("src/components/UsbSyncCoordinator.tsx"),
  read("src-tauri/src/lib.rs"),
  read("src-tauri/src/startup.rs"),
  read("src/screens/settings/DeviceSyncSettings.tsx"),
  read("src/screens/settings/DeviceSyncSettings.module.css"),
  read("src-tauri/src/device_sync/mod.rs"),
  read("src-tauri/src/device_sync/cancellation.rs"),
  read("src-tauri/src/device_sync/network.rs"),
  read("src-tauri/src/device_sync/transport.rs"),
  read("src-tauri/src/device_sync/usb.rs"),
  read("src-tauri/src/device_sync/package.rs"),
  read("src-tauri/src/device_sync/scope.rs"),
  read("src-tauri/src/device_sync/merge.rs"),
  read("src-tauri/src/ai/profiles/repository.rs"),
  read("src-tauri/src/ai/profiles/defaults.rs"),
  read("src-tauri/src/ai/probes/repository.rs"),
  read("src-tauri/src/logging.rs"),
  read("scripts/setup-ubuntu.sh"),
  read("src-tauri/tauri.linux.conf.json"),
  read("src-tauri/packaging/linux/70-ostudy-android-usb.rules"),
  read("plugins/ostudy-platform/android/src/main/res/xml/ostudy_usb_accessory_filter.xml"),
  read("src/ui/tokens/source/reference.tokens.json"),
]);
const failures = [];
const referenceTokenData = JSON.parse(referenceTokens);
const syncTransportProduction = syncTransport.split("\n#[cfg(test)]", 1)[0];

expectContains(nativeScanner, "GmsBarcodeScanning.getClient(activity, options)", "Google Code Scanner client");
expectContains(nativeScanner, ".setBarcodeFormats(Barcode.FORMAT_QR_CODE)", "QR-only native scan format");
expectContains(nativeScanner, ".enableAutoZoom()", "first-party native scan auto zoom");
expectContains(nativeScanner, "AtomicBoolean", "concurrent and duplicate scanner completion guard");
expectContains(nativeScanner, ".addOnCanceledListener", "native scanner cancellation path");
expectContains(nativeScanner, "MAX_QR_CONTENT_CHARS", "bounded decoded QR payload");
expectNotContains(nativeScanner, "IntentIntegrator", "removed ZXing Embedded scanner");
expectNotContains(nativeScanner, "parsed.contents", "removed ZXing activity-result parser");
expectContains(qrCommand, "pub async fn scan_device_pairing_qr_async", "async QR command adapter");
expectContains(qrCommand, "tauri::async_runtime::spawn_blocking", "QR mobile-plugin worker handoff");
expectContains(frontendIpc, 'invoke<string | null>("scan_device_pairing_qr_async")', "frontend async QR invocation");
expectContains(frontendIpc, 'invoke<void>("cancel_device_sync_session"', "frontend session cancellation invocation");
expectContains(frontendIpc, "export interface DeviceSyncSelection", "typed granular sender selection");
expectContains(frontendIpc, 'transport: input.transport ?? "local-network"', "explicit transport IPC projection");
expectContains(frontendIpc, "joinUsbDevicePairing", "automatic attached-USB receive IPC");
expectContains(frontendIpc, "ostudy-sync://usb-attached", "internal USB attachment sentinel");
expectNotContains(frontendIpc, "includeCredentials", "removed receiver credential consent field");
expectContains(
  compositionRoot,
  "device_sync_qr_commands::scan_device_pairing_qr_async",
  "async QR command registration",
);
expectContains(compositionRoot, "tauri_plugin_ostudy_platform::init_usb()", "dedicated native USB plugin registration");
expectContains(
  compositionRoot,
  "device_sync::cancel_device_sync_session",
  "sync cancellation command registration",
);
expectContains(compositionRoot, ".setup(startup::setup)", "startup composition boundary registration");
expectContains(startup, "device_sync::reconcile_startup(&paths)", "stale-session startup reconciliation");
expectContains(startup, "logging::initialize(paths.app_data_dir())", "persistent diagnostic initialization");

expectContains(nativeUsb, "UsbManager.ACTION_USB_ACCESSORY_ATTACHED", "Android Open Accessory attachment handling");
expectContains(nativeUsb, "manager.openAccessory(accessory)", "Android accessory descriptor open");
expectContains(nativeUsb, "descriptor.detachFd()", "raw accessory descriptor handoff to Rust");
expectContains(nativeUsb, 'trigger("usb-sync-ready"', "native USB-ready event");
expectContains(nativeUsb, 'fun takeUsbAccessory', "native one-shot accessory descriptor claim");
expectContains(nativeManifest, "android.hardware.usb.action.USB_ACCESSORY_ATTACHED", "Android accessory intent filter");
expectContains(nativeManifest, "@xml/ostudy_usb_accessory_filter", "Android accessory identity filter");
expectContains(accessoryFilter, 'manufacturer="oStudy"', "oStudy accessory manufacturer identity");
expectContains(accessoryFilter, 'model="oStudy USB Sync"', "oStudy accessory model identity");
expectContains(platformIpc, 'const USB_PLUGIN = "ostudy-usb"', "separate USB plugin event namespace");
expectContains(platformIpc, '"usb-sync-ready"', "frontend USB attachment event");
expectContains(usbCoordinator, "joinUsbDevicePairing()", "automatic Android USB receive coordinator");
expectContains(usbCoordinator, "listenToAndroidUsbSync", "USB attachment event coordinator");

expectContains(settingsUi, 'role="status"', "accessible scanner result status");
expectContains(settingsUi, "Sync offer detected", "visual offer detection result");
expectContains(settingsUi, "await receiveOffer(scanned, true)", "automatic receive after QR detection");
expectContains(settingsUi, "Accept sync offer", "text-invitation acceptance action");
expectContains(settingsUi, "Retry connection", "failed automatic scan retry action");
expectContains(settingsUi, "connection did not complete", "failed automatic scan explanation");
expectContains(settingsUi, "Replace with network offer", "single unclaimed outgoing network offer behavior");
expectContains(settingsUi, 'aria-label={`Cancel ${session.direction', "active-session cancellation affordance");
expectContains(settingsUi, "What to synchronize", "sender category selection group");
expectContains(settingsUi, "Flashcards", "Flashcards scope checkbox");
expectContains(settingsUi, "Documents", "Documents scope checkbox");
expectContains(settingsUi, "Study Guides", "Study Guides scope checkbox");
expectContains(settingsUi, "API provider settings", "API provider settings scope checkbox");
expectContains(settingsUi, "Saved API keys", "saved API credential scope checkbox");
expectContains(settingsUi, 'changeSelection("documents"', "Documents dependency handler");
expectContains(settingsUi, 'changeSelection("studyGuides"', "Study Guide dependency handler");
expectContains(settingsUi, 'changeSelection("apiCredentials"', "API credential dependency handler");
expectContainsNormalized(settingsUi, "app settings always stay local", "device-local app-settings explanation");
expectContains(settingsUi, "entire white border visible", "LAN QR framing guidance");
expectContains(settingsUi, 'onClick={() => void startUsbSync()}', "one-click direct USB action");
expectContains(settingsUi, '"USB Sync"', "direct USB action label");
expectContains(settingsUi, 'transport: "usb"', "USB transport selection without address input");
expectContains(settingsUi, "no tethering, IP address, QR code", "direct USB user-flow explanation");
expectNotContains(settingsUi, "USB interface address", "manual USB endpoint field");
expectNotContains(settingsUi, 'value="usb-network"', "USB-network transport selector");
expectNotContains(settingsUi, "USB tethering", "manual USB tethering instruction");
expectContains(settingsCss, "width: min(100%, 340px);", "large desktop pairing QR presentation");
expectContains(settingsCss, ".scopeFieldset", "granular sync scope presentation");
expectContains(
  settingsCss,
  "background: var(--ostudy-color-neutral-0);",
  "token-owned high-contrast QR quiet-zone background",
);
if (referenceTokenData?.color?.neutral?.["0"]?.$value !== "#ffffff") {
  failures.push("Reference token color.neutral.0 must remain literal #ffffff for QR quiet-zone contrast.");
}
expectContains(settingsCss, '.scanStatus[data-state="receiving"]', "visible receiving-state treatment");
expectContains(settingsCss, '.scanStatus[data-state="received"]', "visible completed-state treatment");
expectContains(settingsCss, ".sessionPill", "active-session cancellation presentation");

expectContains(syncCore, "const PROTOCOL_VERSION: u32 = 3;", "transport-bound offer protocol version");
expectContains(syncCore, "PairingTransport", "typed LAN/direct-USB invitation transport");
expectContains(syncCore, "UsbSyncBootstrap", "USB cable invitation bootstrap");
expectContains(syncCore, "run_host_usb", "direct USB sender adapter");
expectContains(syncCore, "run_join_session_with_stream", "shared join protocol over generic streams");
expectContains(syncCore, "handle_host_connection<S>", "shared sender protocol over generic streams");
expectContains(syncCore, "network::bind_listener()", "stable LAN listener allocation");
expectContains(syncCore, "network::connect_to_sender", "actionable LAN connection boundary");
expectContains(syncCore, "cancel_superseded_unclaimed_hosts", "replacement of stale unclaimed sender offers");
expectContains(syncCore, "cancellation::claim_peer", "same-peer active-session claim enforcement");
expectContains(syncCore, "sent_package_bytes: 0", "receiver does not upload a package during offer acceptance");
expectContains(syncCore, "usb_host_available: usb::host_available()", "platform USB-host capability projection");
expectContains(syncCore, 'endpoint != usb::USB_ENDPOINT_HINT', "USB invitation endpoint confinement");
expectContains(syncCore, "transport: invitation.transport", "transport binding in authenticated transcript");

expectContains(syncTransportProduction, "pub(crate) trait SyncIo", "transport-neutral synchronization byte-stream boundary");
expectNotContains(syncTransportProduction, "TcpStream", "TCP ownership removed from production secure framing");
expectContains(syncCancellation, "device_sync_peer_busy", "same-peer overlap rejection");
expectContains(syncCancellation, "trait SyncAbort", "transport-neutral cancellation capability");
expectContains(syncCancellation, "abort.abort()", "active transport interruption dispatch on cancellation");
expectNotContains(syncCancellation, "TcpStream", "TCP ownership removed from cancellation registry");
expectContains(syncCancellation, "device_sync_process_interrupted", "restart reconciliation for stale sessions");
expectContains(syncCancellation, "CANCELLABLE_STATES_SQL", "non-cancelable atomic merge boundary");
expectContains(syncNetwork, "impl SyncIo for TcpStream", "TCP implementation of the generic sync stream");
expectContains(syncNetwork, "Shutdown::Both", "LAN adapter interrupts active TCP streams");
expectContains(syncNetwork, "LISTENER_PORT_START: u16 = 49_251", "stable LAN port range start");
expectContains(syncNetwork, "LISTENER_PORT_END: u16 = 49_258", "stable LAN port range end");
expectContains(syncNetwork, "TCP path is blocked or unreachable", "network-specific connection failure guidance");
expectContains(syncNetwork, "ports {LISTENER_PORT_START}-{LISTENER_PORT_END}", "firewall port guidance");

expectContains(syncUsb, "const AOA_GET_PROTOCOL: u8 = 51", "AOA protocol discovery request");
expectContains(syncUsb, "const AOA_SEND_STRING: u8 = 52", "AOA accessory identity request");
expectContains(syncUsb, "const AOA_START: u8 = 53", "AOA accessory-mode start request");
expectContains(syncUsb, "GOOGLE_VENDOR_ID: u16 = 0x18d1", "AOA re-enumerated Google VID");
expectContains(syncUsb, "AOA_PRODUCT_ID: u16 = 0x2d00", "AOA accessory PID");
expectContains(syncUsb, "AOA_ADB_PRODUCT_ID: u16 = 0x2d01", "AOA accessory plus ADB PID compatibility");
expectContains(syncUsb, "impl SyncIo for HostUsbStream", "Linux libusb stream implements shared sync transport");
expectContains(syncUsb, "impl SyncIo for AndroidUsbStream", "Android accessory descriptor implements shared sync transport");
expectContains(syncUsb, "libusb_bulk_transfer", "direct Linux USB bulk transfer");
expectContains(syncUsb, "c_poll", "cancel-aware Android accessory descriptor polling");
expectNotContains(syncUsb, "TcpStream", "direct USB adapter does not tunnel over TCP");

expectContains(linuxSetup, "libusb-1.0-0-dev", "Linux native libusb build dependency");
expectContains(linuxSetup, "70-ostudy-android-usb.rules", "developer setup USB rule installation");
expectContains(linuxBundle, '"libusb-1.0-0"', "Debian runtime libusb dependency");
expectContains(linuxBundle, '"/usr/lib/udev/rules.d/70-ostudy-android-usb.rules"', "Debian oStudy USB udev rule");
expectContains(udevRules, 'ATTR{idVendor}=="18d1"', "Google/Pixel USB vendor rule");
expectContains(udevRules, 'ATTR{idVendor}=="04e8"', "Samsung USB vendor rule");
expectContains(udevRules, 'ATTR{idVendor}=="2a70"', "OnePlus USB vendor rule");
expectContains(udevRules, 'ATTR{idProduct}=="2d00"', "AOA udev accessory product rule");
expectContains(udevRules, 'ATTR{idProduct}=="2d01"', "AOA udev accessory+ADB product rule");
expectNotContains(linuxBundle, "android-sdk-platform-tools-common", "ADB/developer package runtime dependency");

expectContains(syncScope, "DELETE FROM app_settings;", "app settings removed before package transmission");
expectContains(syncScope, "crate::ai::profiles::sanitize_for_device_transfer", "AI profile transfer policy delegated to AI ownership");
expectContains(syncScope, "crate::ai::probes::sanitize_for_device_transfer", "AI probe transfer policy delegated to AI ownership");
expectContains(aiDefaults, "DELETE FROM ai_task_defaults", "API task defaults removed by their owning module when scope is off");
expectContains(aiProfileRepository, "DELETE FROM ai_provider_profiles", "API profiles removed by their owning module when scope is off");
expectContains(aiProfileRepository, "DELETE FROM ai_profile_operations", "profile operation cache excluded from transfer");
expectContains(aiProbeRepository, "DELETE FROM ai_provider_probe_results", "probe evidence excluded from transfer");
expectContains(aiProbeRepository, "DELETE FROM ai_provider_capabilities", "capability evidence excluded from transfer");
expectContains(syncScope, '"documents"', "Documents synchronization scope");
expectContains(syncScope, '"document_study_guides"', "Study Guide synchronization scope");
expectContains(syncScope, "api_credentials", "separate saved-credential scope");
expectContains(syncPackage, "const PACKAGE_VERSION: i64 = 3;", "granular sync package version");
expectContains(syncPackage, "document_backup::export_document_files", "document managed-file export reuse");
expectContains(syncPackage, "document_backup::validate_document_files", "document managed-file validation reuse");
expectNotContains(syncMerge, 'TableSpec::new("app_settings"', "app settings in merge allowlist");
expectContains(syncMerge, 'TableSpec::new("documents"', "Documents merge graph");
expectContains(syncMerge, 'TableSpec::new("document_study_guides"', "Study Guide merge graph");
expectContains(
  syncMerge,
  "first_contact_remote_creation_is_not_a_delete_conflict",
  "first-contact sender rows merge without synthetic delete conflicts",
);
expectContains(logging, 'DIAGNOSTIC_FILE: &str = "ostudy.jsonl"', "persistent diagnostic log");
expectContains(logging, "WRITER_QUEUE_CAPACITY", "bounded asynchronous diagnostic queue");
expectContains(logging, "sender.try_send(WriterMessage::Event", "nonblocking diagnostic write path");
expectContains(logging, "MAX_DIAGNOSTIC_BYTES", "bounded diagnostic rotation");
expectContains(logging, "sensitive_key", "central diagnostic redaction policy");
expectNotContains(logging, 'cfg(target_os = "android")', "platform-specific branch in logging core");

if (failures.length > 0) {
  console.error("Device-sync QR/offer contract failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log(
  "OK   Device sync keeps LAN QR pairing and direct Android USB as separate adapters beneath one authenticated SyncIo protocol; USB uses Android Open Accessory bulk I/O with automatic attachment, no tethering/IP/QR/debugging workflow; sender-selected scopes, merge/privacy rules, and credential boundaries remain shared.",
);

async function read(relativePath) {
  return readFile(join(root, relativePath), "utf8");
}

function expectContains(source, expected, label) {
  if (!source.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}

function expectContainsNormalized(source, expected, label) {
  const normalized = source.replace(/\s+/gu, " ");
  if (!normalized.includes(expected)) failures.push(`Missing ${label}: ${expected}`);
}

function expectNotContains(source, forbidden, label) {
  if (source.includes(forbidden)) failures.push(`Forbidden ${label}: ${forbidden}`);
}
