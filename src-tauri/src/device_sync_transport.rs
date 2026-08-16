//! Encrypted, QR-bootstrapped local-network transport for device snapshots.
//!
//! The QR invitation authenticates a short-lived one-shot HTTP endpoint. The
//! snapshot is encrypted before it reaches the socket with a 256-bit
//! ChaCha20-Poly1305 key. First pairing includes that random key in the QR code;
//! later sessions omit it and load the peer key from the platform secure store.
//! HTTP is therefore only an opaque LAN carrier, not a confidentiality boundary.

use std::{
    collections::HashMap,
    fs,
    io::Write,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket},
    path::{Path, PathBuf},
    str::FromStr,
    sync::LazyLock,
};

use base64::{
    engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use qrcode::{Color, QrCode};
use reqwest::{redirect::Policy, Client, StatusCode};
use ring::{
    aead::{self, Aad, LessSafeKey, Nonce, UnboundKey},
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
    time::{timeout, Duration},
};
use url::{Host, Url};

use crate::{
    database::Database,
    device_sync::{
        self, DeviceSyncPreviewDto, DeviceSyncSnapshot, SyncIdentityDto,
    },
    error::CommandError,
    paths::AppPaths,
    platform::{PlatformRuntime, SecretBytes, SecretLocator, SecretStoreAvailability},
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const INVITATION_FORMAT: &str = "ostudy-device-sync-invitation";
const INVITATION_VERSION: i64 = 1;
const ENVELOPE_VERSION: i64 = 1;
const INVITATION_PREFIX: &str = "ostudy-sync:v1:";
const SESSION_LIFETIME_MILLIS: i64 = 10 * 60 * 1_000;
const SERVER_TIMEOUT_SECONDS: u64 = 10 * 60;
const CLIENT_TIMEOUT_SECONDS: u64 = 45;
const MAX_HTTP_HEADER_BYTES: usize = 32 * 1024;
const MAX_ACK_BYTES: usize = 64 * 1024;
const MAX_ENCRYPTED_SNAPSHOT_BYTES: usize = 720 * 1024 * 1024;
const TRANSPORT_KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;
const PEER_REGISTRY_LIMIT: u64 = 512 * 1024;
const MAX_HOST_OVERRIDE_CHARS: usize = 64;

static ACTIVE_SESSIONS: LazyLock<Mutex<HashMap<String, ActiveSyncSession>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartDeviceSyncSessionInput {
    pub device_name: String,
    pub peer_device_id: Option<String>,
    pub include_settings: bool,
    pub include_ai_profiles: bool,
    pub host_override: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReceiveDeviceSyncInvitationInput {
    pub invitation: String,
    pub operation_id: String,
    pub device_name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncSessionIdInput {
    pub session_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSyncInvitationDto {
    pub session_id: String,
    pub invitation: String,
    pub endpoint: String,
    pub expires_at_utc_millis: i64,
    pub qr_size: usize,
    pub qr_modules_base64: String,
    pub pairing: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSyncSessionStatusDto {
    pub session_id: String,
    pub state: String,
    pub downloaded: bool,
    pub acknowledged: bool,
    pub peer_device_id: Option<String>,
    pub peer_device_name: Option<String>,
    pub expires_at_utc_millis: i64,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivedDeviceSyncDto {
    pub peer: TrustedSyncDeviceDto,
    pub preview: DeviceSyncPreviewDto,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustedSyncDeviceDto {
    pub device_id: String,
    pub device_name: String,
    pub paired_at_utc_millis: i64,
    pub last_seen_at_utc_millis: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SyncInvitation {
    format: String,
    version: i64,
    session_id: String,
    source_device_id: String,
    source_device_name: String,
    endpoint: String,
    expires_at_utc_millis: i64,
    pairing_key_base64: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EncryptedEnvelope {
    version: i64,
    nonce_base64: String,
    ciphertext_base64: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SyncTransportPayload {
    snapshot: DeviceSyncSnapshot,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SyncAcknowledgement {
    receiver_device_id: String,
    receiver_device_name: String,
    received_snapshot_id: String,
}

#[derive(Clone, Debug)]
struct ActiveSyncSession {
    status: DeviceSyncSessionStatusDto,
    key: Vec<u8>,
    snapshot: DeviceSyncSnapshot,
    acknowledged_peer: Option<SyncAcknowledgement>,
    finalized: bool,
}

#[tauri::command]
pub async fn start_device_sync_session(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: StartDeviceSyncSessionInput,
) -> Result<DeviceSyncInvitationDto, CommandError> {
    let identity = device_sync::sync_identity(
        database.inner(),
        paths.inner(),
        input.device_name,
    )?;
    let peer_device_id = input
        .peer_device_id
        .as_deref()
        .map(|value| value.trim().replace('-', ""));
    if let Some(peer_device_id) = &peer_device_id {
        validate_entity_id(peer_device_id, "paired device")?;
    }
    let snapshot = device_sync::capture_sync_snapshot_for_transport(
        database.inner(),
        paths.inner(),
        identity.device_name.clone(),
        input.include_settings,
        input.include_ai_profiles,
    )?;
    let session_id = database.with_connection(|connection| generate_id(connection))?;
    let key = if let Some(peer_device_id) = &peer_device_id {
        read_peer_key(runtime.inner(), peer_device_id)?
    } else {
        random_transport_key()?
    };
    let payload = SyncTransportPayload {
        snapshot: snapshot.clone(),
    };
    let aad = snapshot_aad(&session_id, &identity.device_id);
    let encrypted = encrypt_json(&payload, &key, &aad)?;
    let encrypted_bytes = serde_json::to_vec(&encrypted).map_err(|error| {
        transport_serialization_error("The encrypted snapshot could not be encoded.", error)
    })?;
    if encrypted_bytes.len() > MAX_ENCRYPTED_SNAPSHOT_BYTES {
        return Err(CommandError::new(
            "sync_transport_too_large",
            "The encrypted synchronization snapshot exceeds the LAN transfer limit.",
        ));
    }

    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))
        .await
        .map_err(transport_io_error)?;
    let port = listener.local_addr().map_err(transport_io_error)?.port();
    let host = select_lan_address(input.host_override.as_deref())?;
    let endpoint = endpoint_url(host, port, &session_id);
    let expires_at_utc_millis = unix_time_millis()?.saturating_add(SESSION_LIFETIME_MILLIS);
    let invitation = SyncInvitation {
        format: INVITATION_FORMAT.to_string(),
        version: INVITATION_VERSION,
        session_id: session_id.clone(),
        source_device_id: identity.device_id.clone(),
        source_device_name: identity.device_name,
        endpoint: endpoint.clone(),
        expires_at_utc_millis,
        pairing_key_base64: peer_device_id
            .is_none()
            .then(|| URL_SAFE_NO_PAD.encode(&key)),
    };
    let invitation_text = encode_invitation(&invitation)?;
    let (qr_size, qr_modules_base64) = qr_modules(&invitation_text)?;

    ACTIVE_SESSIONS.lock().await.insert(
        session_id.clone(),
        ActiveSyncSession {
            status: DeviceSyncSessionStatusDto {
                session_id: session_id.clone(),
                state: "waiting".to_string(),
                downloaded: false,
                acknowledged: false,
                peer_device_id: peer_device_id.clone(),
                peer_device_name: None,
                expires_at_utc_millis,
                error_message: None,
            },
            key: key.clone(),
            snapshot,
            acknowledged_peer: None,
            finalized: false,
        },
    );

    tokio::spawn(run_one_shot_server(
        listener,
        session_id.clone(),
        invitation.source_device_id,
        key,
        encrypted_bytes,
    ));

    Ok(DeviceSyncInvitationDto {
        session_id,
        invitation: invitation_text,
        endpoint,
        expires_at_utc_millis,
        qr_size,
        qr_modules_base64,
        pairing: peer_device_id.is_none(),
    })
}

#[tauri::command]
pub async fn receive_device_sync_invitation(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: ReceiveDeviceSyncInvitationInput,
) -> Result<ReceivedDeviceSyncDto, CommandError> {
    validate_entity_id(&input.operation_id, "device sync receive operation")?;
    let invitation = decode_invitation(&input.invitation)?;
    let now = unix_time_millis()?;
    if invitation.expires_at_utc_millis <= now {
        return Err(CommandError::new(
            "sync_invitation_expired",
            "This QR synchronization invitation has expired.",
        ));
    }
    validate_endpoint(&invitation.endpoint, &invitation.session_id)?;
    let key = match &invitation.pairing_key_base64 {
        Some(encoded) => decode_transport_key(encoded)?,
        None => read_peer_key(runtime.inner(), &invitation.source_device_id)?,
    };
    let client = secure_lan_client()?;
    let response = client
        .get(&invitation.endpoint)
        .send()
        .await
        .map_err(transport_request_error)?;
    if response.status() != StatusCode::OK {
        return Err(CommandError::new(
            "sync_transport_failed",
            format!(
                "The paired device rejected the synchronization request with status {}.",
                response.status()
            ),
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_ENCRYPTED_SNAPSHOT_BYTES as u64)
    {
        return Err(CommandError::new(
            "sync_transport_too_large",
            "The paired device reported an oversized synchronization response.",
        ));
    }
    let bytes = response.bytes().await.map_err(transport_request_error)?;
    if bytes.is_empty() || bytes.len() > MAX_ENCRYPTED_SNAPSHOT_BYTES {
        return Err(CommandError::new(
            "sync_transport_too_large",
            "The paired device returned an empty or oversized synchronization response.",
        ));
    }
    let envelope: EncryptedEnvelope = serde_json::from_slice(&bytes).map_err(|error| {
        transport_serialization_error("The encrypted synchronization response is invalid.", error)
    })?;
    let aad = snapshot_aad(&invitation.session_id, &invitation.source_device_id);
    let payload: SyncTransportPayload = decrypt_json(&envelope, &key, &aad)?;
    if payload.snapshot.source_device_id != invitation.source_device_id {
        return Err(CommandError::new(
            "sync_transport_identity_mismatch",
            "The encrypted snapshot identity does not match its QR invitation.",
        ));
    }
    let preview = device_sync::preview_transport_snapshot(
        database.inner(),
        paths.inner(),
        &input.operation_id,
        &invitation.source_device_id,
        payload.snapshot.clone(),
    )?;

    if invitation.pairing_key_base64.is_some() {
        write_peer_key(runtime.inner(), &invitation.source_device_id, &key)?;
    }
    let peer = record_peer(
        paths.inner(),
        &invitation.source_device_id,
        &invitation.source_device_name,
        now,
    )?;
    let receiver = device_sync::sync_identity(
        database.inner(),
        paths.inner(),
        input.device_name,
    )?;
    let acknowledgement = SyncAcknowledgement {
        receiver_device_id: receiver.device_id,
        receiver_device_name: receiver.device_name,
        received_snapshot_id: payload.snapshot.snapshot_id,
    };
    let ack_envelope = encrypt_json(
        &acknowledgement,
        &key,
        &ack_aad(&invitation.session_id, &invitation.source_device_id),
    )?;
    let ack_url = format!("{}/ack", invitation.endpoint.trim_end_matches('/'));
    let ack_response = client
        .post(ack_url)
        .json(&ack_envelope)
        .send()
        .await
        .map_err(transport_request_error)?;
    if ack_response.status() != StatusCode::NO_CONTENT {
        return Err(CommandError::new(
            "sync_acknowledgement_failed",
            "The snapshot was received, but the source device did not accept its pairing acknowledgement.",
        ));
    }
    Ok(ReceivedDeviceSyncDto { peer, preview })
}

#[tauri::command]
pub async fn get_device_sync_session_status(
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    input: SyncSessionIdInput,
) -> Result<DeviceSyncSessionStatusDto, CommandError> {
    validate_entity_id(&input.session_id, "device sync session")?;
    let mut sessions = ACTIVE_SESSIONS.lock().await;
    let session = sessions.get_mut(&input.session_id).ok_or_else(|| {
        CommandError::new(
            "sync_session_not_found",
            "That synchronization session has expired or was cancelled.",
        )
    })?;
    if !session.finalized {
        if let Some(peer) = session.acknowledged_peer.clone() {
            validate_entity_id(&peer.receiver_device_id, "paired receiver")?;
            write_peer_key(runtime.inner(), &peer.receiver_device_id, &session.key)?;
            record_peer(
                paths.inner(),
                &peer.receiver_device_id,
                &peer.receiver_device_name,
                unix_time_millis()?,
            )?;
            device_sync::acknowledge_transport_base(
                paths.inner(),
                &peer.receiver_device_id,
                &session.snapshot,
            )?;
            session.status.peer_device_id = Some(peer.receiver_device_id);
            session.status.peer_device_name = Some(peer.receiver_device_name);
            session.status.state = "acknowledged".to_string();
            session.finalized = true;
        }
    }
    Ok(session.status.clone())
}

#[tauri::command]
pub async fn cancel_device_sync_session(
    input: SyncSessionIdInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.session_id, "device sync session")?;
    if let Some(mut session) = ACTIVE_SESSIONS.lock().await.remove(&input.session_id) {
        session.key.fill(0);
    }
    Ok(())
}

#[tauri::command]
pub fn list_trusted_sync_devices(
    paths: State<'_, AppPaths>,
) -> Result<Vec<TrustedSyncDeviceDto>, CommandError> {
    load_peers(paths.inner())
}

#[tauri::command]
pub fn remove_trusted_sync_device(
    paths: State<'_, AppPaths>,
    runtime: State<'_, PlatformRuntime>,
    peer_device_id: String,
) -> Result<(), CommandError> {
    let peer_device_id = peer_device_id.trim().replace('-', "");
    validate_entity_id(&peer_device_id, "paired device")?;
    let mut peers = load_peers(paths.inner())?;
    peers.retain(|peer| peer.device_id != peer_device_id);
    write_peers(paths.inner(), &peers)?;
    runtime
        .secure_secret_store()
        .delete(&peer_secret_locator(&peer_device_id))
        .map_err(|_| {
            CommandError::new(
                "sync_secure_store_failed",
                "The paired-device key could not be removed from secure storage.",
            )
        })?;
    device_sync::remove_transport_base(paths.inner(), &peer_device_id)?;
    Ok(())
}

async fn run_one_shot_server(
    listener: TcpListener,
    session_id: String,
    source_device_id: String,
    key: Vec<u8>,
    encrypted_snapshot: Vec<u8>,
) {
    let result = timeout(Duration::from_secs(SERVER_TIMEOUT_SECONDS), async {
        loop {
            let (stream, _) = listener.accept().await.map_err(|error| error.to_string())?;
            match handle_connection(
                stream,
                &session_id,
                &source_device_id,
                &key,
                &encrypted_snapshot,
            )
            .await
            {
                Ok(ConnectionOutcome::Continue) => {}
                Ok(ConnectionOutcome::Acknowledged(acknowledgement)) => {
                    let mut sessions = ACTIVE_SESSIONS.lock().await;
                    if let Some(session) = sessions.get_mut(&session_id) {
                        session.status.acknowledged = true;
                        session.status.state = "acknowledged".to_string();
                        session.status.peer_device_id =
                            Some(acknowledgement.receiver_device_id.clone());
                        session.status.peer_device_name =
                            Some(acknowledgement.receiver_device_name.clone());
                        session.acknowledged_peer = Some(acknowledgement);
                    }
                    return Ok::<(), String>(());
                }
                Err(error) => {
                    let mut sessions = ACTIVE_SESSIONS.lock().await;
                    if let Some(session) = sessions.get_mut(&session_id) {
                        session.status.error_message = Some(error);
                    }
                }
            }
        }
    })
    .await;
    let mut sessions = ACTIVE_SESSIONS.lock().await;
    if let Some(session) = sessions.get_mut(&session_id) {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                session.status.state = "failed".to_string();
                session.status.error_message = Some(error);
            }
            Err(_) => {
                session.status.state = "expired".to_string();
                session.status.error_message = Some("The QR synchronization session expired.".to_string());
            }
        }
    }
}

enum ConnectionOutcome {
    Continue,
    Acknowledged(SyncAcknowledgement),
}

async fn handle_connection(
    mut stream: TcpStream,
    session_id: &str,
    source_device_id: &str,
    key: &[u8],
    encrypted_snapshot: &[u8],
) -> Result<ConnectionOutcome, String> {
    let request = read_http_request(&mut stream).await?;
    let expected_path = format!("/v1/sync/{session_id}");
    if request.method == "GET" && request.path == expected_path {
        write_http_response(&mut stream, 200, "application/json", encrypted_snapshot)
            .await?;
        let mut sessions = ACTIVE_SESSIONS.lock().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.status.downloaded = true;
            session.status.state = "downloaded".to_string();
        }
        return Ok(ConnectionOutcome::Continue);
    }
    if request.method == "POST" && request.path == format!("{expected_path}/ack") {
        if request.body.is_empty() || request.body.len() > MAX_ACK_BYTES {
            write_http_response(&mut stream, 413, "text/plain", b"acknowledgement too large")
                .await?;
            return Ok(ConnectionOutcome::Continue);
        }
        let envelope: EncryptedEnvelope = serde_json::from_slice(&request.body)
            .map_err(|error| format!("Invalid encrypted acknowledgement: {error}"))?;
        let acknowledgement: SyncAcknowledgement = decrypt_json(
            &envelope,
            key,
            &ack_aad(session_id, source_device_id),
        )
        .map_err(|error| error.message)?;
        validate_entity_id(&acknowledgement.receiver_device_id, "paired receiver")
            .map_err(|error| error.message)?;
        write_http_response(&mut stream, 204, "text/plain", b"").await?;
        return Ok(ConnectionOutcome::Acknowledged(acknowledgement));
    }
    write_http_response(&mut stream, 404, "text/plain", b"not found").await?;
    Ok(ConnectionOutcome::Continue)
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

async fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut buffer = Vec::new();
    let mut scratch = [0_u8; 4096];
    let header_end = loop {
        let read = stream.read(&mut scratch).await.map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("The LAN peer closed the request before sending headers.".to_string());
        }
        buffer.extend_from_slice(&scratch[..read]);
        if buffer.len() > MAX_HTTP_HEADER_BYTES {
            return Err("The LAN request headers exceed the limit.".to_string());
        }
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
    };
    let header_text = std::str::from_utf8(&buffer[..header_end])
        .map_err(|_| "The LAN request headers are not UTF-8.".to_string())?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "The LAN request line is missing.".to_string())?;
    let parts = request_line.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" {
        return Err("The LAN request line is invalid.".to_string());
    }
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.trim().parse::<usize>())
        .transpose()
        .map_err(|_| "The LAN content length is invalid.".to_string())?
        .unwrap_or(0);
    if content_length > MAX_ACK_BYTES {
        return Err("The LAN request body exceeds the limit.".to_string());
    }
    let body_start = header_end + 4;
    while buffer.len().saturating_sub(body_start) < content_length {
        let read = stream.read(&mut scratch).await.map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("The LAN peer closed the request before sending its body.".to_string());
        }
        buffer.extend_from_slice(&scratch[..read]);
        if buffer.len().saturating_sub(body_start) > MAX_ACK_BYTES {
            return Err("The LAN request body exceeds the limit.".to_string());
        }
    }
    Ok(HttpRequest {
        method: parts[0].to_string(),
        path: parts[1].to_string(),
        body: buffer[body_start..body_start + content_length].to_vec(),
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

async fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Error",
    };
    let headers = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
        body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    stream
        .write_all(body)
        .await
        .map_err(|error| error.to_string())?;
    stream.shutdown().await.map_err(|error| error.to_string())
}

fn random_transport_key() -> Result<Vec<u8>, CommandError> {
    let mut key = vec![0_u8; TRANSPORT_KEY_BYTES];
    SystemRandom::new().fill(&mut key).map_err(|_| {
        CommandError::new(
            "sync_random_failed",
            "The operating system could not generate a secure pairing key.",
        )
    })?;
    Ok(key)
}

fn encrypt_json<T: Serialize>(
    value: &T,
    key: &[u8],
    aad: &[u8],
) -> Result<EncryptedEnvelope, CommandError> {
    let mut plaintext = serde_json::to_vec(value).map_err(|error| {
        transport_serialization_error("The synchronization payload could not be encoded.", error)
    })?;
    let mut nonce_bytes = [0_u8; NONCE_BYTES];
    SystemRandom::new().fill(&mut nonce_bytes).map_err(|_| {
        CommandError::new(
            "sync_random_failed",
            "The operating system could not generate an encryption nonce.",
        )
    })?;
    let sealing_key = transport_key(key)?;
    sealing_key
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::from(aad),
            &mut plaintext,
        )
        .map_err(|_| {
            CommandError::new(
                "sync_encryption_failed",
                "The synchronization payload could not be encrypted.",
            )
        })?;
    Ok(EncryptedEnvelope {
        version: ENVELOPE_VERSION,
        nonce_base64: URL_SAFE_NO_PAD.encode(nonce_bytes),
        ciphertext_base64: URL_SAFE_NO_PAD.encode(plaintext),
    })
}

fn decrypt_json<T: for<'de> Deserialize<'de>>(
    envelope: &EncryptedEnvelope,
    key: &[u8],
    aad: &[u8],
) -> Result<T, CommandError> {
    if envelope.version != ENVELOPE_VERSION {
        return Err(CommandError::new(
            "sync_envelope_unsupported",
            "The encrypted synchronization envelope uses an unsupported version.",
        ));
    }
    let nonce = URL_SAFE_NO_PAD
        .decode(&envelope.nonce_base64)
        .map_err(|_| CommandError::new("sync_envelope_invalid", "The encryption nonce is invalid."))?;
    let nonce: [u8; NONCE_BYTES] = nonce.try_into().map_err(|_| {
        CommandError::new(
            "sync_envelope_invalid",
            "The encryption nonce has the wrong length.",
        )
    })?;
    let mut ciphertext = URL_SAFE_NO_PAD
        .decode(&envelope.ciphertext_base64)
        .map_err(|_| {
            CommandError::new(
                "sync_envelope_invalid",
                "The encrypted synchronization payload is invalid.",
            )
        })?;
    let opening_key = transport_key(key)?;
    let plaintext = opening_key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(aad),
            &mut ciphertext,
        )
        .map_err(|_| {
            CommandError::new(
                "sync_authentication_failed",
                "The synchronization payload failed authenticated decryption.",
            )
        })?;
    serde_json::from_slice(plaintext).map_err(|error| {
        transport_serialization_error("The decrypted synchronization payload is invalid.", error)
    })
}

fn transport_key(key: &[u8]) -> Result<LessSafeKey, CommandError> {
    if key.len() != TRANSPORT_KEY_BYTES {
        return Err(CommandError::new(
            "sync_key_invalid",
            "The paired-device encryption key has the wrong length.",
        ));
    }
    let unbound = UnboundKey::new(&aead::CHACHA20_POLY1305, key).map_err(|_| {
        CommandError::new(
            "sync_key_invalid",
            "The paired-device encryption key is invalid.",
        )
    })?;
    Ok(LessSafeKey::new(unbound))
}

fn encode_invitation(invitation: &SyncInvitation) -> Result<String, CommandError> {
    let bytes = serde_json::to_vec(invitation).map_err(|error| {
        transport_serialization_error("The QR invitation could not be encoded.", error)
    })?;
    Ok(format!("{INVITATION_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes)))
}

fn decode_invitation(value: &str) -> Result<SyncInvitation, CommandError> {
    let value = value.trim();
    let encoded = value.strip_prefix(INVITATION_PREFIX).ok_or_else(|| {
        CommandError::new(
            "sync_invitation_invalid",
            "This QR code is not an ostudy synchronization invitation.",
        )
    })?;
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| {
        CommandError::new(
            "sync_invitation_invalid",
            "The ostudy synchronization invitation is not valid base64url data.",
        )
    })?;
    let invitation: SyncInvitation = serde_json::from_slice(&bytes).map_err(|error| {
        transport_serialization_error("The synchronization invitation is invalid.", error)
    })?;
    if invitation.format != INVITATION_FORMAT || invitation.version != INVITATION_VERSION {
        return Err(CommandError::new(
            "sync_invitation_unsupported",
            "This synchronization invitation uses an unsupported protocol version.",
        ));
    }
    validate_entity_id(&invitation.session_id, "device sync session")?;
    validate_entity_id(&invitation.source_device_id, "pairing source device")?;
    validate_endpoint(&invitation.endpoint, &invitation.session_id)?;
    Ok(invitation)
}

fn qr_modules(value: &str) -> Result<(usize, String), CommandError> {
    let code = QrCode::new(value.as_bytes()).map_err(|_| {
        CommandError::new(
            "sync_qr_too_large",
            "The synchronization invitation is too large to encode as a QR code.",
        )
    })?;
    let colors = code.to_colors();
    let mut packed = vec![0_u8; colors.len().div_ceil(8)];
    for (index, color) in colors.iter().enumerate() {
        if *color == Color::Dark {
            packed[index / 8] |= 1 << (7 - (index % 8));
        }
    }
    Ok((code.width(), BASE64_STANDARD.encode(packed)))
}

fn read_peer_key(runtime: &PlatformRuntime, peer_device_id: &str) -> Result<Vec<u8>, CommandError> {
    if runtime.secure_secret_store().availability() != SecretStoreAvailability::Available {
        return Err(CommandError::new(
            "sync_secure_store_unavailable",
            "Secure storage is unavailable, so this trusted-device key cannot be opened.",
        ));
    }
    let secret = runtime
        .secure_secret_store()
        .read(&peer_secret_locator(peer_device_id))
        .map_err(|_| {
            CommandError::new(
                "sync_secure_store_failed",
                "The trusted-device key could not be read from secure storage.",
            )
        })?
        .ok_or_else(|| {
            CommandError::new(
                "sync_peer_not_paired",
                "This device has no saved encryption key for that peer. Pair it again.",
            )
        })?;
    Ok(secret.as_bytes().to_vec())
}

fn write_peer_key(
    runtime: &PlatformRuntime,
    peer_device_id: &str,
    key: &[u8],
) -> Result<(), CommandError> {
    if runtime.secure_secret_store().availability() != SecretStoreAvailability::Available {
        return Err(CommandError::new(
            "sync_secure_store_unavailable",
            "Secure storage is unavailable, so the paired-device key cannot be saved.",
        ));
    }
    runtime
        .secure_secret_store()
        .write(&peer_secret_locator(peer_device_id), key)
        .map_err(|_| {
            CommandError::new(
                "sync_secure_store_failed",
                "The paired-device key could not be saved to secure storage.",
            )
        })
}

fn peer_secret_locator(peer_device_id: &str) -> SecretLocator {
    SecretLocator {
        namespace: "device_sync",
        key: format!("peer/{peer_device_id}/transport-key"),
    }
}

fn decode_transport_key(value: &str) -> Result<Vec<u8>, CommandError> {
    let key = URL_SAFE_NO_PAD.decode(value).map_err(|_| {
        CommandError::new(
            "sync_key_invalid",
            "The QR pairing key is not valid base64url data.",
        )
    })?;
    if key.len() != TRANSPORT_KEY_BYTES {
        return Err(CommandError::new(
            "sync_key_invalid",
            "The QR pairing key has the wrong length.",
        ));
    }
    Ok(key)
}

fn select_lan_address(host_override: Option<&str>) -> Result<IpAddr, CommandError> {
    if let Some(value) = host_override.map(str::trim).filter(|value| !value.is_empty()) {
        if value.len() > MAX_HOST_OVERRIDE_CHARS {
            return Err(CommandError::new(
                "sync_host_invalid",
                "The manual LAN address is too long.",
            ));
        }
        let ip = IpAddr::from_str(value).map_err(|_| {
            CommandError::new(
                "sync_host_invalid",
                "Enter a numeric private LAN address, not a hostname.",
            )
        })?;
        if !is_private_lan_ip(ip) {
            return Err(CommandError::new(
                "sync_host_invalid",
                "The synchronization endpoint must use a private or link-local address.",
            ));
        }
        return Ok(ip);
    }
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(transport_io_error)?;
    socket
        .connect((Ipv4Addr::new(192, 0, 2, 1), 9))
        .map_err(transport_io_error)?;
    let ip = socket.local_addr().map_err(transport_io_error)?.ip();
    if ip.is_unspecified() || ip.is_loopback() || !is_private_lan_ip(ip) {
        return Err(CommandError::new(
            "sync_lan_address_unavailable",
            "ostudy could not determine a private LAN address. Enter this device's Wi-Fi address manually.",
        ));
    }
    Ok(ip)
}

fn endpoint_url(host: IpAddr, port: u16, session_id: &str) -> String {
    match host {
        IpAddr::V4(address) => format!("http://{address}:{port}/v1/sync/{session_id}"),
        IpAddr::V6(address) => format!("http://[{address}]:{port}/v1/sync/{session_id}"),
    }
}

fn validate_endpoint(endpoint: &str, session_id: &str) -> Result<(), CommandError> {
    let url = Url::parse(endpoint).map_err(|_| {
        CommandError::new(
            "sync_endpoint_invalid",
            "The QR invitation contains an invalid synchronization endpoint.",
        )
    })?;
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.port().is_none()
        || url.path() != format!("/v1/sync/{session_id}")
    {
        return Err(CommandError::new(
            "sync_endpoint_invalid",
            "The QR invitation contains an unsupported synchronization endpoint.",
        ));
    }
    let ip = match url.host() {
        Some(Host::Ipv4(address)) => IpAddr::V4(address),
        Some(Host::Ipv6(address)) => IpAddr::V6(address),
        _ => {
            return Err(CommandError::new(
                "sync_endpoint_invalid",
                "Synchronization invitations must use a numeric private LAN address.",
            ))
        }
    };
    if !is_private_lan_ip(ip) {
        return Err(CommandError::new(
            "sync_endpoint_invalid",
            "The synchronization endpoint is not on a private or link-local network.",
        ));
    }
    Ok(())
}

fn is_private_lan_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(address) => {
            address.is_private() || address.is_link_local() || address.is_loopback()
        }
        IpAddr::V6(address) => {
            address.is_loopback()
                || address.is_unicast_link_local()
                || (address.segments()[0] & 0xfe00) == 0xfc00
        }
    }
}

fn secure_lan_client() -> Result<Client, CommandError> {
    Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(CLIENT_TIMEOUT_SECONDS))
        .build()
        .map_err(|error| {
            CommandError::new(
                "sync_transport_unavailable",
                format!("The LAN synchronization client could not start: {error}"),
            )
        })
}

fn snapshot_aad(session_id: &str, source_device_id: &str) -> Vec<u8> {
    format!("ostudy-sync-snapshot:{session_id}:{source_device_id}").into_bytes()
}

fn ack_aad(session_id: &str, source_device_id: &str) -> Vec<u8> {
    format!("ostudy-sync-ack:{session_id}:{source_device_id}").into_bytes()
}

fn peers_path(paths: &AppPaths) -> PathBuf {
    paths.app_data_dir().join("device-sync").join("trusted-peers.json")
}

fn load_peers(paths: &AppPaths) -> Result<Vec<TrustedSyncDeviceDto>, CommandError> {
    let path = peers_path(paths);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let metadata = fs::metadata(&path).map_err(CommandError::io)?;
    if metadata.len() == 0 || metadata.len() > PEER_REGISTRY_LIMIT {
        return Err(CommandError::new(
            "sync_peer_registry_invalid",
            "The trusted-device registry is empty or exceeds its size limit.",
        ));
    }
    let mut peers: Vec<TrustedSyncDeviceDto> =
        serde_json::from_slice(&fs::read(path).map_err(CommandError::io)?).map_err(|error| {
            transport_serialization_error("The trusted-device registry is invalid.", error)
        })?;
    peers.sort_by(|left, right| left.device_name.cmp(&right.device_name));
    Ok(peers)
}

fn record_peer(
    paths: &AppPaths,
    device_id: &str,
    device_name: &str,
    observed_at: i64,
) -> Result<TrustedSyncDeviceDto, CommandError> {
    validate_entity_id(device_id, "paired device")?;
    let name = device_name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(CommandError::new(
            "sync_device_name_invalid",
            "The paired device name is invalid.",
        ));
    }
    let mut peers = load_peers(paths)?;
    let peer = if let Some(existing) = peers.iter_mut().find(|peer| peer.device_id == device_id) {
        existing.device_name = name.to_string();
        existing.last_seen_at_utc_millis = observed_at;
        existing.clone()
    } else {
        let peer = TrustedSyncDeviceDto {
            device_id: device_id.to_string(),
            device_name: name.to_string(),
            paired_at_utc_millis: observed_at,
            last_seen_at_utc_millis: observed_at,
        };
        peers.push(peer.clone());
        peer
    };
    write_peers(paths, &peers)?;
    Ok(peer)
}

fn write_peers(paths: &AppPaths, peers: &[TrustedSyncDeviceDto]) -> Result<(), CommandError> {
    let path = peers_path(paths);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    let bytes = serde_json::to_vec(peers).map_err(|error| {
        transport_serialization_error("The trusted-device registry could not be encoded.", error)
    })?;
    if bytes.len() as u64 > PEER_REGISTRY_LIMIT {
        return Err(CommandError::new(
            "sync_peer_registry_too_large",
            "The trusted-device registry exceeds its size limit.",
        ));
    }
    let temporary = path.with_extension(format!("part-{}", std::process::id()));
    let _ = fs::remove_file(&temporary);
    write_private_file(&temporary, &bytes)?;
    if path.exists() {
        let backup = path.with_extension("previous");
        let _ = fs::remove_file(&backup);
        fs::rename(&path, &backup).map_err(CommandError::io)?;
        if let Err(error) = fs::rename(&temporary, &path) {
            let _ = fs::rename(&backup, &path);
            return Err(CommandError::io(error));
        }
        let _ = fs::remove_file(backup);
    } else {
        fs::rename(&temporary, &path).map_err(CommandError::io)?;
    }
    Ok(())
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(CommandError::io)?;
    file.write_all(bytes).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)
}

fn transport_serialization_error(message: &str, error: serde_json::Error) -> CommandError {
    CommandError::new(
        "sync_transport_serialization_failed",
        format!("{message} {error}"),
    )
}

fn transport_io_error(error: std::io::Error) -> CommandError {
    CommandError::new(
        "sync_transport_io_failed",
        format!("The local synchronization transport failed: {error}"),
    )
}

fn transport_request_error(error: reqwest::Error) -> CommandError {
    CommandError::new(
        "sync_transport_failed",
        format!("The paired device could not be reached: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitation_round_trip_preserves_pairing_identity() {
        let invitation = SyncInvitation {
            format: INVITATION_FORMAT.to_string(),
            version: INVITATION_VERSION,
            session_id: "11111111111111111111111111111111".to_string(),
            source_device_id: "22222222222222222222222222222222".to_string(),
            source_device_name: "Desktop".to_string(),
            endpoint: "http://192.168.1.20:43111/v1/sync/11111111111111111111111111111111"
                .to_string(),
            expires_at_utc_millis: i64::MAX,
            pairing_key_base64: Some(URL_SAFE_NO_PAD.encode([7_u8; TRANSPORT_KEY_BYTES])),
        };
        assert_eq!(
            decode_invitation(&encode_invitation(&invitation).expect("encode should work"))
                .expect("decode should work")
                .source_device_id,
            invitation.source_device_id
        );
    }

    #[test]
    fn authenticated_envelope_rejects_modified_ciphertext() {
        let key = [9_u8; TRANSPORT_KEY_BYTES];
        let aad = b"test-aad";
        let mut envelope = encrypt_json(&vec!["card"], &key, aad).expect("seal should work");
        let mut ciphertext = URL_SAFE_NO_PAD
            .decode(&envelope.ciphertext_base64)
            .expect("ciphertext should decode");
        ciphertext[0] ^= 0x40;
        envelope.ciphertext_base64 = URL_SAFE_NO_PAD.encode(ciphertext);
        assert!(decrypt_json::<Vec<String>>(&envelope, &key, aad).is_err());
    }

    #[test]
    fn public_endpoints_are_rejected_before_network_access() {
        assert!(validate_endpoint(
            "http://8.8.8.8:4000/v1/sync/11111111111111111111111111111111",
            "11111111111111111111111111111111",
        )
        .is_err());
    }

    #[test]
    fn qr_module_payload_has_exact_bit_capacity() {
        let (size, packed) = qr_modules("ostudy-sync:v1:test").expect("QR should encode");
        let bytes = BASE64_STANDARD.decode(packed).expect("modules should decode");
        assert!(bytes.len() * 8 >= size * size);
        assert!(bytes.len() * 8 < size * size + 8);
    }
}
