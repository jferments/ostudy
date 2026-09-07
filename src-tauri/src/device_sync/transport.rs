use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::Duration,
};

use ring::aead;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::CommandError;

use super::crypto::SessionKeys;

pub(crate) const FRAME_METADATA: u8 = 1;
pub(crate) const FRAME_FILE_HEADER: u8 = 2;
pub(crate) const FRAME_FILE_CHUNK: u8 = 3;
pub(crate) const FRAME_FILE_END: u8 = 4;
pub(crate) const FRAME_COMPLETE: u8 = 5;
pub(crate) const FRAME_ERROR: u8 = 6;

const MAX_PLAIN_JSON_BYTES: usize = 64 * 1024;
const MAX_ENCRYPTED_FRAME_BYTES: usize = 1024 * 1024 + 64;
const MAX_JSON_FRAME_BYTES: usize = 512 * 1024;
const FILE_CHUNK_BYTES: usize = 768 * 1024;
const MAX_TRANSFER_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(120);

/// The byte-stream boundary used by the synchronization protocol.
///
/// Framing, authentication, package transfer, and merge coordination must not
/// know whether bytes travel over LAN TCP, a USB bridge, or a future platform
/// transport. Implementations own only transport-specific stream preparation.
pub(crate) trait SyncIo: Read + Write + Send {
    fn prepare_for_sync(&self, timeout: Duration) -> Result<(), CommandError>;
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FileHeader {
    byte_size: u64,
    sha256: String,
}

const MAX_REMOTE_ERROR_CODE_CHARS: usize = 120;
const MAX_REMOTE_ERROR_MESSAGE_CHARS: usize = 1_500;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RemoteErrorFrame {
    code: String,
    message: String,
}

pub(crate) struct SecureChannel {
    stream: Box<dyn SyncIo>,
    send_key: aead::LessSafeKey,
    receive_key: aead::LessSafeKey,
    send_sequence: u64,
    receive_sequence: u64,
}

impl SecureChannel {
    pub(crate) fn new<S>(stream: S, keys: SessionKeys) -> Result<Self, CommandError>
    where
        S: SyncIo + 'static,
    {
        stream.prepare_for_sync(IO_TIMEOUT)?;
        let send_key = aead::UnboundKey::new(&aead::CHACHA20_POLY1305, &keys.send_key)
            .map(aead::LessSafeKey::new)
            .map_err(|_| transport_crypto_error())?;
        let receive_key = aead::UnboundKey::new(&aead::CHACHA20_POLY1305, &keys.receive_key)
            .map(aead::LessSafeKey::new)
            .map_err(|_| transport_crypto_error())?;
        Ok(Self {
            stream: Box::new(stream),
            send_key,
            receive_key,
            send_sequence: 0,
            receive_sequence: 0,
        })
    }

    pub(crate) fn send_json<T: Serialize>(
        &mut self,
        frame_type: u8,
        value: &T,
    ) -> Result<(), CommandError> {
        let bytes = serde_json::to_vec(value).map_err(CommandError::serialization)?;
        if bytes.len() > MAX_JSON_FRAME_BYTES {
            return Err(CommandError::new(
                "device_sync_message_too_large",
                "A synchronization control message exceeds the supported size limit.",
            ));
        }
        self.send_frame(frame_type, &bytes)
    }

    pub(crate) fn receive_json<T: DeserializeOwned>(
        &mut self,
        expected_type: u8,
    ) -> Result<T, CommandError> {
        let (frame_type, bytes) = match self.receive_frame() {
            Ok(frame) => frame,
            Err(error)
                if expected_type == FRAME_COMPLETE
                    && error.code == "device_sync_connection_failed" =>
            {
                return Err(completion_unconfirmed_error());
            }
            Err(error) => return Err(error),
        };
        if frame_type == FRAME_ERROR {
            return Err(decode_remote_error(&bytes));
        }
        if frame_type != expected_type || bytes.len() > MAX_JSON_FRAME_BYTES {
            return Err(protocol_error(
                "The paired device sent an unexpected control message.",
            ));
        }
        serde_json::from_slice(&bytes).map_err(|error| {
            protocol_error(format!("The paired device sent invalid JSON: {error}"))
        })
    }

    pub(crate) fn send_error(&mut self, error: &CommandError) -> Result<(), CommandError> {
        let code = error
            .code
            .chars()
            .take(MAX_REMOTE_ERROR_CODE_CHARS)
            .collect::<String>();
        let message = error
            .message
            .chars()
            .take(MAX_REMOTE_ERROR_MESSAGE_CHARS)
            .collect::<String>();
        let payload = serde_json::to_vec(&RemoteErrorFrame { code, message })
            .map_err(CommandError::serialization)?;
        self.send_frame(FRAME_ERROR, &payload)
    }

    pub(crate) fn send_file(&mut self, source: &Path) -> Result<(), CommandError> {
        let metadata = fs::metadata(source).map_err(CommandError::io)?;
        if metadata.len() == 0 || metadata.len() > MAX_TRANSFER_BYTES {
            return Err(CommandError::new(
                "device_sync_package_too_large",
                "The synchronization package exceeds the supported transfer size.",
            ));
        }
        let sha256 = hash_file(source)?;
        self.send_json(
            FRAME_FILE_HEADER,
            &FileHeader {
                byte_size: metadata.len(),
                sha256,
            },
        )?;
        let mut file = OpenOptions::new()
            .read(true)
            .open(source)
            .map_err(CommandError::io)?;
        let mut buffer = vec![0_u8; FILE_CHUNK_BYTES];
        loop {
            let read = file.read(&mut buffer).map_err(CommandError::io)?;
            if read == 0 {
                break;
            }
            self.send_frame(FRAME_FILE_CHUNK, &buffer[..read])?;
        }
        self.send_frame(FRAME_FILE_END, &[])
    }

    pub(crate) fn receive_file(&mut self, destination: &Path) -> Result<(), CommandError> {
        let header: FileHeader = self.receive_json(FRAME_FILE_HEADER)?;
        if header.byte_size == 0
            || header.byte_size > MAX_TRANSFER_BYTES
            || header.sha256.len() != 64
        {
            return Err(protocol_error(
                "The paired device announced an invalid synchronization package.",
            ));
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(CommandError::io)?;
        }
        match fs::remove_file(destination) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(CommandError::io(error)),
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(CommandError::io)?;
        let mut received = 0_u64;
        let mut hasher = Sha256::new();
        loop {
            let (frame_type, bytes) = self.receive_frame()?;
            match frame_type {
                FRAME_FILE_CHUNK => {
                    received =
                        received.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
                    if received > header.byte_size || received > MAX_TRANSFER_BYTES {
                        let _ = fs::remove_file(destination);
                        return Err(protocol_error(
                            "The paired device exceeded the announced package size.",
                        ));
                    }
                    file.write_all(&bytes).map_err(CommandError::io)?;
                    hasher.update(&bytes);
                }
                FRAME_FILE_END => break,
                FRAME_ERROR => {
                    let _ = fs::remove_file(destination);
                    return Err(decode_remote_error(&bytes));
                }
                _ => {
                    let _ = fs::remove_file(destination);
                    return Err(protocol_error(
                        "The paired device interrupted the package with an unexpected message.",
                    ));
                }
            }
        }
        file.sync_all().map_err(CommandError::io)?;
        if received != header.byte_size || format!("{:x}", hasher.finalize()) != header.sha256 {
            let _ = fs::remove_file(destination);
            return Err(protocol_error(
                "The received synchronization package failed its size or hash check.",
            ));
        }
        Ok(())
    }

    fn send_frame(&mut self, frame_type: u8, payload: &[u8]) -> Result<(), CommandError> {
        if payload.len() + 1 > MAX_ENCRYPTED_FRAME_BYTES {
            return Err(CommandError::new(
                "device_sync_message_too_large",
                "A synchronization frame exceeds the supported size limit.",
            ));
        }
        let sequence = self.send_sequence;
        self.send_sequence = self
            .send_sequence
            .checked_add(1)
            .ok_or_else(|| protocol_error("The synchronization frame sequence overflowed."))?;
        let mut sealed = Vec::with_capacity(payload.len() + 1 + aead::MAX_TAG_LEN);
        sealed.push(frame_type);
        sealed.extend_from_slice(payload);
        self.send_key
            .seal_in_place_append_tag(
                nonce(sequence),
                aead::Aad::from(frame_aad(sequence)),
                &mut sealed,
            )
            .map_err(|_| transport_crypto_error())?;
        let length = u32::try_from(sealed.len()).map_err(|_| {
            CommandError::new(
                "device_sync_message_too_large",
                "A synchronization frame exceeds the transport range.",
            )
        })?;
        let write_result = self
            .stream
            .write_all(&length.to_be_bytes())
            .and_then(|_| self.stream.write_all(&sealed))
            .and_then(|_| self.stream.flush());
        match write_result {
            Ok(()) => Ok(()),
            Err(error) if frame_type == FRAME_COMPLETE => {
                // FRAME_COMPLETE is emitted only after the receiver has
                // durably committed the merge and peer baseline. Losing the
                // physical link while returning that final receipt must not
                // reclassify the already-committed receiver session as failed.
                crate::logging::warn_target(
                    "device_sync.transport",
                    "device_sync_completion_acknowledgement_lost",
                    "The synchronized data was committed, but the completion acknowledgement could not be delivered.",
                    serde_json::json!({ "ioError": error.to_string() }),
                );
                Ok(())
            }
            Err(error) => Err(transport_io_error("write a synchronization frame", error)),
        }
    }

    fn receive_frame(&mut self) -> Result<(u8, Vec<u8>), CommandError> {
        let mut length_bytes = [0_u8; 4];
        self.stream
            .read_exact(&mut length_bytes)
            .map_err(|error| transport_io_error("read a synchronization frame", error))?;
        let length = usize::try_from(u32::from_be_bytes(length_bytes)).unwrap_or(usize::MAX);
        if length <= aead::MAX_TAG_LEN || length > MAX_ENCRYPTED_FRAME_BYTES + aead::MAX_TAG_LEN {
            return Err(protocol_error(
                "The paired device announced an invalid encrypted frame size.",
            ));
        }
        let mut sealed = vec![0_u8; length];
        self.stream.read_exact(&mut sealed).map_err(|error| {
            transport_io_error("read an encrypted synchronization frame", error)
        })?;
        let sequence = self.receive_sequence;
        self.receive_sequence = self
            .receive_sequence
            .checked_add(1)
            .ok_or_else(|| protocol_error("The synchronization frame sequence overflowed."))?;
        let opened = self
            .receive_key
            .open_in_place(
                nonce(sequence),
                aead::Aad::from(frame_aad(sequence)),
                &mut sealed,
            )
            .map_err(|_| {
                CommandError::new(
                    "device_sync_authentication_failed",
                    "The encrypted synchronization frame could not be authenticated.",
                )
            })?;
        let (&frame_type, payload) = opened
            .split_first()
            .ok_or_else(|| protocol_error("The paired device sent an empty encrypted frame."))?;
        Ok((frame_type, payload.to_vec()))
    }
}

pub(crate) fn write_plain_json<T: Serialize, S: SyncIo + ?Sized>(
    stream: &mut S,
    value: &T,
) -> Result<(), CommandError> {
    stream.prepare_for_sync(IO_TIMEOUT)?;
    let bytes = serde_json::to_vec(value).map_err(CommandError::serialization)?;
    if bytes.is_empty() || bytes.len() > MAX_PLAIN_JSON_BYTES {
        return Err(protocol_error(
            "The pairing handshake message exceeds the supported size.",
        ));
    }
    let length = u32::try_from(bytes.len()).map_err(|_| {
        protocol_error("The pairing handshake message exceeds the transport range.")
    })?;
    stream
        .write_all(&length.to_be_bytes())
        .and_then(|_| stream.write_all(&bytes))
        .and_then(|_| stream.flush())
        .map_err(CommandError::io)
}

pub(crate) fn read_plain_json<T: DeserializeOwned, S: SyncIo + ?Sized>(
    stream: &mut S,
) -> Result<T, CommandError> {
    stream.prepare_for_sync(IO_TIMEOUT)?;
    let mut length_bytes = [0_u8; 4];
    stream
        .read_exact(&mut length_bytes)
        .map_err(|error| transport_io_error("read the pairing handshake", error))?;
    let length = usize::try_from(u32::from_be_bytes(length_bytes)).unwrap_or(usize::MAX);
    if length == 0 || length > MAX_PLAIN_JSON_BYTES {
        return Err(protocol_error(
            "The paired device announced an invalid handshake size.",
        ));
    }
    let mut bytes = vec![0_u8; length];
    stream
        .read_exact(&mut bytes)
        .map_err(|error| transport_io_error("read the pairing handshake payload", error))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| protocol_error(format!("The pairing handshake is invalid: {error}")))
}

fn nonce(sequence: u64) -> aead::Nonce {
    let mut bytes = [0_u8; 12];
    bytes[4..].copy_from_slice(&sequence.to_be_bytes());
    aead::Nonce::assume_unique_for_key(bytes)
}

fn frame_aad(sequence: u64) -> Vec<u8> {
    let mut value = b"ostudy-device-sync-v1-frame".to_vec();
    value.extend_from_slice(&sequence.to_be_bytes());
    value
}

fn hash_file(path: &Path) -> Result<String, CommandError> {
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(CommandError::io)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(CommandError::io)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn protocol_error(message: impl Into<String>) -> CommandError {
    CommandError::new("device_sync_protocol_error", message)
}

fn completion_unconfirmed_error() -> CommandError {
    CommandError::new(
        "device_sync_completion_unconfirmed",
        "The selected data was fully sent, but the receiving device disconnected before confirming the committed synchronization. Check the receiving device before retrying.",
    )
}

fn transport_crypto_error() -> CommandError {
    CommandError::new(
        "device_sync_crypto_unavailable",
        "The encrypted synchronization channel could not be initialized.",
    )
}

fn transport_io_error(operation: &str, error: std::io::Error) -> CommandError {
    CommandError::new(
        "device_sync_connection_failed",
        format!("The paired device connection could not {operation}: {error}"),
    )
}

fn decode_remote_error(bytes: &[u8]) -> CommandError {
    if let Ok(remote) = serde_json::from_slice::<RemoteErrorFrame>(bytes) {
        let code = remote.code.trim();
        let message = remote.message.trim();
        if !code.is_empty()
            && code.chars().count() <= MAX_REMOTE_ERROR_CODE_CHARS
            && !message.is_empty()
            && message.chars().count() <= MAX_REMOTE_ERROR_MESSAGE_CHARS
        {
            return CommandError::new(
                "device_sync_remote_error",
                format!("The paired device could not complete synchronization ({code}): {message}"),
            );
        }
    }
    let legacy = String::from_utf8_lossy(bytes);
    let detail = legacy.trim();
    CommandError::new(
        "device_sync_remote_error",
        if detail.is_empty() {
            "The paired device reported an unspecified synchronization failure.".to_string()
        } else {
            format!("The paired device rejected synchronization: {detail}")
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Cursor, Read, Write},
        net::{TcpListener, TcpStream},
        thread,
    };

    struct MemorySyncIo {
        bytes: Cursor<Vec<u8>>,
    }

    impl MemorySyncIo {
        fn new() -> Self {
            Self {
                bytes: Cursor::new(Vec::new()),
            }
        }
    }

    impl Read for MemorySyncIo {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            self.bytes.read(buffer)
        }
    }

    impl Write for MemorySyncIo {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.bytes.write(buffer)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.bytes.flush()
        }
    }

    impl SyncIo for MemorySyncIo {
        fn prepare_for_sync(&self, _timeout: Duration) -> Result<(), CommandError> {
            Ok(())
        }
    }

    struct FailingWriteSyncIo;

    impl Read for FailingWriteSyncIo {
        fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
            Ok(0)
        }
    }

    impl Write for FailingWriteSyncIo {
        fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "simulated disconnected peer",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl SyncIo for FailingWriteSyncIo {
        fn prepare_for_sync(&self, _timeout: Duration) -> Result<(), CommandError> {
            Ok(())
        }
    }

    fn test_keys() -> SessionKeys {
        SessionKeys {
            send_key: [17_u8; 32],
            receive_key: [19_u8; 32],
        }
    }

    #[test]
    fn plain_handshake_framing_is_transport_neutral() {
        let mut stream = MemorySyncIo::new();
        write_plain_json(&mut stream, &"hello over any byte stream")
            .expect("handshake should write");
        stream.bytes.set_position(0);
        let decoded: String = read_plain_json(&mut stream).expect("handshake should read");
        assert_eq!(decoded, "hello over any byte stream");
    }

    #[test]
    fn missing_final_receipt_has_completion_unconfirmed_semantics() {
        let stream = MemorySyncIo::new();
        let mut channel = SecureChannel::new(stream, test_keys()).expect("channel should open");
        let error = channel
            .receive_json::<String>(FRAME_COMPLETE)
            .expect_err("missing final receipt should remain distinguishable from transfer failure");
        assert_eq!(error.code, "device_sync_completion_unconfirmed");
        assert!(error.message.contains("fully sent"));
        assert!(error.message.contains("before retrying"));
    }

    #[test]
    fn committed_receiver_does_not_fail_when_final_receipt_cannot_be_written() {
        let mut channel =
            SecureChannel::new(FailingWriteSyncIo, test_keys()).expect("channel should open");
        channel
            .send_json(FRAME_COMPLETE, &"committed")
            .expect("a lost post-commit receipt must not roll back receiver success");

        let mut non_terminal =
            SecureChannel::new(FailingWriteSyncIo, test_keys()).expect("channel should open");
        let error = non_terminal
            .send_json(FRAME_METADATA, &"not committed")
            .expect_err("ordinary frame write failures must still fail the transfer");
        assert_eq!(error.code, "device_sync_connection_failed");
    }

    #[test]
    fn encrypted_channel_propagates_typed_remote_failure_before_eof() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let address = listener.local_addr().expect("address should resolve");
        let client_keys = SessionKeys {
            send_key: [11_u8; 32],
            receive_key: [13_u8; 32],
        };
        let server_keys = SessionKeys {
            send_key: client_keys.receive_key,
            receive_key: client_keys.send_key,
        };
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("connection should accept");
            let mut channel = SecureChannel::new(stream, server_keys).expect("channel should open");
            channel
                .send_error(&CommandError::new(
                    "device_sync_integrity_failed",
                    "The synchronized library would violate relational integrity.",
                ))
                .expect("typed error should send");
        });
        let stream = TcpStream::connect(address).expect("client should connect");
        let mut channel = SecureChannel::new(stream, client_keys).expect("channel should open");
        let error = channel
            .receive_json::<String>(FRAME_COMPLETE)
            .expect_err("remote failure should be surfaced before EOF");
        assert_eq!(error.code, "device_sync_remote_error");
        assert!(error.message.contains("device_sync_integrity_failed"));
        assert!(error.message.contains("relational integrity"));
        handle.join().expect("server should finish");
    }

    #[test]
    fn encrypted_channel_round_trip_preserves_json_and_file_bytes() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let address = listener.local_addr().expect("address should resolve");
        let keys = SessionKeys {
            send_key: [7_u8; 32],
            receive_key: [9_u8; 32],
        };
        let server_keys = SessionKeys {
            send_key: keys.receive_key,
            receive_key: keys.send_key,
        };
        let root =
            std::env::temp_dir().join(format!("ostudy-sync-transport-{}", std::process::id()));
        let source = root.with_extension("source");
        let destination = root.with_extension("destination");
        fs::write(&source, b"encrypted sync payload").expect("source should write");
        let server_destination = destination.clone();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("connection should accept");
            let mut channel = SecureChannel::new(stream, server_keys).expect("channel should open");
            let message: String = channel
                .receive_json(FRAME_METADATA)
                .expect("message should receive");
            assert_eq!(message, "hello");
            channel
                .receive_file(&server_destination)
                .expect("file should receive");
            channel
                .send_json(FRAME_COMPLETE, &"done")
                .expect("completion should send");
        });
        let stream = TcpStream::connect(address).expect("client should connect");
        let mut channel = SecureChannel::new(stream, keys).expect("channel should open");
        channel
            .send_json(FRAME_METADATA, &"hello")
            .expect("message should send");
        channel.send_file(&source).expect("file should send");
        let result: String = channel
            .receive_json(FRAME_COMPLETE)
            .expect("completion should receive");
        assert_eq!(result, "done");
        handle.join().expect("server should finish");
        assert_eq!(
            fs::read(&destination).expect("destination should read"),
            b"encrypted sync payload"
        );
        let _ = fs::remove_file(source);
        let _ = fs::remove_file(destination);
    }
}
