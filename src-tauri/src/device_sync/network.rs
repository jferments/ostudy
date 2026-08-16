use std::{
    io,
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    time::Duration,
};

use crate::error::CommandError;

pub(crate) const LISTENER_PORT_START: u16 = 49_251;
pub(crate) const LISTENER_PORT_END: u16 = 49_258;
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) fn bind_listener() -> Result<TcpListener, CommandError> {
    let mut last_in_use = None;
    for port in LISTENER_PORT_START..=LISTENER_PORT_END {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listener_range_is_small_stable_and_in_the_private_dynamic_port_space() {
        assert!(LISTENER_PORT_START >= 49_152);
        assert!(LISTENER_PORT_END >= LISTENER_PORT_START);
        assert_eq!(LISTENER_PORT_END - LISTENER_PORT_START + 1, 8);
    }
}
