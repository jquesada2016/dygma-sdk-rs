//! This module defines the [`SerialPort`] type, which is used to abstract over
//! platform specific implementation, allowing to share a common interface on
//! both native and WASM targets.

use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_serial::{SerialPortBuilderExt, SerialPortType, UsbPortInfo};

/// Error opening serial port.
#[derive(Debug, Display, Error)]
pub enum OpenSerialPortError {
    /// We were unable to get a list of available serial ports.
    #[display("failed to enumerate serial port devices")]
    EnumeratingDevices(tokio_serial::Error),
    /// Could not find the device with the given manufacturer and product name.
    #[display("the device with the provided manufacturer and product name was not found")]
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
    pub async fn connect(
        manufacturer_name: &str,
        product_name: &str,
        baud_rate: u32,
    ) -> Result<SerialPort, OpenSerialPortError> {
        let port_name = tokio_serial::available_ports()
            .map_err(OpenSerialPortError::EnumeratingDevices)?
            .into_iter()
            .filter(|info| {
                matches!(
                      &info.port_type,
                    SerialPortType::UsbPort(UsbPortInfo {
                        manufacturer: Some(manufacturer),
                        product: Some(product),
                        ..
                    }) if manufacturer == manufacturer_name  && product ==  product_name
                )
            })
            .map(|info| info.port_name)
            .next()
            .ok_or(OpenSerialPortError::DeviceNotFound)?;

        let port = tokio_serial::new(&port_name, baud_rate)
            .timeout(std::time::Duration::from_secs(5))
            .open_native_async()
            .map_err(OpenSerialPortError::OpeningPort)?;

        #[cfg(target_os = "macos")]
        tokio::process::Command::new("stty")
            .args(["-f", &port_name, "clocal"])
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(OpenSerialPortError::ConfiguringPort)?
            .wait()
            .await
            .map_err(OpenSerialPortError::ConfiguringPort)?;

        Ok(SerialPort(port))
    }
}
