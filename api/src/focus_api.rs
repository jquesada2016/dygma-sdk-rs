//! Low-level functions for interacting with the Focus API provided with Dygma
//! firmware.

use crate::{
    parsing,
    serial_port::{OpenSerialPortError, SerialPort},
};
use async_hid::{AsyncHidRead, AsyncHidWrite};
use bytes::Bytes;
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;

/// Error returned when running commands.
#[derive(Debug, Display, Error)]
pub enum SerialPortRunCommandError {
    /// We hit an error sending the command to the device.
    #[display("Failed to send command: {_0}")]
    SendingCommand(std::io::Error),
    /// We were not able to receive a response from the device.
    #[display("Failed to receive response: {_0}")]
    RecievingResponse(std::io::Error),
    /// The response from the device could not be interpreted.
    #[display("received an unexpected response:\n{_0}")]
    UnexpectedResponse(parsing::focus_api::TryFromResponseError),
    /// The response stream completed before a response could be interpreted.
    #[display("response stream terminated while waiting for the response to complete")]
    ResponseStreamTerminatedPrematurely,
}

/// Error while getting a list of available commands supported by the device.
#[derive(Debug, Display, From, Error)]
#[display("failed to retrieve available commands with the `help command: {_0}")]
pub struct GetCommandsError(HidRunCommandError);

/// Error creating Focus API struct.
#[derive(Debug, Display, Error, From)]
#[display("failed to create FocusAPI: {_0}")]
pub struct CreateSerialPortFocusApiError(OpenSerialPortError);

/// Error returned when creating a [`HidFocusApi`].
#[derive(Debug, Display, Error)]
pub enum CreateHidFoducApiError {
    #[display("failed to enumerate HID devices: [_0]")]
    /// Something went wrong enumerating available HID devices.
    EnumeratingFailure(async_hid::HidError),
    /// The requested device could not be found.
    #[display("device not found")]
    DeviceNotFound,
    /// There was a problem connecting to the requested device.
    #[display("failed to connect to the device")]
    ConnectingToDevice(async_hid::HidError),
}

/// error returned when running commands.
#[derive(Debug, Display, Error)]
pub enum HidRunCommandError {
    /// Something went wrong sending the command to the device.
    SendingCommand(async_hid::HidError),
    /// Something went wrong receiving data from the device.
    RecievingResponse(async_hid::HidError),
    /// The response from the device could not be interpreted.
    #[display("received an unexpected response:\n{_0}")]
    UnexpectedResponse(parsing::focus_api::TryFromResponseError),
}

/// Abstracts over a serial port connection to provide the firmware's
/// Focus API, which is used for controlling the keyboard.
#[derive(Debug)]
pub struct SerialPortFocusApi(SerialPort);

impl SerialPortFocusApi {
    const MANUFACTURER_NAME: &str = "DYGMA";

    /// Creates a Focus API instance.
    pub async fn new(
        product_name: &str,
        baud_rate: u32,
    ) -> Result<Self, CreateSerialPortFocusApiError> {
        let sp = SerialPort::connect(Self::MANUFACTURER_NAME, product_name, baud_rate).await?;

        Ok(Self(sp))
    }

    /// Executes commands and returns their response.
    pub async fn run_command(
        &mut self,
        command: &str,
        data: Option<&str>,
    ) -> Result<String, SerialPortRunCommandError> {
        let port = &mut self.0;

        let mut write = async |str: &str| {
            port.write_all(str.as_bytes())
                .await
                .map_err(SerialPortRunCommandError::SendingCommand)
        };

        write(command).await?;

        if let Some(data) = data {
            write(" ").await?;
            write(data).await?;
        }

        write("\n").await?;

        let mut stream = ReaderStream::new(port);

        let mut buf = Bytes::new();

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res.map_err(SerialPortRunCommandError::RecievingResponse)?;

            buf = Bytes::from([buf.as_ref(), chunk.as_ref()].concat());

            let data = buf.as_ref();
            let data = match str::from_utf8(data) {
                Ok(data) => data,
                Err(err) => {
                    debug!(
                        "data received is not utf8, retrying when more data is available\
                        \n{err}"
                    );
                    continue;
                }
            };

            return match parsing::focus_api::Response::try_from(data).map(|res| res.0) {
                Ok(res) => Ok(res),
                Err(parsing::focus_api::TryFromResponseError::Incomplete) => continue,
                Err(err) => return Err(SerialPortRunCommandError::UnexpectedResponse(err)),
            };
        }

        Err(SerialPortRunCommandError::ResponseStreamTerminatedPrematurely)
    }
}

/// Abstracts over a HID connection to provide the firmware's
/// Focus API, which is used for controlling the keyboard.
///
/// Note that this works also over BTLE.
#[derive(Debug)]
pub struct HidFocusApi {
    _device: async_hid::Device,
    #[debug(ignore)]
    reader: async_hid::DeviceReader,
    #[debug(ignore)]
    writer: async_hid::DeviceWriter,
}

impl HidFocusApi {
    const VENDOR_ID: u16 = 13807;
    const USAGE_ID: u16 = 1;
    const USAGE_PAGE: u16 = 65280;
    const REPORT_ID: u8 = 5;
    const MAX_SEND_SIZE: usize = 200;

    /// Opens a connection to the requested Dygma device.
    pub async fn new(product_id: u16) -> Result<Self, CreateHidFoducApiError> {
        let backend = async_hid::HidBackend::default();

        let device_stream = backend
            .enumerate()
            .await
            .map_err(CreateHidFoducApiError::EnumeratingFailure)?
            .filter(|device| {
                device.matches(
                    Self::USAGE_PAGE,
                    Self::USAGE_ID,
                    Self::VENDOR_ID,
                    product_id,
                )
            });

        futures::pin_mut!(device_stream);

        let device = device_stream
            .next()
            .await
            .ok_or(CreateHidFoducApiError::DeviceNotFound)?;

        let (reader, writer) = device
            .open()
            .await
            .map_err(CreateHidFoducApiError::ConnectingToDevice)?;

        Ok(Self {
            _device: device,
            reader,
            writer,
        })
    }

    /// Executes commands and returns their response.
    pub async fn run_command(
        &mut self,
        command: &str,
        data: Option<&str>,
    ) -> Result<String, HidRunCommandError> {
        let data_to_send = if let Some(data) = data {
            [
                command.as_bytes(),
                " ".as_bytes(),
                data.as_bytes(),
                "\n".as_bytes(),
            ]
            .concat()
        } else {
            [command.as_bytes(), "\n".as_bytes()].concat()
        };

        for chunk in data_to_send.chunks(Self::MAX_SEND_SIZE) {
            let data = [&[Self::REPORT_ID], chunk].concat();

            self.writer
                .write_output_report(data.as_slice())
                .await
                .map_err(HidRunCommandError::SendingCommand)?;
        }

        let mut res = Vec::new();

        // We need MAX_SEND_SIZE + 1 because of the leading report id byte
        let mut buf = [0; Self::MAX_SEND_SIZE + 1];

        loop {
            let bytes_read = self
                .reader
                .read_input_report(&mut buf)
                .await
                .map_err(HidRunCommandError::RecievingResponse)?;

            res.extend_from_slice(&buf[..bytes_read]);

            let str_res = match str::from_utf8(res.as_ref()) {
                Ok(data) => data,
                Err(err) => {
                    debug!(
                        "data received is not utf8, retrying when more data is available\
                        \n{err}"
                    );
                    continue;
                }
            };

            return match parsing::focus_api::Response::try_from(str_res).map(|res| res.0) {
                Ok(res) => Ok(res),
                Err(parsing::focus_api::TryFromResponseError::Incomplete) => continue,
                Err(err) => return Err(HidRunCommandError::UnexpectedResponse(err)),
            };
        }
    }

    /// Gets a list of available commands on the device.
    pub async fn available_commands(&mut self) -> Result<Vec<String>, GetCommandsError> {
        let cmds = self
            .run_command("help", None)
            .await?
            .lines()
            .map(ToOwned::to_owned)
            .collect();

        Ok(cmds)
    }
}
