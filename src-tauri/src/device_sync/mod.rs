//! Explicit trusted-device synchronization over the local network.
//!
//! A pairing invitation is a short-lived one-way offer created by the device
//! that will SEND its study library. The receiving device scans or pastes that
//! offer, authenticates the sender, and merges the offered data locally. A
//! reverse transfer is a separate explicit offer; accepting one invitation
//! never uploads the recipient's library back to the sender.
//!
//! General application/appearance settings are device-local and never belong in
//! a synchronization package. AI provider configuration and secure credentials
//! are an independent sender-controlled opt-in scope.

mod cancellation;
mod credentials;
mod crypto;
mod merge;
mod package;
mod store;
mod transport;

use std::{
    fs,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream, UdpSocket},
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::{
    database::Database,
    error::CommandError,
    paths::AppPaths,
    platform::{CapabilityState, PlatformRuntime},
    util::{unix_time_millis, validate_entity_id},
};

use self::{
    credentials::CredentialRecord,
    crypto::{EphemeralKey, IdentityKey, SessionRole},
    package::PackageSummary,
    store::{ConflictRecord, DeviceIdentityRecord, PeerRecord, SessionRecord, SyncStore},
    transport::{read_plain_json, write_plain_json, SecureChannel, FRAME_COMPLETE, FRAME_METADATA},
};

const PROTOCOL_VERSION: u32 = 2;
const INVITATION_PREFIX: &str = "ostudy-sync://pair/";
const DEFAULT_EXPIRY_SECONDS: i64 = 300;
const MIN_EXPIRY_SECONDS: i64 = 60;
const MAX_EXPIRY_SECONDS: i64 = 900;
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_INVITATION_CHARS: usize = 8 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartDevicePairingInput {
    pub host_override: Option<String>,
    pub include_api_settings: bool,
    pub expires_in_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JoinDevicePairingInput {
    pub invitation: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelDeviceSyncSessionInput {
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateDeviceSyncNameInput {
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveDeviceSyncPeerInput {
    pub peer_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveDeviceSyncConflictInput {
    pub conflict_id: String,
    pub resolution: String,
    pub merged_payload_json: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSyncStateDto {
    pub identity: DeviceIdentityRecord,
    pub fingerprint: String,
    pub peers: Vec<PeerRecord>,
    pub conflicts: Vec<ConflictRecord>,
    pub sessions: Vec<SessionRecord>,
    pub local_network_available: bool,
    pub qr_scan_available: bool,
    pub secure_credentials_available: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingInvitationDto {
    pub session_id: String,
    pub invitation: String,
    pub qr_svg: String,
    pub endpoint: String,
    pub fingerprint: String,
    pub expires_at: i64,
    pub include_api_settings: bool,
    pub credentials_available: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSyncResultDto {
    pub session_id: String,
    pub peer: PeerRecord,
    pub changed_row_count: usize,
    pub conflict_count: usize,
    pub transferred_credential_count: usize,
    pub sent_package_bytes: u64,
    pub received_package_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PairingInvitation {
    protocol_version: u32,
    session_id: String,
    host_device_id: String,
    host_display_name: String,
    host_identity_public_key: String,
    host_identity_fingerprint: String,
    host_ephemeral_public_key: String,
    endpoint: String,
    pairing_secret: String,
    expires_at: i64,
    include_api_settings: bool,
    credentials_available: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClientHello {
    protocol_version: u32,
    session_id: String,
    device_id: String,
    display_name: String,
    identity_public_key: String,
    ephemeral_public_key: String,
    secure_credentials_available: bool,
    pairing_proof: String,
    identity_signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UnsignedClientHello<'a> {
    protocol_version: u32,
    session_id: &'a str,
    host_device_id: &'a str,
    host_identity_public_key: &'a str,
    host_ephemeral_public_key: &'a str,
    endpoint: &'a str,
    expires_at: i64,
    include_api_settings: bool,
    credentials_available: bool,
    device_id: &'a str,
    display_name: &'a str,
    identity_public_key: &'a str,
    ephemeral_public_key: &'a str,
    secure_credentials_available: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ServerHello {
    protocol_version: u32,
    session_id: String,
    device_id: String,
    display_name: String,
    identity_public_key: String,
    identity_fingerprint: String,
    include_api_settings: bool,
    credentials_included: bool,
    identity_signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UnsignedServerHello<'a> {
    protocol_version: u32,
    session_id: &'a str,
    device_id: &'a str,
    display_name: &'a str,
    identity_public_key: &'a str,
    identity_fingerprint: &'a str,
    include_api_settings: bool,
    credentials_included: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TransferMetadata {
    protocol_version: u32,
    session_id: String,
    device_id: String,
    display_name: String,
    identity_public_key: String,
    include_api_settings: bool,
    credentials: Vec<CredentialRecord>,
    package: PackageSummary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReceiverAcknowledgement {
    protocol_version: u32,
    session_id: String,
    changed_row_count: usize,
    conflict_count: usize,
    transferred_credential_count: usize,
}

pub(crate) fn reconcile_startup(paths: &AppPaths) -> Result<(), CommandError> {
    let _store = SyncStore::open(paths)?;
    cancellation::reconcile_startup(paths)
}

#[tauri::command]
pub fn get_device_sync_state(
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
) -> Result<DeviceSyncStateDto, CommandError> {
    let store = SyncStore::open(paths.inner())?;
    let (identity, identity_key) = ensure_identity(&store, runtime.inner())?;
    // PR #51 briefly allowed app_settings into the merge set. Preserve the
    // local value and retire those obsolete conflicts on first state load.
    for conflict in store
        .list_conflicts()?
        .into_iter()
        .filter(|conflict| conflict.table_name == "app_settings")
    {
        store.resolve_conflict(&conflict.id, "keep_left")?;
    }
    let capabilities = runtime.capabilities();
    Ok(DeviceSyncStateDto {
        identity,
        fingerprint: identity_key.fingerprint(),
        peers: store.list_peers()?,
        conflicts: store.list_conflicts()?,
        sessions: store.list_sessions()?,
        local_network_available: matches!(
            capabilities.services.local_network.state,
            CapabilityState::Available
        ),
        qr_scan_available: matches!(
            capabilities.services.qr_scan.state,
            CapabilityState::Available | CapabilityState::PermissionRequired
        ),
        secure_credentials_available: matches!(
            capabilities.services.secure_secrets.state,
            CapabilityState::Available
        ),
    })
}

#[tauri::command]
pub fn update_device_sync_name(
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: UpdateDeviceSyncNameInput,
) -> Result<DeviceIdentityRecord, CommandError> {
    let store = SyncStore::open(paths.inner())?;
    ensure_identity(&store, runtime.inner())?;
    store.set_identity_name(&input.display_name)
}

#[tauri::command]
pub fn start_device_pairing(
    app: AppHandle,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: StartDevicePairingInput,
) -> Result<PairingInvitationDto, CommandError> {
    let store = SyncStore::open(paths.inner())?;
    let (identity, identity_key) = ensure_identity(&store, runtime.inner())?;
    for superseded in cancellation::cancel_superseded_unclaimed_hosts(paths.inner())? {
        crate::logging::info(
            "device_sync_offer_superseded",
            format!("session={superseded} reason=newer_unclaimed_offer"),
        );
    }
    let expiry_seconds = input
        .expires_in_seconds
        .unwrap_or(DEFAULT_EXPIRY_SECONDS)
        .clamp(MIN_EXPIRY_SECONDS, MAX_EXPIRY_SECONDS);
    let expires_at = unix_time_millis()?.saturating_add(expiry_seconds.saturating_mul(1_000));
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(|error| {
        CommandError::new(
            "device_sync_listener_failed",
            format!("ostudy could not open a local synchronization listener: {error}"),
        )
    })?;
    listener.set_nonblocking(true).map_err(CommandError::io)?;
    let port = listener.local_addr().map_err(CommandError::io)?.port();
    let host_ip = resolve_host_ip(input.host_override.as_deref())?;
    let endpoint = SocketAddr::new(host_ip, port).to_string();
    let pairing_secret = crypto::random_pairing_secret()?;
    let ephemeral = EphemeralKey::generate()?;
    let session_id = random_id(&pairing_secret[..16]);
    let credentials_available = input.include_api_settings
        && matches!(
            runtime.capabilities().services.secure_secrets.state,
            CapabilityState::Available
        );
    let invitation_payload = PairingInvitation {
        protocol_version: PROTOCOL_VERSION,
        session_id: session_id.clone(),
        host_device_id: identity.device_id.clone(),
        host_display_name: identity.display_name.clone(),
        host_identity_public_key: identity.public_key_base64.clone(),
        host_identity_fingerprint: identity_key.fingerprint(),
        host_ephemeral_public_key: BASE64_STANDARD.encode(&ephemeral.public_key),
        endpoint: endpoint.clone(),
        pairing_secret: BASE64_STANDARD.encode(pairing_secret),
        expires_at,
        include_api_settings: input.include_api_settings,
        credentials_available,
    };
    let invitation_json =
        serde_json::to_vec(&invitation_payload).map_err(CommandError::serialization)?;
    let invitation = format!(
        "{INVITATION_PREFIX}{}",
        crypto::encode_url(&invitation_json)
    );
    if invitation.len() > MAX_INVITATION_CHARS {
        return Err(CommandError::new(
            "device_sync_invitation_too_large",
            "The synchronization invitation exceeds the supported QR-code size.",
        ));
    }
    let qr_svg = render_pairing_qr(&invitation)?;
    store.create_session(&session_id, "host", &endpoint, expires_at)?;
    let cancel_signal = cancellation::begin(&session_id)?;
    crate::logging::info(
        "device_sync_offer_created",
        format!(
            "session={session_id} endpoint={endpoint} api_scope={}",
            input.include_api_settings
        ),
    );
    let host_app = app.clone();
    let host_session = session_id.clone();
    let worker_signal = cancel_signal.clone();
    let spawn_result = thread::Builder::new()
        .name(format!("ostudy-sync-send-{}", &session_id[..8]))
        .spawn(move || {
            let result = cancellation::normalize_result(
                &worker_signal,
                run_host_listener(
                    host_app.clone(),
                    listener,
                    invitation_payload,
                    pairing_secret,
                    ephemeral,
                    worker_signal.clone(),
                ),
            );
            let paths = host_app.state::<AppPaths>();
            match &result {
                Ok(peer_id) => {
                    let _ = cancellation::finish_terminal(
                        paths.inner(),
                        &host_session,
                        Some(peer_id),
                        "completed",
                        None,
                    );
                }
                Err(error) if error.code == "device_sync_invitation_expired" => {
                    let _ = cancellation::finish_terminal(
                        paths.inner(),
                        &host_session,
                        None,
                        "expired",
                        Some((&error.code, &error.message)),
                    );
                }
                Err(error) if error.code == "device_sync_canceled" => {
                    let _ = cancellation::finish_terminal(
                        paths.inner(),
                        &host_session,
                        None,
                        "canceled",
                        Some((&error.code, &error.message)),
                    );
                }
                Err(error) => {
                    let _ = cancellation::finish_terminal(
                        paths.inner(),
                        &host_session,
                        None,
                        "failed",
                        Some((&error.code, &error.message)),
                    );
                }
            }
            cancellation::finish(&host_session);
        });
    if let Err(error) = spawn_result {
        cancellation::finish(&session_id);
        let command_error = CommandError::new(
            "device_sync_listener_failed",
            format!("The local synchronization worker could not start: {error}"),
        );
        let _ = cancellation::finish_terminal(
            paths.inner(),
            &session_id,
            None,
            "failed",
            Some((&command_error.code, &command_error.message)),
        );
        return Err(command_error);
    }

    Ok(PairingInvitationDto {
        session_id,
        invitation,
        qr_svg,
        endpoint,
        fingerprint: identity_key.fingerprint(),
        expires_at,
        include_api_settings: input.include_api_settings,
        credentials_available,
    })
}

#[tauri::command]
pub async fn join_device_pairing(
    app: AppHandle,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: JoinDevicePairingInput,
) -> Result<DeviceSyncResultDto, CommandError> {
    let invitation = parse_invitation(&input.invitation)?;
    cancellation::ensure_peer_available(paths.inner(), &invitation.host_device_id)?;
    let store = SyncStore::open(paths.inner())?;
    ensure_identity(&store, runtime.inner())?;
    store.create_session(
        &invitation.session_id,
        "join",
        &invitation.endpoint,
        invitation.expires_at,
    )?;
    let cancel_signal = cancellation::begin(&invitation.session_id)?;
    store.update_session(&invitation.session_id, None, "connecting", None)?;
    crate::logging::info(
        "device_sync_receive_started",
        format!(
            "session={} sender={} endpoint={}",
            invitation.session_id, invitation.host_device_id, invitation.endpoint
        ),
    );
    let session_id = invitation.session_id.clone();
    let worker_app = app.clone();
    let worker_signal = cancel_signal.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        run_join_session(worker_app, invitation, worker_signal)
    })
    .await
    .map_err(|_| {
        CommandError::new(
            "device_sync_worker_interrupted",
            "The device synchronization worker ended before returning a result.",
        )
    })?;
    let result = cancellation::normalize_result(&cancel_signal, result);
    match &result {
        Ok(value) => {
            cancellation::finish_terminal(
                paths.inner(),
                &session_id,
                Some(&value.peer.id),
                "completed",
                None,
            )?;
        }
        Err(error) if error.code == "device_sync_canceled" => {
            cancellation::finish_terminal(
                paths.inner(),
                &session_id,
                None,
                "canceled",
                Some((&error.code, &error.message)),
            )?;
        }
        Err(error) => {
            cancellation::finish_terminal(
                paths.inner(),
                &session_id,
                None,
                "failed",
                Some((&error.code, &error.message)),
            )?;
        }
    }
    cancellation::finish(&session_id);
    result
}

#[tauri::command]
pub fn cancel_device_sync_session(
    paths: State<'_, AppPaths>,
    input: CancelDeviceSyncSessionInput,
) -> Result<(), CommandError> {
    cancellation::cancel_session(paths.inner(), &input.session_id)
}

#[tauri::command]
pub fn scan_device_pairing_qr(app: AppHandle) -> Result<Option<String>, CommandError> {
    #[cfg(mobile)]
    {
        use tauri_plugin_ostudy_platform::{OstudyPlatformExt, ScanQrCodeRequest};

        let result = app
            .ostudy_platform()
            .scan_qr_code(ScanQrCodeRequest {
                prompt: "Scan the ostudy sync offer shown on the sending device".to_string(),
            })
            .map_err(|error| {
                CommandError::new(
                    "device_sync_qr_scan_failed",
                    format!("The native QR scanner failed: {error}"),
                )
            })?;
        Ok(if result.cancelled {
            None
        } else {
            result.contents
        })
    }
    #[cfg(not(mobile))]
    {
        let _ = app;
        Err(CommandError::new(
            "device_sync_qr_scan_unavailable",
            "Paste the synchronization invitation text on this device.",
        ))
    }
}

#[tauri::command]
pub fn remove_device_sync_peer(
    paths: State<'_, AppPaths>,
    input: RemoveDeviceSyncPeerInput,
) -> Result<(), CommandError> {
    SyncStore::open(paths.inner())?.remove_peer(&input.peer_id)
}

#[tauri::command]
pub fn resolve_device_sync_conflict(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: ResolveDeviceSyncConflictInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.conflict_id, "sync conflict")?;
    let store = SyncStore::open(paths.inner())?;
    let conflict = store.conflict(&input.conflict_id)?;
    if conflict.table_name == "app_settings" {
        // Old PR #51 pre-v2 conflicts are retired without ever applying their
        // remote appearance value to this device.
        return store.resolve_conflict(&input.conflict_id, "keep_left");
    }
    let payload = match input.resolution.as_str() {
        "keep_left" => conflict.left_payload_json.as_deref(),
        "keep_right" => conflict.right_payload_json.as_deref(),
        "delete" => None,
        "merged" => Some(input.merged_payload_json.as_deref().ok_or_else(|| {
            CommandError::new(
                "device_sync_resolution_invalid",
                "A merged conflict resolution must include the merged row payload.",
            )
        })?),
        _ => {
            return Err(CommandError::new(
                "device_sync_resolution_invalid",
                "Choose keep left, keep right, merged, or delete for this conflict.",
            ));
        }
    };
    database.with_connection(|connection| {
        merge::apply_conflict_payload(connection, &conflict.table_name, &conflict.row_key, payload)
    })?;
    store.resolve_conflict(&input.conflict_id, &input.resolution)
}

fn run_host_listener(
    app: AppHandle,
    listener: TcpListener,
    invitation: PairingInvitation,
    pairing_secret: [u8; 32],
    ephemeral: EphemeralKey,
    cancel_signal: cancellation::SyncCancellationSignal,
) -> Result<String, CommandError> {
    let deadline = Instant::now()
        + Duration::from_millis(
            u64::try_from(
                invitation
                    .expires_at
                    .saturating_sub(unix_time_millis()?)
                    .max(0),
            )
            .unwrap_or_default(),
        );
    loop {
        cancel_signal.check()?;
        if Instant::now() >= deadline || unix_time_millis()? >= invitation.expires_at {
            return Err(CommandError::new(
                "device_sync_invitation_expired",
                "The synchronization invitation expired before another device accepted it.",
            ));
        }
        match listener.accept() {
            Ok((stream, remote_address)) => {
                if !is_private_address(remote_address.ip()) {
                    return Err(CommandError::new(
                        "device_sync_address_rejected",
                        "The synchronization connection did not originate from a private local-network address.",
                    ));
                }
                cancel_signal.attach_stream(&stream)?;
                let result = handle_host_connection(
                    app,
                    stream,
                    remote_address,
                    invitation,
                    pairing_secret,
                    ephemeral,
                    cancel_signal.clone(),
                );
                return cancellation::normalize_result(&cancel_signal, result);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(error) => {
                return Err(CommandError::new(
                    "device_sync_listener_failed",
                    format!("The local synchronization listener failed: {error}"),
                ));
            }
        }
    }
}

fn handle_host_connection(
    app: AppHandle,
    mut stream: TcpStream,
    remote_address: SocketAddr,
    invitation: PairingInvitation,
    pairing_secret: [u8; 32],
    ephemeral: EphemeralKey,
    cancel_signal: cancellation::SyncCancellationSignal,
) -> Result<String, CommandError> {
    cancel_signal.check()?;
    let paths = app.state::<AppPaths>();
    let database = app.state::<Database>();
    let runtime = app.state::<PlatformRuntime>();
    let store = SyncStore::open(paths.inner())?;
    let (identity, identity_key) = ensure_identity(&store, runtime.inner())?;
    let hello: ClientHello = read_plain_json(&mut stream)?;
    cancel_signal.check()?;
    validate_client_hello(&invitation, &hello, &pairing_secret)?;
    let transcript = client_transcript(&invitation, &hello)?;
    let client_ephemeral =
        crypto::decode_standard(&hello.ephemeral_public_key, "client ephemeral public key")?;
    let keys = ephemeral.derive(
        &client_ephemeral,
        &pairing_secret,
        &transcript,
        SessionRole::Host,
    )?;
    let mut channel = SecureChannel::new(stream, keys)?;
    let credentials_included = invitation.include_api_settings
        && invitation.credentials_available
        && hello.secure_credentials_available;
    let identity_fingerprint = identity_key.fingerprint();
    let unsigned_server = UnsignedServerHello {
        protocol_version: PROTOCOL_VERSION,
        session_id: &invitation.session_id,
        device_id: &identity.device_id,
        display_name: &identity.display_name,
        identity_public_key: &identity.public_key_base64,
        identity_fingerprint: &identity_fingerprint,
        include_api_settings: invitation.include_api_settings,
        credentials_included,
    };
    let server_transcript = server_transcript(&transcript, &unsigned_server)?;
    channel.send_json(
        FRAME_METADATA,
        &ServerHello {
            protocol_version: PROTOCOL_VERSION,
            session_id: invitation.session_id.clone(),
            device_id: identity.device_id.clone(),
            display_name: identity.display_name.clone(),
            identity_public_key: identity.public_key_base64.clone(),
            identity_fingerprint: identity_fingerprint.clone(),
            include_api_settings: invitation.include_api_settings,
            credentials_included,
            identity_signature: identity_key.sign(&server_transcript),
        },
    )?;
    cancel_signal.check()?;

    let peer_fingerprint = fingerprint_from_base64(&hello.identity_public_key)?;
    store.upsert_peer(
        &hello.device_id,
        &hello.display_name,
        &hello.identity_public_key,
        &peer_fingerprint,
        &remote_address.to_string(),
    )?;
    let peer = store.peer(&hello.device_id)?.ok_or_else(|| {
        CommandError::new(
            "device_sync_store_error",
            "The receiving device was not persisted after authentication.",
        )
    })?;
    cancellation::claim_peer(paths.inner(), &invitation.session_id, &peer.id)?;
    store.update_session(&invitation.session_id, Some(&peer.id), "transferring", None)?;
    crate::logging::info(
        "device_sync_sender_authenticated",
        format!("session={} peer={}", invitation.session_id, peer.id),
    );

    cancel_signal.check()?;
    let workspace = SyncWorkspace::new(paths.inner(), &invitation.session_id, "send")?;
    let outgoing_package = workspace.path.join("outgoing.syncpkg");
    let outgoing_summary = package::create_package(
        database.inner(),
        paths.inner(),
        &outgoing_package,
        invitation.include_api_settings,
    )?;
    cancel_signal.check()?;
    let outgoing_credentials = if credentials_included {
        credentials::collect_credentials(database.inner(), runtime.secure_secret_store())?
    } else {
        Vec::new()
    };
    channel.send_json(
        FRAME_METADATA,
        &TransferMetadata {
            protocol_version: PROTOCOL_VERSION,
            session_id: invitation.session_id.clone(),
            device_id: identity.device_id.clone(),
            display_name: identity.display_name.clone(),
            identity_public_key: identity.public_key_base64.clone(),
            include_api_settings: invitation.include_api_settings,
            credentials: outgoing_credentials,
            package: outgoing_summary,
        },
    )?;
    channel.send_file(&outgoing_package)?;
    cancel_signal.check()?;

    let acknowledgement: ReceiverAcknowledgement = channel.receive_json(FRAME_COMPLETE)?;
    cancel_signal.check()?;
    if acknowledgement.protocol_version != PROTOCOL_VERSION
        || acknowledgement.session_id != invitation.session_id
    {
        return Err(CommandError::new(
            "device_sync_protocol_error",
            "The receiving device returned an acknowledgement for another synchronization session.",
        ));
    }

    let baseline_result = (|| -> Result<(), CommandError> {
        let extracted = package::extract_package(
            &outgoing_package,
            &workspace.path.join("accepted-sender-baseline"),
        )?;
        let snapshot =
            Connection::open(&extracted.database_path).map_err(CommandError::database)?;
        merge::remember_sent_database(&snapshot, &store, &peer.id)
    })();
    if let Err(error) = baseline_result {
        crate::logging::error(
            "device_sync_sender_baseline_failed",
            format!(
                "peer={} code={} message={}",
                peer.id, error.code, error.message
            ),
        );
    }

    store.mark_peer_synced(&peer.id, &remote_address.to_string())?;
    crate::logging::info(
        "device_sync_sender_completed",
        format!(
            "peer={} changed_rows={} conflicts={} credentials={}",
            peer.id,
            acknowledgement.changed_row_count,
            acknowledgement.conflict_count,
            acknowledgement.transferred_credential_count
        ),
    );
    Ok(peer.id)
}

fn run_join_session(
    app: AppHandle,
    invitation: PairingInvitation,
    cancel_signal: cancellation::SyncCancellationSignal,
) -> Result<DeviceSyncResultDto, CommandError> {
    cancel_signal.check()?;
    let paths = app.state::<AppPaths>();
    let database = app.state::<Database>();
    let runtime = app.state::<PlatformRuntime>();
    let store = SyncStore::open(paths.inner())?;
    let (identity, identity_key) = ensure_identity(&store, runtime.inner())?;
    let address: SocketAddr = invitation.endpoint.parse().map_err(|_| {
        CommandError::new(
            "device_sync_invitation_invalid",
            "The synchronization invitation contains an invalid network endpoint.",
        )
    })?;
    if !is_private_address(address.ip()) {
        return Err(CommandError::new(
            "device_sync_address_rejected",
            "Device synchronization is restricted to private local-network addresses.",
        ));
    }
    let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT).map_err(|error| {
        CommandError::new(
            "device_sync_connection_failed",
            format!(
                "The sending device could not be reached at {}: {error}",
                invitation.endpoint
            ),
        )
    })?;
    cancel_signal.attach_stream(&stream)?;
    cancel_signal.check()?;
    crate::logging::info(
        "device_sync_receive_connected",
        format!(
            "session={} endpoint={}",
            invitation.session_id, invitation.endpoint
        ),
    );
    let pairing_secret = crypto::decode_standard(&invitation.pairing_secret, "pairing secret")?;
    let host_ephemeral = crypto::decode_standard(
        &invitation.host_ephemeral_public_key,
        "sender ephemeral public key",
    )?;
    let ephemeral = EphemeralKey::generate()?;
    let secure_credentials_available = matches!(
        runtime.capabilities().services.secure_secrets.state,
        CapabilityState::Available
    );
    let ephemeral_public_key = BASE64_STANDARD.encode(&ephemeral.public_key);
    let unsigned = UnsignedClientHello {
        protocol_version: PROTOCOL_VERSION,
        session_id: &invitation.session_id,
        host_device_id: &invitation.host_device_id,
        host_identity_public_key: &invitation.host_identity_public_key,
        host_ephemeral_public_key: &invitation.host_ephemeral_public_key,
        endpoint: &invitation.endpoint,
        expires_at: invitation.expires_at,
        include_api_settings: invitation.include_api_settings,
        credentials_available: invitation.credentials_available,
        device_id: &identity.device_id,
        display_name: &identity.display_name,
        identity_public_key: &identity.public_key_base64,
        ephemeral_public_key: &ephemeral_public_key,
        secure_credentials_available,
    };
    let transcript = serde_json::to_vec(&unsigned).map_err(CommandError::serialization)?;
    write_plain_json(
        &mut stream,
        &ClientHello {
            protocol_version: PROTOCOL_VERSION,
            session_id: invitation.session_id.clone(),
            device_id: identity.device_id.clone(),
            display_name: identity.display_name.clone(),
            identity_public_key: identity.public_key_base64.clone(),
            ephemeral_public_key,
            secure_credentials_available,
            pairing_proof: crypto::pairing_proof(&pairing_secret, &transcript),
            identity_signature: identity_key.sign(&transcript),
        },
    )?;
    let keys = ephemeral.derive(
        &host_ephemeral,
        &pairing_secret,
        &transcript,
        SessionRole::Joiner,
    )?;
    let mut channel = SecureChannel::new(stream, keys)?;
    let server: ServerHello = channel.receive_json(FRAME_METADATA)?;
    cancel_signal.check()?;
    validate_server_hello(
        &invitation,
        &server,
        &transcript,
        secure_credentials_available,
    )?;
    store.upsert_peer(
        &server.device_id,
        &server.display_name,
        &server.identity_public_key,
        &server.identity_fingerprint,
        &invitation.endpoint,
    )?;
    let sender = store.peer(&server.device_id)?.ok_or_else(|| {
        CommandError::new(
            "device_sync_store_error",
            "The sending device was not persisted after authentication.",
        )
    })?;
    cancellation::claim_peer(paths.inner(), &invitation.session_id, &sender.id)?;
    store.update_session(
        &invitation.session_id,
        Some(&sender.id),
        "transferring",
        None,
    )?;
    crate::logging::info(
        "device_sync_receive_authenticated",
        format!("session={} sender={}", invitation.session_id, sender.id),
    );

    cancel_signal.check()?;
    let metadata: TransferMetadata = channel.receive_json(FRAME_METADATA)?;
    validate_transfer_metadata(&invitation, &server, &metadata)?;
    let workspace = SyncWorkspace::new(paths.inner(), &invitation.session_id, "receive")?;
    let incoming_package = workspace.path.join("incoming.syncpkg");
    channel.receive_file(&incoming_package)?;
    cancel_signal.check()?;
    store.update_session(&invitation.session_id, Some(&sender.id), "merging", None)?;
    crate::logging::info(
        "device_sync_package_received",
        format!(
            "session={} sender={} bytes={}",
            invitation.session_id, sender.id, metadata.package.package_bytes
        ),
    );

    let extracted = package::extract_package(&incoming_package, &workspace.path.join("incoming"))?;
    if extracted.include_api_settings != invitation.include_api_settings {
        return Err(CommandError::new(
            "device_sync_protocol_error",
            "The synchronization package scope did not match the accepted invitation.",
        ));
    }
    log_staged_assets("receive", &extracted.assets);
    package::install_assets(paths.inner(), &extracted.assets)?;
    cancel_signal.check()?;

    let remote_database =
        Connection::open(&extracted.database_path).map_err(CommandError::database)?;
    remote_database
        .execute_batch("PRAGMA foreign_keys = OFF; DROP TABLE IF EXISTS app_settings;")
        .map_err(CommandError::database)?;
    if !invitation.include_api_settings {
        remote_database
            .execute_batch(
                "DROP TABLE IF EXISTS ai_task_defaults;
                 DROP TABLE IF EXISTS ai_provider_profiles;",
            )
            .map_err(CommandError::database)?;
    }
    remote_database
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(CommandError::database)?;

    cancel_signal.check()?;
    crate::logging::info(
        "device_sync_merge_started",
        format!("session={} sender={}", invitation.session_id, sender.id),
    );
    let merge = database.with_connection(|local| {
        merge::merge_remote_database(
            local,
            &remote_database,
            &store,
            &identity.device_id,
            &sender.id,
        )
    })?;
    crate::logging::info(
        "device_sync_merge_completed",
        format!(
            "session={} sender={} changed_rows={} conflicts={}",
            invitation.session_id, sender.id, merge.changed_row_count, merge.conflict_count
        ),
    );
    cancel_signal.check()?;
    let transferred_credential_count = if server.credentials_included {
        credentials::apply_credentials(
            database.inner(),
            runtime.secure_secret_store(),
            &metadata.credentials,
        )?
    } else {
        0
    };
    cancel_signal.check()?;
    channel.send_json(
        FRAME_COMPLETE,
        &ReceiverAcknowledgement {
            protocol_version: PROTOCOL_VERSION,
            session_id: invitation.session_id.clone(),
            changed_row_count: merge.changed_row_count,
            conflict_count: merge.conflict_count,
            transferred_credential_count,
        },
    )?;
    store.mark_peer_synced(&sender.id, &invitation.endpoint)?;
    Ok(DeviceSyncResultDto {
        session_id: invitation.session_id,
        peer: sender,
        changed_row_count: merge.changed_row_count,
        conflict_count: merge.conflict_count,
        transferred_credential_count,
        sent_package_bytes: 0,
        received_package_bytes: metadata.package.package_bytes,
    })
}

fn ensure_identity(
    store: &SyncStore,
    runtime: &PlatformRuntime,
) -> Result<(DeviceIdentityRecord, IdentityKey), CommandError> {
    store.ensure_identity(&default_device_name())?;
    let identity = store.identity()?.ok_or_else(|| {
        CommandError::new(
            "device_sync_store_error",
            "The local device identity was not persisted.",
        )
    })?;
    let identity_key = IdentityKey::load_or_create(runtime.secure_secret_store().as_ref())?;
    if identity.public_key_base64 == identity_key.public_key_base64() {
        return Ok((identity, identity_key));
    }
    let identity = store.set_identity_public_key(&identity_key.public_key_base64())?;
    Ok((identity, identity_key))
}

fn log_staged_assets(direction: &str, assets: &[package::ExtractedAsset]) {
    let asset_bytes = assets
        .iter()
        .fold(0_u64, |total, asset| total.saturating_add(asset.byte_size));
    let (first_asset, first_mime) = assets
        .first()
        .map(|asset| (asset.asset_id.as_str(), asset.mime_type.as_str()))
        .unwrap_or(("none", "none"));
    crate::logging::info(
        "device_sync_assets_staged",
        format!(
            "direction={direction} count={} bytes={asset_bytes} first_asset={first_asset} first_mime={first_mime}",
            assets.len()
        ),
    );
}

fn render_pairing_qr(invitation: &str) -> Result<String, CommandError> {
    use qrcode::{render::svg, QrCode};

    let code = QrCode::new(invitation.as_bytes()).map_err(|_| {
        CommandError::new(
            "device_sync_invitation_too_large",
            "The synchronization invitation is too large to render as a QR code.",
        )
    })?;
    Ok(code
        .render::<svg::Color>()
        .min_dimensions(320, 320)
        .max_dimensions(640, 640)
        .build())
}

fn parse_invitation(value: &str) -> Result<PairingInvitation, CommandError> {
    let trimmed = value.trim();
    if trimmed.len() > MAX_INVITATION_CHARS || !trimmed.starts_with(INVITATION_PREFIX) {
        return Err(CommandError::new(
            "device_sync_invitation_invalid",
            "Scan or paste an ostudy synchronization invitation.",
        ));
    }
    let encoded = &trimmed[INVITATION_PREFIX.len()..];
    let bytes = crypto::decode_url(encoded)?;
    let invitation: PairingInvitation = serde_json::from_slice(&bytes).map_err(|error| {
        CommandError::new(
            "device_sync_invitation_invalid",
            format!("The synchronization invitation is invalid: {error}"),
        )
    })?;
    validate_entity_id(&invitation.session_id, "sync session")?;
    validate_entity_id(&invitation.host_device_id, "sending device")?;
    if invitation.protocol_version != PROTOCOL_VERSION
        || invitation.expires_at <= unix_time_millis()?
        || invitation.expires_at
            > unix_time_millis()?.saturating_add(MAX_EXPIRY_SECONDS.saturating_mul(1_000))
    {
        return Err(CommandError::new(
            "device_sync_invitation_expired",
            "The synchronization invitation is expired or unsupported. Create a new offer on the sending device.",
        ));
    }
    if invitation.credentials_available && !invitation.include_api_settings {
        return Err(CommandError::new(
            "device_sync_invitation_invalid",
            "The synchronization invitation contains an invalid credential scope.",
        ));
    }
    let address: SocketAddr = invitation.endpoint.parse().map_err(|_| {
        CommandError::new(
            "device_sync_invitation_invalid",
            "The synchronization invitation contains an invalid local-network endpoint.",
        )
    })?;
    if !is_private_address(address.ip()) {
        return Err(CommandError::new(
            "device_sync_address_rejected",
            "The synchronization invitation does not point to a private local-network address.",
        ));
    }
    let host_public = crypto::decode_standard(
        &invitation.host_identity_public_key,
        "sender identity public key",
    )?;
    let host_ephemeral = crypto::decode_standard(
        &invitation.host_ephemeral_public_key,
        "sender ephemeral public key",
    )?;
    let secret = crypto::decode_standard(&invitation.pairing_secret, "pairing secret")?;
    if host_public.len() != 32
        || host_ephemeral.len() != 32
        || secret.len() != 32
        || crypto::fingerprint(&host_public) != invitation.host_identity_fingerprint
    {
        return Err(CommandError::new(
            "device_sync_invitation_invalid",
            "The synchronization invitation contains invalid cryptographic identity data.",
        ));
    }
    Ok(invitation)
}

fn validate_client_hello(
    invitation: &PairingInvitation,
    hello: &ClientHello,
    pairing_secret: &[u8],
) -> Result<(), CommandError> {
    validate_entity_id(&hello.device_id, "receiving device")?;
    if hello.protocol_version != PROTOCOL_VERSION
        || hello.session_id != invitation.session_id
        || unix_time_millis()? >= invitation.expires_at
    {
        return Err(CommandError::new(
            "device_sync_handshake_failed",
            "The receiving device used an expired or mismatched synchronization session.",
        ));
    }
    let transcript = client_transcript(invitation, hello)?;
    crypto::verify_pairing_proof(pairing_secret, &transcript, &hello.pairing_proof)?;
    crypto::verify_signature(
        &hello.identity_public_key,
        &transcript,
        &hello.identity_signature,
    )?;
    if crypto::decode_standard(&hello.identity_public_key, "receiver identity public key")?.len()
        != 32
        || crypto::decode_standard(&hello.ephemeral_public_key, "receiver ephemeral public key")?
            .len()
            != 32
    {
        return Err(CommandError::new(
            "device_sync_handshake_failed",
            "The receiving device presented invalid public keys.",
        ));
    }
    Ok(())
}

fn validate_server_hello(
    invitation: &PairingInvitation,
    server: &ServerHello,
    client_transcript: &[u8],
    receiver_secure_credentials_available: bool,
) -> Result<(), CommandError> {
    if server.protocol_version != PROTOCOL_VERSION
        || server.session_id != invitation.session_id
        || server.device_id != invitation.host_device_id
        || server.identity_public_key != invitation.host_identity_public_key
        || server.identity_fingerprint != invitation.host_identity_fingerprint
        || server.include_api_settings != invitation.include_api_settings
        || (server.credentials_included
            && (!invitation.include_api_settings
                || !invitation.credentials_available
                || !receiver_secure_credentials_available))
    {
        return Err(CommandError::new(
            "device_sync_handshake_failed",
            "The sending device identity or offered synchronization scope did not match the scanned invitation.",
        ));
    }
    let unsigned = UnsignedServerHello {
        protocol_version: server.protocol_version,
        session_id: &server.session_id,
        device_id: &server.device_id,
        display_name: &server.display_name,
        identity_public_key: &server.identity_public_key,
        identity_fingerprint: &server.identity_fingerprint,
        include_api_settings: server.include_api_settings,
        credentials_included: server.credentials_included,
    };
    let transcript = server_transcript(client_transcript, &unsigned)?;
    crypto::verify_signature(
        &server.identity_public_key,
        &transcript,
        &server.identity_signature,
    )
}

fn validate_transfer_metadata(
    invitation: &PairingInvitation,
    server: &ServerHello,
    metadata: &TransferMetadata,
) -> Result<(), CommandError> {
    if metadata.protocol_version != PROTOCOL_VERSION
        || metadata.session_id != invitation.session_id
        || metadata.device_id != server.device_id
        || metadata.display_name != server.display_name
        || metadata.identity_public_key != server.identity_public_key
        || metadata.include_api_settings != invitation.include_api_settings
        || (!server.credentials_included && !metadata.credentials.is_empty())
    {
        return Err(CommandError::new(
            "device_sync_protocol_error",
            "The sending device sent inconsistent synchronization metadata.",
        ));
    }
    Ok(())
}

fn client_transcript(
    invitation: &PairingInvitation,
    hello: &ClientHello,
) -> Result<Vec<u8>, CommandError> {
    serde_json::to_vec(&UnsignedClientHello {
        protocol_version: hello.protocol_version,
        session_id: &hello.session_id,
        host_device_id: &invitation.host_device_id,
        host_identity_public_key: &invitation.host_identity_public_key,
        host_ephemeral_public_key: &invitation.host_ephemeral_public_key,
        endpoint: &invitation.endpoint,
        expires_at: invitation.expires_at,
        include_api_settings: invitation.include_api_settings,
        credentials_available: invitation.credentials_available,
        device_id: &hello.device_id,
        display_name: &hello.display_name,
        identity_public_key: &hello.identity_public_key,
        ephemeral_public_key: &hello.ephemeral_public_key,
        secure_credentials_available: hello.secure_credentials_available,
    })
    .map_err(CommandError::serialization)
}

fn server_transcript(
    client_transcript: &[u8],
    server: &UnsignedServerHello<'_>,
) -> Result<Vec<u8>, CommandError> {
    let mut output = client_transcript.to_vec();
    output.extend_from_slice(&serde_json::to_vec(server).map_err(CommandError::serialization)?);
    Ok(output)
}

fn resolve_host_ip(override_value: Option<&str>) -> Result<IpAddr, CommandError> {
    if let Some(value) = override_value
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let address: IpAddr = value.parse().map_err(|_| {
            CommandError::new(
                "device_sync_host_invalid",
                "The advanced synchronization host override must be a private IP address.",
            )
        })?;
        if is_private_address(address) {
            return Ok(address);
        }
        return Err(CommandError::new(
            "device_sync_host_invalid",
            "The advanced synchronization host override must be a private IP address.",
        ));
    }
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(CommandError::io)?;
    socket
        .connect((Ipv4Addr::new(192, 0, 2, 1), 9))
        .map_err(CommandError::io)?;
    let address = socket.local_addr().map_err(CommandError::io)?.ip();
    if is_private_address(address) && !address.is_loopback() {
        Ok(address)
    } else {
        Err(CommandError::new(
            "device_sync_host_unavailable",
            "ostudy could not determine a private LAN address. Enter this device's LAN IP in the advanced host field.",
        ))
    }
}

fn is_private_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            address.is_private() || address.is_loopback() || address.is_link_local()
        }
        IpAddr::V6(address) => {
            address.is_loopback()
                || address.is_unicast_link_local()
                || is_unique_local_ipv6(address)
        }
    }
}

fn is_unique_local_ipv6(address: Ipv6Addr) -> bool {
    address.segments()[0] & 0xfe00 == 0xfc00
}

fn fingerprint_from_base64(public_key: &str) -> Result<String, CommandError> {
    let bytes = crypto::decode_standard(public_key, "device identity public key")?;
    if bytes.len() != 32 {
        return Err(CommandError::new(
            "device_sync_handshake_failed",
            "The paired device identity public key has an invalid size.",
        ));
    }
    Ok(crypto::fingerprint(&bytes))
}

fn random_id(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn default_device_name() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value.chars().count() <= 120)
        .unwrap_or_else(platform_default_device_name)
}

#[cfg(mobile)]
fn platform_default_device_name() -> String {
    "Mobile device".to_string()
}

#[cfg(not(mobile))]
fn platform_default_device_name() -> String {
    "ostudy device".to_string()
}

struct SyncWorkspace {
    path: PathBuf,
}

impl SyncWorkspace {
    fn new(paths: &AppPaths, session_id: &str, role: &str) -> Result<Self, CommandError> {
        let root = paths
            .app_data_dir()
            .join("transfers")
            .join("device-sync")
            .join(format!("{session_id}-{role}-{}", std::process::id()));
        match fs::remove_dir_all(&root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(CommandError::io(error)),
        }
        fs::create_dir_all(&root).map_err(CommandError::io)?;
        Ok(Self { path: root })
    }
}

impl Drop for SyncWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_invitation(include_api_settings: bool) -> PairingInvitation {
        PairingInvitation {
            protocol_version: PROTOCOL_VERSION,
            session_id: "a".repeat(32),
            host_device_id: "b".repeat(32),
            host_display_name: "Sender".to_string(),
            host_identity_public_key: BASE64_STANDARD.encode([1_u8; 32]),
            host_identity_fingerprint: crypto::fingerprint(&[1_u8; 32]),
            host_ephemeral_public_key: BASE64_STANDARD.encode([2_u8; 32]),
            endpoint: "192.168.1.10:4444".to_string(),
            pairing_secret: BASE64_STANDARD.encode([3_u8; 32]),
            expires_at: crate::platform::now_utc_millis().unwrap_or_default() + 60_000,
            include_api_settings,
            credentials_available: include_api_settings,
        }
    }

    #[test]
    fn invitation_round_trip_rejects_public_endpoints() {
        let mut payload = test_invitation(false);
        payload.endpoint = "203.0.113.10:4444".to_string();
        let encoded =
            crypto::encode_url(&serde_json::to_vec(&payload).expect("invitation should serialize"));
        assert!(parse_invitation(&format!("{INVITATION_PREFIX}{encoded}")).is_err());
    }

    #[test]
    fn invitation_rejects_credentials_without_api_scope() {
        let mut payload = test_invitation(false);
        payload.credentials_available = true;
        let encoded =
            crypto::encode_url(&serde_json::to_vec(&payload).expect("invitation should serialize"));
        assert!(parse_invitation(&format!("{INVITATION_PREFIX}{encoded}")).is_err());
    }

    #[test]
    fn client_transcript_binds_sender_selected_api_scope() {
        let hello = ClientHello {
            protocol_version: PROTOCOL_VERSION,
            session_id: "a".repeat(32),
            device_id: "c".repeat(32),
            display_name: "Receiver".to_string(),
            identity_public_key: BASE64_STANDARD.encode([4_u8; 32]),
            ephemeral_public_key: BASE64_STANDARD.encode([5_u8; 32]),
            secure_credentials_available: true,
            pairing_proof: String::new(),
            identity_signature: String::new(),
        };
        let without_api = client_transcript(&test_invitation(false), &hello).unwrap();
        let with_api = client_transcript(&test_invitation(true), &hello).unwrap();
        assert_ne!(without_api, with_api);
    }

    #[test]
    fn private_address_policy_accepts_lan_loopback_and_unique_local_ipv6() {
        assert!(is_private_address("192.168.1.10".parse().unwrap()));
        assert!(is_private_address("10.0.0.8".parse().unwrap()));
        assert!(is_private_address("127.0.0.1".parse().unwrap()));
        assert!(is_private_address("fd00::1".parse().unwrap()));
        assert!(!is_private_address("8.8.8.8".parse().unwrap()));
    }
}
