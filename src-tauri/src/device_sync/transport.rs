use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    net::TcpStream,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FileHeader {
    byte_size: u64,
    sha256: String,
}

pub(crate) struct SecureChannel {
    stream: TcpStream,
    send_key: aead::LessSafeKey,
    receive_key: aead::LessSafeKey,
    send_sequence: u64,
    receive_sequence: u64,
}

impl SecureChannel {
    pub(crate) fn new(stream: TcpStream, keys: SessionKeys) -> Result<Self, CommandError> {
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .map_err(CommandError::io)?;
        stream
            .set_write_timeout(Some(IO_TIMEOUT))
            .map_err(CommandError::io)?;
        stream.set_nodelay(true).map_err(CommandError::io)?;
        let send_key = aead::UnboundKey::new(&aead::CHACHA20_POLY1305, &keys.send_key)
            .map(aead::LessSafeKey::new)
            .map_err(|_| transport_crypto_error())?;
        let receive_key = aead::UnboundKey::new(&aead::CHACHA20_POLY1305, &keys.receive_key)
            .map(aead::LessSafeKey::new)
            .map_err(|_| transport_crypto_error())?;
        Ok(Self {
            stream,
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
        let (frame_type, bytes) = self.receive_frame()?;
        if frame_type == FRAME_ERROR {
            let detail = String::from_utf8_lossy(&bytes);
            return Err(CommandError::new(
                "device_sync_remote_error",
                format!("The paired device rejected synchronization: {detail}"),
            ));
        }
        if frame_type != expected_type || bytes.len() > MAX_JSON_FRAME_BYTES {
            return Err(protocol_error("The paired device sent an unexpected control message."));
        }
        serde_json::from_slice(&bytes).map_err(|error| {
            protocol_error(format!("The paired device sent invalid JSON: {error}"))
        })
    }

    pub(crate) fn send_error(&mut self, message: &str) {
        let safe = message
            .chars()
            .filter(|character| !character.is_control() || *character == ' ')
            .take(500)
            .collect::<String>();
        let _ = self.send_frame(FRAME_ERROR, safe.as_bytes());
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
                    received = received.saturating_add(
                        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                    );
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
                    return Err(CommandError::new(
                        "device_sync_remote_error",
                        String::from_utf8_lossy(&bytes).into_owned(),
                    ));
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
        self.stream
            .write_all(&length.to_be_bytes())
            .and_then(|_| self.stream.write_all(&sealed))
            .and_then(|_| self.stream.flush())
            .map_err(CommandError::io)
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
        self.stream
            .read_exact(&mut sealed)
            .map_err(|error| transport_io_error("read an encrypted synchronization frame", error))?;
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
        let (&frame_type, payload) = opened.split_first().ok_or_else(|| {
            protocol_error("The paired device sent an empty encrypted frame.")
        })?;
        Ok((frame_type, payload.to_vec()))
    }
}

pub(crate) fn write_plain_json<T: Serialize>(
    stream: &mut TcpStream,
    value: &T,
) -> Result<(), CommandError> {
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(CommandError::io)?;
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

pub(crate) fn read_plain_json<T: DeserializeOwned>(
    stream: &mut TcpStream,
) -> Result<T, CommandError> {
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(CommandError::io)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, thread};

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
        let root = std::env::temp_dir().join(format!(
            "ostudy-sync-transport-{}",
            std::process::id()
        ));
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
