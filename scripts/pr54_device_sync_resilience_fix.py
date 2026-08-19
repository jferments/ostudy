#!/usr/bin/env python3
"""One-shot PR #54 fix for Android sync failure propagation and optional credentials."""
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    p = Path(path)
    value = p.read_text()
    if new in value:
        return
    count = value.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor in {path}, found {count}")
    p.write_text(value.replace(old, new, 1))


# ---------------------------------------------------------------------------
# Encrypted transport failures: once a secure channel exists, propagate a safe
# typed error rather than making the peer infer failure from EOF.
# ---------------------------------------------------------------------------
path = "src-tauri/src/device_sync/transport.rs"
replace_once(
    path,
    '''struct FileHeader {
    byte_size: u64,
    sha256: String,
}
''',
    '''struct FileHeader {
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
''',
    "typed remote error frame",
)
replace_once(
    path,
    '''        if frame_type == FRAME_ERROR {
            let detail = String::from_utf8_lossy(&bytes);
            return Err(CommandError::new(
                "device_sync_remote_error",
                format!("The paired device rejected synchronization: {detail}"),
            ));
        }''',
    '''        if frame_type == FRAME_ERROR {
            return Err(decode_remote_error(&bytes));
        }''',
    "receive-json remote error decoding",
)
replace_once(
    path,
    '''    pub(crate) fn send_file(&mut self, source: &Path) -> Result<(), CommandError> {''',
    '''    pub(crate) fn send_error(&mut self, error: &CommandError) -> Result<(), CommandError> {
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

    pub(crate) fn send_file(&mut self, source: &Path) -> Result<(), CommandError> {''',
    "secure-channel send-error method",
)
replace_once(
    path,
    '''                FRAME_ERROR => {
                    let _ = fs::remove_file(destination);
                    return Err(CommandError::new(
                        "device_sync_remote_error",
                        String::from_utf8_lossy(&bytes).into_owned(),
                    ));
                }''',
    '''                FRAME_ERROR => {
                    let _ = fs::remove_file(destination);
                    return Err(decode_remote_error(&bytes));
                }''',
    "receive-file remote error decoding",
)
replace_once(
    path,
    '''fn transport_io_error(operation: &str, error: std::io::Error) -> CommandError {
    CommandError::new(
        "device_sync_connection_failed",
        format!("The paired device connection could not {operation}: {error}"),
    )
}
''',
    '''fn transport_io_error(operation: &str, error: std::io::Error) -> CommandError {
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
''',
    "remote error decoding helper",
)
replace_once(
    path,
    '''    #[test]
    fn encrypted_channel_round_trip_preserves_json_and_file_bytes() {''',
    '''    #[test]
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
    fn encrypted_channel_round_trip_preserves_json_and_file_bytes() {''',
    "typed remote error regression",
)

# ---------------------------------------------------------------------------
# Receiver commit semantics: report post-auth failures through FRAME_ERROR and
# do not turn an optional secure-credential copy failure into a failed library
# synchronization after the study data was already merged.
# ---------------------------------------------------------------------------
path = "src-tauri/src/device_sync/mod.rs"
replace_once(
    path,
    '''    pub transferred_credential_count: usize,
    pub sent_package_bytes: u64,''',
    '''    pub transferred_credential_count: usize,
    pub credential_warning: Option<String>,
    pub sent_package_bytes: u64,''',
    "sync result credential warning",
)
replace_once(
    path,
    '''    let mut channel = SecureChannel::new(stream, keys)?;
    let server: ServerHello = channel.receive_json(FRAME_METADATA)?;''',
    '''    let mut channel = SecureChannel::new(stream, keys)?;
    let receiver_session_id = invitation.session_id.clone();
    let transfer_result = (|| -> Result<DeviceSyncResultDto, CommandError> {
    let server: ServerHello = channel.receive_json(FRAME_METADATA)?;''',
    "receiver protected transfer boundary",
)
replace_once(
    path,
    '''    let transferred_credential_count = if server.credentials_included {
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
}''',
    '''    let (transferred_credential_count, credential_warning) = if server.credentials_included {
        match credentials::apply_credentials(
            database.inner(),
            runtime.secure_secret_store(),
            &metadata.credentials,
        ) {
            Ok(count) => (count, None),
            Err(error) => {
                crate::logging::warn_target(
                    "device_sync.credentials",
                    "device_sync_credentials_not_applied",
                    "Study-library synchronization completed, but optional saved API credentials could not be stored on the receiving device.",
                    serde_json::json!({
                        "session": invitation.session_id.as_str(),
                        "sender": sender.id.as_str(),
                        "code": error.code.as_str(),
                    }),
                );
                (
                    0,
                    Some(
                        "Study data synchronized, but saved API credentials could not be stored securely on this device. Re-enter them in Settings → AI if needed."
                            .to_string(),
                    ),
                )
            }
        }
    } else {
        (0, None)
    };
    cancel_signal.check()?;
    store.mark_peer_synced(&sender.id, &invitation.endpoint)?;
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
    Ok(DeviceSyncResultDto {
        session_id: invitation.session_id,
        peer: sender,
        changed_row_count: merge.changed_row_count,
        conflict_count: merge.conflict_count,
        transferred_credential_count,
        credential_warning,
        sent_package_bytes: 0,
        received_package_bytes: metadata.package.package_bytes,
    })
    })();
    if let Err(error) = &transfer_result {
        crate::logging::error_target(
            "device_sync.receive",
            "device_sync_receiver_transfer_failed",
            "The receiving device could not complete an authenticated synchronization transfer.",
            serde_json::json!({
                "session": receiver_session_id,
                "code": error.code.as_str(),
            }),
        );
        // Once authentication has established the encrypted channel, send the
        // peer the safe typed failure before closing. A broken socket may make
        // this best-effort notification fail; the original local error wins.
        let _ = channel.send_error(error);
    }
    transfer_result
}''',
    "receiver failure propagation and credential warning",
)

# ---------------------------------------------------------------------------
# IPC/UI: distinguish a successful library merge with an optional credential
# warning from a failed synchronization.
# ---------------------------------------------------------------------------
path = "src/ipc/deviceSync.ts"
replace_once(
    path,
    '''  transferredCredentialCount: number;
  sentPackageBytes: number;''',
    '''  transferredCredentialCount: number;
  credentialWarning: string | null;
  sentPackageBytes: number;''',
    "frontend sync result warning",
)

path = "src/screens/settings/DeviceSyncSettings.tsx"
replace_once(
    path,
    '''    showToast(
      result.conflictCount > 0 ? "info" : "success",
      `Received from ${result.peer.displayName}: ${result.changedRowCount} rows changed${
        result.conflictCount > 0 ? ` and ${result.conflictCount} conflicts need review` : ""
      }.${credentialCopy}`,
    );''',
    '''    const credentialWarning = result.credentialWarning ? ` ${result.credentialWarning}` : "";
    showToast(
      result.conflictCount > 0 || result.credentialWarning ? "info" : "success",
      `Received from ${result.peer.displayName}: ${result.changedRowCount} rows changed${
        result.conflictCount > 0 ? ` and ${result.conflictCount} conflicts need review` : ""
      }.${credentialCopy}${credentialWarning}`,
    );''',
    "credential warning UI",
)

path = "src/screens/settings/DeviceSyncSettings.test.tsx"
replace_once(
    path,
    '''      transferredCredentialCount: 0,
      sentPackageBytes: 0,''',
    '''      transferredCredentialCount: 0,
      credentialWarning: null,
      sentPackageBytes: 0,''',
    "sync test result fixture",
)
replace_once(
    path,
    '''  it("accepts pasted invitation text without a receiver-side API toggle", async () => {''',
    '''  it("reports optional credential-store failure without failing the received library", async () => {
    vi.mocked(syncIpc.joinDevicePairing).mockResolvedValueOnce({
      sessionId: "5".repeat(32),
      peer: state().peers[0],
      changedRowCount: 12,
      conflictCount: 0,
      transferredCredentialCount: 0,
      credentialWarning:
        "Study data synchronized, but saved API credentials could not be stored securely on this device. Re-enter them in Settings → AI if needed.",
      sentPackageBytes: 0,
      receivedPackageBytes: 20,
    });
    const showToast = renderSync();
    await screen.findByText("Android phone");
    fireEvent.change(screen.getByLabelText("Synchronization invitation"), {
      target: { value: "ostudy-sync://pair/credential-warning" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Accept sync offer" }));

    await waitFor(() => expect(syncIpc.joinDevicePairing).toHaveBeenCalledTimes(1));
    expect(showToast).toHaveBeenCalledWith(
      "info",
      expect.stringContaining("Study data synchronized, but saved API credentials could not be stored securely"),
    );
  });

  it("accepts pasted invitation text without a receiver-side API toggle", async () => {''',
    "credential warning UI regression",
)

# ---------------------------------------------------------------------------
# Android diagnostics: Rust structured events are in the app-private JSONL log,
# not Android's system logcat. Provide one maintained command to retrieve them
# from the debuggable package during physical-device testing.
# ---------------------------------------------------------------------------
path = "package.json"
replace_once(
    path,
    '''    "android:doctor:emulator": "bash ./scripts/android-doctor.sh --emulator",
    "android:runner:doctor":''',
    '''    "android:doctor:emulator": "bash ./scripts/android-doctor.sh --emulator",
    "android:diagnostics": "bash ./scripts/android-diagnostics.sh",
    "android:runner:doctor":''',
    "Android diagnostics package script",
)

path = "docs/installation/android.md"
replace_once(
    path,
    '''### A document provider import fails

Retry with a local Downloads copy and inspect Logcat.''',
    '''### Device synchronization fails after QR detection

Android system Logcat is useful for native camera/Activity crashes, but ostudy's Rust synchronization diagnostics are written to the debug app's private structured JSONL log. With the phone connected, retrieve the current log with:

```bash
pnpm android:diagnostics
```

To select one phone when several ADB devices are connected:

```bash
bash ./scripts/android-diagnostics.sh --serial <adb-serial>
```

The command uses Android `run-as` and therefore targets the debuggable package `io.github.jferments.ostudy.dev`. It prints the private `diagnostics/ostudy.jsonl` without copying credentials, document text, prompts, or other data excluded by the diagnostic redaction boundary.

After the encrypted pairing handshake, a receiver-side synchronization failure is returned to the sender as a typed encrypted error rather than appearing only as an EOF/`failed to fill whole buffer`. Optional API-credential storage failure does not invalidate an already committed study-library merge; the receiver completes library synchronization and reports that credentials need to be entered again.

### A document provider import fails

Retry with a local Downloads copy and inspect Logcat.''',
    "Android sync troubleshooting documentation",
)

print("PR #54 Android sync resilience changes applied.")
