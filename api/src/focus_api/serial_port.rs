//! This module defines the [`SerialPort`] type, which is used to abstract over
//! platform specific implementation, allowing to share a common interface on
//! both native and WASM targets.

use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_serial::{SerialPortBuilderExt, SerialPortInfo, SerialPortType, UsbPortInfo};

use super::identities::DeviceIdentity;

/// Error opening serial port.
#[derive(Debug, Display, Error)]
pub enum OpenSerialPortError {
    /// We were unable to get a list of available serial ports.
    #[display("failed to enumerate serial port devices")]
    EnumeratingDevices(tokio_serial::Error),
    /// Could not find a serial port whose USB (vendor id, product id)
    /// pair matches one of the requested device identities.
    #[display("no serial port carrying one of the requested device identities was found")]
    DeviceNotFound,
    /// We were unable to open the actual serial port.
    #[display("device was found, but failed to open serial port: {_0}")]
    OpeningPort(tokio_serial::Error),
    /// On macOS, we were unable to use `stty` to configure the serial port to be `clocal`.
    #[display("failed to configure serial port as clocal using `stty`")]
    ConfiguringPort(std::io::Error),
}

/// Serial port connection.
#[derive(Debug)]
#[pin_project]
#[cfg(not(target_arch = "wasm32"))]
pub struct SerialPort(#[pin] tokio_serial::SerialStream);

impl AsyncRead for SerialPort {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.project();

        this.0.poll_read(cx, buf)
    }
}

impl AsyncWrite for SerialPort {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let this = self.project();

        this.0.poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.project();

        this.0.poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.project();

        this.0.poll_shutdown(cx)
    }
}

impl SerialPort {
    /// Searches for and opens a serial port with the device.
    ///
    /// Ports are matched by their USB (vendor id, product id) pair —
    /// one of `identities` — the way Bazecor's serial scan matches
    /// them; the manufacturer/product strings the ports carry are
    /// never consulted (Bazecor does not match on them, and they are
    /// not reliably populated on every platform). The first matching
    /// port wins when several are attached, and the identity that
    /// matched is returned alongside the port so the caller knows
    /// which product it opened.
    pub async fn connect(
        identities: &[DeviceIdentity],
        baud_rate: u32,
    ) -> Result<(SerialPort, DeviceIdentity), OpenSerialPortError> {
        let ports =
            tokio_serial::available_ports().map_err(OpenSerialPortError::EnumeratingDevices)?;

        let (info, identity) = ports
            .iter()
            .find_map(|info| matched_identity(info, identities).map(|id| (info, id)))
            .ok_or(OpenSerialPortError::DeviceNotFound)?;

        let port_name = info.port_name.as_str();

        let port = tokio_serial::new(port_name, baud_rate)
            .timeout(std::time::Duration::from_secs(5))
            .open_native_async()
            .map_err(OpenSerialPortError::OpeningPort)?;

        #[cfg(target_os = "macos")]
        tokio::process::Command::new("stty")
            .args(["-f", port_name, "clocal"])
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(OpenSerialPortError::ConfiguringPort)?
            .wait()
            .await
            .map_err(OpenSerialPortError::ConfiguringPort)?;

        Ok((SerialPort(port), identity))
    }
}

/// The identity of the first requested one this port carries, or
/// `None` when the port is not one of them: non-USB ports never match,
/// and USB ports match on the exact (vendor id, product id) pair —
/// Bazecor's serial scan
/// (`src/api/comms/serial/SerialAPI.ts`, `enumerate`) does the same,
/// and never consults the port's manufacturer/product strings.
fn matched_identity(
    info: &SerialPortInfo,
    identities: &[DeviceIdentity],
) -> Option<DeviceIdentity> {
    let SerialPortType::UsbPort(UsbPortInfo { vid, pid, .. }) = &info.port_type else {
        return None;
    };

    identities
        .iter()
        .find(|identity| identity.matches(*vid, *pid))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usb_port(vendor_id: u16, product_id: u16) -> SerialPortInfo {
        SerialPortInfo {
            port_name: "/dev/ttyUSB0".to_owned(),
            port_type: SerialPortType::UsbPort(UsbPortInfo {
                vid: vendor_id,
                pid: product_id,
                serial_number: None,
                manufacturer: None,
                product: None,
            }),
        }
    }

    #[test]
    fn a_usb_port_matches_by_the_exact_identity_pair() {
        let port = usb_port(super::super::identities::VENDOR_DYGMA, 18);

        assert_eq!(
            matched_identity(&port, &super::super::identities::SERIAL_IDENTITIES),
            Some(super::super::identities::DEFY_WIRELESS)
        );
    }

    #[test]
    fn a_port_with_an_unknown_pair_matches_nothing() {
        // A Dygma vendor id with an unregistered product id: not one of
        // ours, not matchable.
        let port = usb_port(super::super::identities::VENDOR_DYGMA, 0x9999);

        assert_eq!(
            matched_identity(&port, &super::super::identities::SERIAL_IDENTITIES),
            None
        );
    }

    #[test]
    fn a_port_from_another_vendor_matches_nothing() {
        let port = usb_port(0x1234, 18);

        assert_eq!(
            matched_identity(&port, &super::super::identities::SERIAL_IDENTITIES),
            None
        );
    }

    #[test]
    fn non_usb_ports_never_match() {
        let bluetooth = SerialPortInfo {
            port_name: "/dev/rfcomm0".to_owned(),
            port_type: SerialPortType::BluetoothPort,
        };
        let unknown = SerialPortInfo {
            port_name: "/dev/ttyS0".to_owned(),
            port_type: SerialPortType::Unknown,
        };

        for info in [&bluetooth, &unknown] {
            assert_eq!(
                matched_identity(info, &super::super::identities::SERIAL_IDENTITIES),
                None
            );
        }
    }

    #[test]
    fn every_serial_identity_is_matchable_through_the_table() {
        for identity in super::super::identities::SERIAL_IDENTITIES {
            let port = usb_port(identity.vendor_id, identity.product_id);

            assert_eq!(
                matched_identity(&port, &super::super::identities::SERIAL_IDENTITIES),
                Some(identity)
            );
        }
    }
}
