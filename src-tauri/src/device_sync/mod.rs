//! Explicit trusted-device synchronization over the local network.
//!
//! A QR invitation carries only a short-lived endpoint, one-time secret, and
//! public handshake material. Library snapshots and optional credentials travel
//! through an authenticated encrypted channel. The merge engine keeps stable
//! entity identities, unions independent changes, and persists explicit
//! conflicts instead of silently discarding concurrent edits.

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
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::{
    database::Database,
    error::CommandError,
    paths::AppPaths,
    platform::PlatformRuntime,
    util::{unix_time_millis, validate_entity_id},
};

use self::{
    credentials::CredentialRecord,
    crypto::{EphemeralKey, IdentityKey, SessionRole},
    merge::MergeSummary,
    package::PackageSummary,
    store::{
        ConflictRecord, DeviceIdentityRecord, NewConflict, PeerRecord, SessionRecord, SyncStore,
    },
    transport::{read_plain_json, write_plain_json, SecureChannel, FRAME_COMPLETE, FRAME_METADATA},
};

const PROTOCOL_VERSION: u32 = 1;
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
    pub include_credentials: bool,
    pub expires_in_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JoinDevicePairingInput {
    pub invitation: String,
    pub include_credentials: bool,
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
    include_credentials: bool,
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
    device_id: &'a str,
    display_name: &'a str,
    identity_public_key: &'a str,
    ephemeral_public_key: &'a str,
    include_credentials: bool,
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
    credentials_accepted: bool,
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
    credentials_accepted: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TransferMetadata {
    protocol_version: u32,
    session_id: String,
    device_id: String,
    display_name: String,
    identity_public_key: String,
    include_credentials: bool,
    credentials: Vec<CredentialRecord>,
    package: PackageSummary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HostResponseMetadata {
    protocol_version: u32,
    session_id: String,
    merge: MergeSummary,
    conflicts: Vec<ConflictRecord>,
    credentials: Vec<CredentialRecord>,
    package: PackageSummary,
}

#[tauri::command]
pub fn get_device_sync_state(
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
) -> Result<DeviceSyncStateDto, CommandError> {
    let store = SyncStore::open(paths.inner())?;
    let (identity, identity_key) = ensure_identity(&store, runtime.inner())?;
    let capabilities = runtime.capabilities();
    Ok(DeviceSyncStateDto {
        identity,
        fingerprint: identity_key.fingerprint(),
        peers: store.list_peers()?,
        conflicts: store.list_conflicts()?,
        sessions: store.list_sessions()?,
        local_network_available: matches!(
            capabilities.services.local_network.state,
            crate::platform::CapabilityState::Available
        ),
        qr_scan_available: matches!(
            capabilities.services.qr_scan.state,
            crate::platform::CapabilityState::Available
                | crate::platform::CapabilityState::PermissionRequired
        ),
        secure_credentials_available: matches!(
            capabilities.services.secure_secrets.state,
            crate::platform::CapabilityState::Available
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
    let expiry_seconds = input
        .expires_in_seconds
        .unwrap_or(DEFAULT_EXPIRY_SECONDS)
        .clamp(MIN_EXPIRY_SECONDS, MAX_EXPIRY_SECONDS);
    let expires_at = unix_time_millis()?.saturating_add(expiry_seconds.saturating_mul(1_000));
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(|error| {
        CommandError::new(
            "device_sync_listener_failed",
            format!("ostudy could not open a local pairing listener: {error}"),
        )
    })?;
    listener.set_nonblocking(true).map_err(CommandError::io)?;
    let port = listener.local_addr().map_err(CommandError::io)?.port();
    let host_ip = resolve_host_ip(input.host_override.as_deref())?;
    let endpoint = SocketAddr::new(host_ip, port).to_string();
    let pairing_secret = crypto::random_pairing_secret()?;
    let ephemeral = EphemeralKey::generate()?;
    let session_id = random_id(&pairing_secret[..16]);
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
        credentials_available: input.include_credentials,
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
            "The pairing invitation exceeds the supported QR-code size.",
        ));
    }
    let qr_svg = render_pairing_qr(&invitation)?;
    store.create_session(&session_id, "host", &endpoint, expires_at)?;

    let host_app = app.clone();
    let host_session = session_id.clone();
    thread::Builder::new()
        .name(format!("ostudy-sync-host-{}", &session_id[..8]))
        .spawn(move || {
            let result = run_host_listener(
                host_app.clone(),
                listener,
                invitation_payload,
                pairing_secret,
                ephemeral,
            );
            let paths = host_app.state::<AppPaths>();
            if let Ok(store) = SyncStore::open(paths.inner()) {
                match result {
                    Ok(peer_id) => {
                        let _ =
                            store.update_session(&host_session, Some(&peer_id), "completed", None);
                    }
                    Err(error) if error.code == "device_sync_invitation_expired" => {
                        let _ = store.update_session(
                            &host_session,
                            None,
                            "expired",
                            Some((&error.code, &error.message)),
                        );
                    }
                    Err(error) => {
                        let _ = store.update_session(
                            &host_session,
                            None,
                            "failed",
                            Some((&error.code, &error.message)),
                        );
                    }
                }
            }
        })
        .map_err(|error| {
            CommandError::new(
                "device_sync_listener_failed",
                format!("The local pairing worker could not start: {error}"),
            )
        })?;

    Ok(PairingInvitationDto {
        session_id,
        invitation,
        qr_svg,
        endpoint,
        fingerprint: identity_key.fingerprint(),
        expires_at,
        credentials_available: input.include_credentials,
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
    let store = SyncStore::open(paths.inner())?;
    ensure_identity(&store, runtime.inner())?;
    store.create_session(
        &invitation.session_id,
        "join",
        &invitation.endpoint,
        invitation.expires_at,
    )?;
    store.update_session(&invitation.session_id, None, "connecting", None)?;
    let session_id = invitation.session_id.clone();
    let worker_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        run_join_session(worker_app, invitation, input.include_credentials)
    })
    .await
    .map_err(|_| {
        CommandError::new(
            "device_sync_worker_interrupted",
            "The device synchronization worker ended before returning a result.",
        )
    })?;
    match &result {
        Ok(value) => {
            store.update_session(&session_id, Some(&value.peer.id), "completed", None)?;
        }
        Err(error) => {
            store.update_session(
                &session_id,
                None,
                "failed",
                Some((&error.code, &error.message)),
            )?;
        }
    }
    result
}

#[tauri::command]
pub fn scan_device_pairing_qr(app: AppHandle) -> Result<Option<String>, CommandError> {
    #[cfg(mobile)]
    {
        use tauri_plugin_ostudy_platform::{OstudyPlatformExt, ScanQrCodeRequest};

        let result = app
            .ostudy_platform()
            .scan_qr_code(ScanQrCodeRequest {
                prompt: "Scan the ostudy pairing code shown on the other device".to_string(),
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
            "Use the pairing invitation text or scan it with the receiving mobile device.",
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
        if Instant::now() >= deadline || unix_time_millis()? >= invitation.expires_at {
            return Err(CommandError::new(
                "device_sync_invitation_expired",
                "The pairing invitation expired before another device connected.",
            ));
        }
        match listener.accept() {
            Ok((stream, remote_address)) => {
                if !is_private_address(remote_address.ip()) {
                    return Err(CommandError::new(
                        "device_sync_address_rejected",
                        "The pairing connection did not originate from a private local-network address.",
                    ));
                }
                return handle_host_connection(
                    app,
                    stream,
                    remote_address,
                    invitation,
                    pairing_secret,
                    ephemeral,
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(error) => {
                return Err(CommandError::new(
                    "device_sync_listener_failed",
                    format!("The local pairing listener failed: {error}"),
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
) -> Result<String, CommandError> {
    let paths = app.state::<AppPaths>();
    let database = app.state::<Database>();
    let runtime = app.state::<PlatformRuntime>();
    let store = SyncStore::open(paths.inner())?;
    let (identity, identity_key) = ensure_identity(&store, runtime.inner())?;
    let hello: ClientHello = read_plain_json(&mut stream)?;
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
    let credentials_accepted = invitation.credentials_available && hello.include_credentials;
    let identity_fingerprint = identity_key.fingerprint();
    let unsigned_server = UnsignedServerHello {
        protocol_version: PROTOCOL_VERSION,
        session_id: &invitation.session_id,
        device_id: &identity.device_id,
        display_name: &identity.display_name,
        identity_public_key: &identity.public_key_base64,
        identity_fingerprint: &identity_fingerprint,
        credentials_accepted,
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
            credentials_accepted,
            identity_signature: identity_key.sign(&server_transcript),
        },
    )?;

    let peer_fingerprint = fingerprint_from_base64(&hello.identity_public_key)?;
    let peer = store.upsert_peer(
        &hello.device_id,
        &hello.display_name,
        &hello.identity_public_key,
        &peer_fingerprint,
        &remote_address.to_string(),
    )?;
    store.update_session(&invitation.session_id, Some(&peer.id), "transferring", None)?;
    let metadata: TransferMetadata = channel.receive_json(FRAME_METADATA)?;
    validate_transfer_metadata(
        &invitation.session_id,
        &hello,
        &metadata,
        credentials_accepted,
    )?;
    let workspace = SyncWorkspace::new(paths.inner(), &invitation.session_id, "host")?;
    let incoming_package = workspace.path.join("incoming.syncpkg");
    channel.receive_file(&incoming_package)?;
    store.update_session(&invitation.session_id, Some(&peer.id), "merging", None)?;

    let extracted = package::extract_package(&incoming_package, &workspace.path.join("incoming"))?;
    package::install_assets(paths.inner(), &extracted.assets)?;
    let remote_database =
        Connection::open_with_flags(&extracted.database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(CommandError::database)?;
    let merge = database.with_connection(|local| {
        merge::merge_remote_database(
            local,
            &remote_database,
            &store,
            &identity.device_id,
            &peer.id,
        )
    })?;
    let transferred_credential_count = if credentials_accepted {
        credentials::apply_credentials(
            database.inner(),
            runtime.secure_secret_store(),
            &metadata.credentials,
        )?
    } else {
        0
    };
    let response_package = workspace.path.join("merged.syncpkg");
    let response_summary =
        package::create_package(database.inner(), paths.inner(), &response_package)?;
    let response_credentials = if credentials_accepted {
        credentials::collect_credentials(database.inner(), runtime.secure_secret_store())?
    } else {
        Vec::new()
    };
    let conflicts = store
        .list_conflicts()?
        .into_iter()
        .filter(|conflict| conflict.peer_id == peer.id)
        .collect::<Vec<_>>();
    channel.send_json(
        FRAME_METADATA,
        &HostResponseMetadata {
            protocol_version: PROTOCOL_VERSION,
            session_id: invitation.session_id.clone(),
            merge: merge.clone(),
            conflicts,
            credentials: response_credentials,
            package: response_summary,
        },
    )?;
    channel.send_file(&response_package)?;
    let acknowledgement: String = channel.receive_json(FRAME_COMPLETE)?;
    if acknowledgement != "applied" {
        return Err(CommandError::new(
            "device_sync_protocol_error",
            "The joining device did not confirm the merged library.",
        ));
    }
    store.mark_peer_synced(&peer.id, &remote_address.to_string())?;
    crate::logging::info(
        "device_sync_host_completed",
        format!(
            "peer={} changed_rows={} conflicts={} credentials={transferred_credential_count}",
            peer.id, merge.changed_row_count, merge.conflict_count
        ),
    );
    Ok(peer.id)
}

fn run_join_session(
    app: AppHandle,
    invitation: PairingInvitation,
    include_credentials: bool,
) -> Result<DeviceSyncResultDto, CommandError> {
    let paths = app.state::<AppPaths>();
    let database = app.state::<Database>();
    let runtime = app.state::<PlatformRuntime>();
    let store = SyncStore::open(paths.inner())?;
    let (identity, identity_key) = ensure_identity(&store, runtime.inner())?;
    let address: SocketAddr = invitation.endpoint.parse().map_err(|_| {
        CommandError::new(
            "device_sync_invitation_invalid",
            "The pairing invitation contains an invalid network endpoint.",
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
                "The paired device could not be reached at {}: {error}",
                invitation.endpoint
            ),
        )
    })?;
    let pairing_secret = crypto::decode_standard(&invitation.pairing_secret, "pairing secret")?;
    let host_ephemeral = crypto::decode_standard(
        &invitation.host_ephemeral_public_key,
        "host ephemeral public key",
    )?;
    let ephemeral = EphemeralKey::generate()?;
    let include_credentials = include_credentials && invitation.credentials_available;
    let ephemeral_public_key = BASE64_STANDARD.encode(&ephemeral.public_key);
    let unsigned = UnsignedClientHello {
        protocol_version: PROTOCOL_VERSION,
        session_id: &invitation.session_id,
        host_device_id: &invitation.host_device_id,
        host_identity_public_key: &invitation.host_identity_public_key,
        host_ephemeral_public_key: &invitation.host_ephemeral_public_key,
        endpoint: &invitation.endpoint,
        expires_at: invitation.expires_at,
        device_id: &identity.device_id,
        display_name: &identity.display_name,
        identity_public_key: &identity.public_key_base64,
        ephemeral_public_key: &ephemeral_public_key,
        include_credentials,
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
            include_credentials,
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
    validate_server_hello(&invitation, &server, &transcript)?;
    let host = store.upsert_peer(
        &server.device_id,
        &server.display_name,
        &server.identity_public_key,
        &server.identity_fingerprint,
        &invitation.endpoint,
    )?;

    let workspace = SyncWorkspace::new(paths.inner(), &invitation.session_id, "join")?;
    let outgoing_package = workspace.path.join("outgoing.syncpkg");
    let outgoing_summary =
        package::create_package(database.inner(), paths.inner(), &outgoing_package)?;
    let outgoing_credentials = if server.credentials_accepted {
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
            include_credentials: server.credentials_accepted,
            credentials: outgoing_credentials,
            package: outgoing_summary.clone(),
        },
    )?;
    channel.send_file(&outgoing_package)?;

    let response: HostResponseMetadata = channel.receive_json(FRAME_METADATA)?;
    if response.protocol_version != PROTOCOL_VERSION || response.session_id != invitation.session_id
    {
        return Err(CommandError::new(
            "device_sync_protocol_error",
            "The paired device returned a response for another synchronization session.",
        ));
    }
    let incoming_package = workspace.path.join("merged.syncpkg");
    channel.receive_file(&incoming_package)?;
    let extracted = package::extract_package(&incoming_package, &workspace.path.join("merged"))?;
    package::install_assets(paths.inner(), &extracted.assets)?;
    let authoritative =
        Connection::open_with_flags(&extracted.database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(CommandError::database)?;
    let applied = database.with_connection(|local| {
        merge::apply_authoritative_database(local, &authoritative, &store, &host.id)
    })?;
    let transferred_credential_count = if server.credentials_accepted {
        credentials::apply_credentials(
            database.inner(),
            runtime.secure_secret_store(),
            &response.credentials,
        )?
    } else {
        0
    };
    for conflict in &response.conflicts {
        store.record_conflict(NewConflict {
            peer_id: &host.id,
            table_name: &conflict.table_name,
            row_key: &conflict.row_key,
            left_device_id: &conflict.left_device_id,
            left_payload_json: conflict.left_payload_json.as_deref(),
            right_device_id: &conflict.right_device_id,
            right_payload_json: conflict.right_payload_json.as_deref(),
            selected_payload_json: conflict.selected_payload_json.as_deref(),
            conflicting_fields: &conflict.conflicting_fields,
        })?;
    }
    channel.send_json(FRAME_COMPLETE, &"applied")?;
    store.mark_peer_synced(&host.id, &invitation.endpoint)?;
    let conflict_count = store
        .list_conflicts()?
        .into_iter()
        .filter(|conflict| conflict.peer_id == host.id)
        .count();
    Ok(DeviceSyncResultDto {
        session_id: invitation.session_id,
        peer: host,
        changed_row_count: response
            .merge
            .changed_row_count
            .saturating_add(applied.changed_row_count),
        conflict_count,
        transferred_credential_count,
        sent_package_bytes: outgoing_summary.package_bytes,
        received_package_bytes: response.package.package_bytes,
    })
}

fn ensure_identity(
    store: &SyncStore,
    runtime: &PlatformRuntime,
) -> Result<(DeviceIdentityRecord, IdentityKey), CommandError> {
    let identity = store.ensure_identity(&default_device_name())?;
    let identity_key = IdentityKey::load_or_create(runtime.secure_secret_store().as_ref())?;
    let identity = store.set_identity_public_key(&identity_key.public_key_base64())?;
    Ok((identity, identity_key))
}

fn render_pairing_qr(invitation: &str) -> Result<String, CommandError> {
    use qrcode::{render::svg, QrCode};

    let code = QrCode::new(invitation.as_bytes()).map_err(|_| {
        CommandError::new(
            "device_sync_invitation_too_large",
            "The pairing invitation is too large to render as a QR code.",
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
            "Scan or paste an ostudy device pairing invitation.",
        ));
    }
    let encoded = &trimmed[INVITATION_PREFIX.len()..];
    let bytes = crypto::decode_url(encoded)?;
    let invitation: PairingInvitation = serde_json::from_slice(&bytes).map_err(|error| {
        CommandError::new(
            "device_sync_invitation_invalid",
            format!("The pairing invitation is invalid: {error}"),
        )
    })?;
    validate_entity_id(&invitation.session_id, "sync session")?;
    validate_entity_id(&invitation.host_device_id, "host device")?;
    if invitation.protocol_version != PROTOCOL_VERSION
        || invitation.expires_at <= unix_time_millis()?
        || invitation.expires_at
            > unix_time_millis()?.saturating_add(MAX_EXPIRY_SECONDS.saturating_mul(1_000))
    {
        return Err(CommandError::new(
            "device_sync_invitation_expired",
            "The pairing invitation is expired or unsupported.",
        ));
    }
    let address: SocketAddr = invitation.endpoint.parse().map_err(|_| {
        CommandError::new(
            "device_sync_invitation_invalid",
            "The pairing invitation contains an invalid local-network endpoint.",
        )
    })?;
    if !is_private_address(address.ip()) {
        return Err(CommandError::new(
            "device_sync_address_rejected",
            "The pairing invitation does not point to a private local-network address.",
        ));
    }
    let host_public = crypto::decode_standard(
        &invitation.host_identity_public_key,
        "host identity public key",
    )?;
    let host_ephemeral = crypto::decode_standard(
        &invitation.host_ephemeral_public_key,
        "host ephemeral public key",
    )?;
    let secret = crypto::decode_standard(&invitation.pairing_secret, "pairing secret")?;
    if host_public.len() != 32
        || host_ephemeral.len() != 32
        || secret.len() != 32
        || crypto::fingerprint(&host_public) != invitation.host_identity_fingerprint
    {
        return Err(CommandError::new(
            "device_sync_invitation_invalid",
            "The pairing invitation contains invalid cryptographic identity data.",
        ));
    }
    Ok(invitation)
}

fn validate_client_hello(
    invitation: &PairingInvitation,
    hello: &ClientHello,
    pairing_secret: &[u8],
) -> Result<(), CommandError> {
    validate_entity_id(&hello.device_id, "joining device")?;
    if hello.protocol_version != PROTOCOL_VERSION
        || hello.session_id != invitation.session_id
        || unix_time_millis()? >= invitation.expires_at
    {
        return Err(CommandError::new(
            "device_sync_handshake_failed",
            "The joining device used an expired or mismatched pairing session.",
        ));
    }
    let transcript = client_transcript(invitation, hello)?;
    crypto::verify_pairing_proof(pairing_secret, &transcript, &hello.pairing_proof)?;
    crypto::verify_signature(
        &hello.identity_public_key,
        &transcript,
        &hello.identity_signature,
    )?;
    if crypto::decode_standard(&hello.identity_public_key, "client identity public key")?.len()
        != 32
        || crypto::decode_standard(&hello.ephemeral_public_key, "client ephemeral public key")?
            .len()
            != 32
    {
        return Err(CommandError::new(
            "device_sync_handshake_failed",
            "The joining device presented invalid public keys.",
        ));
    }
    Ok(())
}

fn validate_server_hello(
    invitation: &PairingInvitation,
    server: &ServerHello,
    client_transcript: &[u8],
) -> Result<(), CommandError> {
    if server.protocol_version != PROTOCOL_VERSION
        || server.session_id != invitation.session_id
        || server.device_id != invitation.host_device_id
        || server.identity_public_key != invitation.host_identity_public_key
        || server.identity_fingerprint != invitation.host_identity_fingerprint
    {
        return Err(CommandError::new(
            "device_sync_handshake_failed",
            "The host identity did not match the scanned pairing invitation.",
        ));
    }
    let unsigned = UnsignedServerHello {
        protocol_version: server.protocol_version,
        session_id: &server.session_id,
        device_id: &server.device_id,
        display_name: &server.display_name,
        identity_public_key: &server.identity_public_key,
        identity_fingerprint: &server.identity_fingerprint,
        credentials_accepted: server.credentials_accepted,
    };
    let transcript = server_transcript(client_transcript, &unsigned)?;
    crypto::verify_signature(
        &server.identity_public_key,
        &transcript,
        &server.identity_signature,
    )
}

fn validate_transfer_metadata(
    session_id: &str,
    hello: &ClientHello,
    metadata: &TransferMetadata,
    credentials_accepted: bool,
) -> Result<(), CommandError> {
    if metadata.protocol_version != PROTOCOL_VERSION
        || metadata.session_id != session_id
        || metadata.device_id != hello.device_id
        || metadata.display_name != hello.display_name
        || metadata.identity_public_key != hello.identity_public_key
        || metadata.include_credentials != credentials_accepted
        || (!credentials_accepted && !metadata.credentials.is_empty())
    {
        return Err(CommandError::new(
            "device_sync_protocol_error",
            "The joining device sent inconsistent synchronization metadata.",
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
        device_id: &hello.device_id,
        display_name: &hello.display_name,
        identity_public_key: &hello.identity_public_key,
        ephemeral_public_key: &hello.ephemeral_public_key,
        include_credentials: hello.include_credentials,
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
                "The advanced pairing host override must be a private IP address.",
            )
        })?;
        if is_private_address(address) {
            return Ok(address);
        }
        return Err(CommandError::new(
            "device_sync_host_invalid",
            "The advanced pairing host override must be a private IP address.",
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

    #[test]
    fn invitation_round_trip_rejects_public_endpoints() {
        let payload = PairingInvitation {
            protocol_version: PROTOCOL_VERSION,
            session_id: "a".repeat(32),
            host_device_id: "b".repeat(32),
            host_display_name: "Host".to_string(),
            host_identity_public_key: BASE64_STANDARD.encode([1_u8; 32]),
            host_identity_fingerprint: crypto::fingerprint(&[1_u8; 32]),
            host_ephemeral_public_key: BASE64_STANDARD.encode([2_u8; 32]),
            endpoint: "203.0.113.10:4444".to_string(),
            pairing_secret: BASE64_STANDARD.encode([3_u8; 32]),
            expires_at: crate::platform::now_utc_millis().unwrap_or_default() + 60_000,
            credentials_available: false,
        };
        let encoded =
            crypto::encode_url(&serde_json::to_vec(&payload).expect("invitation should serialize"));
        assert!(parse_invitation(&format!("{INVITATION_PREFIX}{encoded}")).is_err());
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
