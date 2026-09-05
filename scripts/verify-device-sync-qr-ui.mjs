#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const [
  nativeScanner,
  qrCommand,
  frontendIpc,
  compositionRoot,
  startup,
  settingsUi,
  settingsCss,
  syncCore,
  syncCancellation,
  syncNetwork,
  syncTransport,
  syncPackage,
  syncScope,
  syncMerge,
  aiProfileRepository,
  aiDefaults,
  aiProbeRepository,
  logging,
  referenceTokens,
] = await Promise.all([
  read("plugins/ostudy-platform/android/src/main/java/OstudyQrPlugin.kt"),
  read("src-tauri/src/device_sync_qr_commands.rs"),
  read("src/ipc/deviceSync.ts"),
  read("src-tauri/src/lib.rs"),
  read("src-tauri/src/startup.rs"),
  read("src/screens/settings/DeviceSyncSettings.tsx"),
  read("src/screens/settings/DeviceSyncSettings.module.css"),
  read("src-tauri/src/device_sync/mod.rs"),
  read("src-tauri/src/device_sync/cancellation.rs"),
  read("src-tauri/src/device_sync/network.rs"),
  read("src-tauri/src/device_sync/transport.rs"),
  read("src-tauri/src/device_sync/package.rs"),
  read("src-tauri/src/device_sync/scope.rs"),
  read("src-tauri/src/device_sync/merge.rs"),
  read("src-tauri/src/ai/profiles/repository.rs"),
  read("src-tauri/src/ai/profiles/defaults.rs"),
  read("src-tauri/src/ai/probes/repository.rs"),
  read("src-tauri/src/logging.rs"),
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
expectContains(frontendIpc, "selection: input.selection", "granular selection IPC projection");
expectNotContains(frontendIpc, "includeCredentials", "removed receiver credential consent field");
expectContains(
  compositionRoot,
  "device_sync_qr_commands::scan_device_pairing_qr_async",
  "async QR command registration",
);
expectContains(
  compositionRoot,
  "device_sync::cancel_device_sync_session",
  "sync cancellation command registration",
);
expectContains(compositionRoot, ".setup(startup::setup)", "startup composition boundary registration");
expectContains(startup, "device_sync::reconcile_startup(&paths)", "stale-session startup reconciliation");
expectContains(startup, "logging::initialize(paths.app_data_dir())", "persistent diagnostic initialization");
expectContains(settingsUi, 'role="status"', "accessible scanner result status");
expectContains(settingsUi, "Sync offer detected", "visual offer detection result");
expectContains(settingsUi, "await receiveOffer(scanned, true)", "automatic receive after QR detection");
expectContains(settingsUi, "Accept sync offer", "text-invitation acceptance action");
expectContains(settingsUi, "Retry connection", "failed automatic scan retry action");
expectContains(settingsUi, "connection did not complete", "failed automatic scan explanation");
expectContains(settingsUi, "Replace waiting offer", "single unclaimed outgoing offer behavior");
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
expectContains(settingsUi, "entire white border visible", "host-side QR framing guidance");
expectContains(settingsUi, '<option value="usb-network">USB cable</option>', "explicit USB transport choice");
expectContains(settingsUi, "USB interface address", "USB endpoint fallback field");
expectContains(settingsUi, "startDevicePairing({", "shared pairing command for LAN and USB offers");
expectContains(settingsUi, "same authenticated synchronization protocol", "USB protocol-reuse explanation");
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
expectContains(syncCore, "const PROTOCOL_VERSION: u32 = 2;", "one-way offer protocol version");
expectContains(syncCore, "network::bind_listener()", "stable LAN listener allocation");
expectContains(syncCore, "network::connect_to_sender", "actionable LAN connection boundary");
expectContains(syncCore, "cancel_superseded_unclaimed_hosts", "replacement of stale unclaimed sender offers");
expectContains(syncCore, "cancellation::claim_peer", "same-peer active-session claim enforcement");
expectContains(syncCore, "sent_package_bytes: 0", "receiver does not upload a package during offer acceptance");
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
  "OK   Device-sync QR uses permissionless Google Code Scanner with bounded nonblocking delivery; LAN and USB-network offers share authenticated pairing while secure framing and cancellation stay transport-neutral; sender checkboxes select Flashcards, Documents/LLM processing, Study Guides, API settings, and credential scopes; AI transfer sanitization stays inside AI-owned repositories; document files reuse the validated backup ledger; local app settings stay off the wire/merge allowlist; diagnostics persist through a bounded asynchronous writer.",
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
