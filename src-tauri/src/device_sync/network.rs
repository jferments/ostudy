use std::{
    io,
    net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream},
    ops::RangeInclusive,
    sync::Arc,
    time::Duration,
};

use crate::error::CommandError;

use super::{
    cancellation::{SyncAbort, SyncAbortSource},
    transport::SyncIo,
};

pub(crate) const LISTENER_PORT_START: u16 = 49_251;
pub(crate) const LISTENER_PORT_END: u16 = 49_258;
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

fn listener_ports() -> RangeInclusive<u16> {
    LISTENER_PORT_START..=LISTENER_PORT_END
}

#[derive(Debug)]
struct TcpStreamAbort {
    stream: TcpStream,
}

impl SyncAbort for TcpStreamAbort {
    fn abort(&self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

impl SyncAbortSource for TcpStream {
    fn sync_abort_handle(&self) -> Result<Arc<dyn SyncAbort>, CommandError> {
        Ok(Arc::new(TcpStreamAbort {
            stream: self.try_clone().map_err(CommandError::io)?,
        }))
    }
}

impl SyncIo for TcpStream {
    fn prepare_for_sync(&self, timeout: Duration) -> Result<(), CommandError> {
        self.set_read_timeout(Some(timeout))
            .map_err(CommandError::io)?;
        self.set_write_timeout(Some(timeout))
            .map_err(CommandError::io)?;
        self.set_nodelay(true).map_err(CommandError::io)
    }
}

pub(crate) fn bind_listener() -> Result<TcpListener, CommandError> {
    let mut last_in_use = None;
    for port in listener_ports() {
        match TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)) {
            Ok(listener) => {
                listener.set_nonblocking(true).map_err(CommandError::io)?;
                return Ok(listener);
            }
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                last_in_use = Some(error);
            }
            Err(error) => {
                return Err(CommandError::new(
                    "device_sync_listener_failed",
                    format!(
                        "ostudy could not open TCP port {port} for local device synchronization: {error}"
                    ),
                ));
            }
        }
    }

    let suffix = last_in_use
        .map(|error| format!(" Last bind error: {error}"))
        .unwrap_or_default();
    Err(CommandError::new(
        "device_sync_listener_ports_busy",
        format!(
            "All ostudy synchronization ports {LISTENER_PORT_START}-{LISTENER_PORT_END} are already in use. Finish or cancel active synchronization requests, close duplicate ostudy processes, and try again.{suffix}"
        ),
    ))
}

pub(crate) fn connect_to_sender(
    address: SocketAddr,
    endpoint: &str,
) -> Result<TcpStream, CommandError> {
    TcpStream::connect_timeout(&address, CONNECT_TIMEOUT).map_err(|error| {
        CommandError::new(
            "device_sync_connection_failed",
            format!(
                "The sending device did not accept a local-network connection at {endpoint} within {} seconds. The QR invitation was read successfully, but the TCP path is blocked or unreachable. Confirm both devices are on the same LAN and allow inbound TCP ports {LISTENER_PORT_START}-{LISTENER_PORT_END} through the sending device's firewall. Network error: {error}",
                CONNECT_TIMEOUT.as_secs()
            ),
        )
    })
}

// `execute_join_session` has one cross-platform type shape. Non-Android builds
// can never construct the USB branch, but the compiler still type-checks it.
// This unreachable stub supplies that type boundary without introducing USB
// behavior into the network adapter or a fake runtime fallback.
#[cfg(not(target_os = "android"))]
mod unavailable_usb_join_stub {
    use std::{
        io::{self, Read, Write},
        sync::Arc,
        time::Duration,
    };

    use crate::error::CommandError;

    use super::super::{
        cancellation::{SyncAbort, SyncAbortSource},
        transport::SyncIo,
        UsbJoinStream,
    };

    #[derive(Debug)]
    struct UnavailableUsbAbort;

    impl SyncAbort for UnavailableUsbAbort {
        fn abort(&self) {}
    }

    impl Read for UsbJoinStream {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Android USB join streams do not exist on this platform.",
            ))
        }
    }

    impl Write for UsbJoinStream {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Android USB join streams do not exist on this platform.",
            ))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl SyncIo for UsbJoinStream {
        fn prepare_for_sync(&self, _timeout: Duration) -> Result<(), CommandError> {
            Err(CommandError::new(
                "device_sync_usb_unavailable",
                "Android USB join streams do not exist on this platform.",
            ))
        }
    }

    impl SyncAbortSource for UsbJoinStream {
        fn sync_abort_handle(&self) -> Result<Arc<dyn SyncAbort>, CommandError> {
            Ok(Arc::new(UnavailableUsbAbort))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listener_range_is_small_stable_and_in_the_private_dynamic_port_space() {
        let ports = listener_ports().collect::<Vec<_>>();
        assert_eq!(ports, (49_251..=49_258).collect::<Vec<_>>());
        assert_eq!(ports.len(), 8);
    }
}
