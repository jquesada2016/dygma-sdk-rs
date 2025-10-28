//! Functions for interacting with the Focus API provided with Dygma
//! firmware.

pub mod parsing;
mod serial_port;

use async_hid::{AsyncHidRead, AsyncHidWrite};
use bytes::Bytes;
use serial_port::{OpenSerialPortError, SerialPort};
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;

use crate::focus_api::parsing::focus_api::serialize_command;

/// Trait used to abstract over focus API connections.
#[allow(async_fn_in_trait)]
pub trait FocusApiConnection {
    /// Executes commands and returns their response.
    async fn run_command(
        &mut self,
        command: &str,
        data: Option<&str>,
    ) -> Result<String, RunCommandError>;

    /// Gets a list of available commands on the device.
    async fn available_commands(&mut self) -> Result<Vec<String>, GetCommandsError> {
        let cmds = self
            .run_command("help", None)
            .await?
            .lines()
            .map(ToOwned::to_owned)
            .collect();

        Ok(cmds)
    }
}

/// Error returned when running commands.
#[derive(Debug, Display, Error)]
pub enum RunCommandError {
    /// We hit an error sending the command to the device.
    #[display("Failed to send command: {_0}")]
    SendingCommand(Box<dyn std::error::Error + Send + Sync>),
    /// We were not able to receive a response from the device.
    #[display("Failed to receive response: {_0}")]
    RecievingResponse(Box<dyn std::error::Error + Send + Sync>),
    /// The response from the device could not be interpreted.
    #[display("received an unexpected response:\n{_0}")]
    UnexpectedResponse(parsing::focus_api::ParseResponseError),
    /// The response stream completed before a response could be interpreted.
    #[display("response stream terminated while waiting for the response to complete")]
    ResponseStreamTerminatedPrematurely,
}

impl From<SerialPortRunCommandError> for RunCommandError {
    fn from(err: SerialPortRunCommandError) -> Self {
        match err {
            SerialPortRunCommandError::SendingCommand(err) => Self::SendingCommand(err.into()),
            SerialPortRunCommandError::RecievingResponse(err) => {
                Self::RecievingResponse(err.into())
            }
            SerialPortRunCommandError::UnexpectedResponse(err) => Self::UnexpectedResponse(err),
            SerialPortRunCommandError::ResponseStreamTerminatedPrematurely => {
                Self::ResponseStreamTerminatedPrematurely
            }
        }
    }
}

impl From<HidRunCommandError> for RunCommandError {
    fn from(err: HidRunCommandError) -> Self {
        match err {
            HidRunCommandError::SendingCommand(err) => Self::SendingCommand(err.into()),
            HidRunCommandError::RecievingResponse(err) => Self::RecievingResponse(err.into()),
            HidRunCommandError::UnexpectedResponse(err) => Self::UnexpectedResponse(err),
        }
    }
}

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
    UnexpectedResponse(parsing::focus_api::ParseResponseError),
    /// The response stream completed before a response could be interpreted.
    #[display("response stream terminated while waiting for the response to complete")]
    ResponseStreamTerminatedPrematurely,
}

/// Error while getting a list of available commands supported by the device.
#[derive(Debug, Display, From, Error)]
#[display("failed to retrieve available commands with the `help command: {_0}")]
pub struct GetCommandsError(RunCommandError);

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
    UnexpectedResponse(parsing::focus_api::ParseResponseError),
}

/// Abstracts over a serial port connection to provide the firmware's
/// Focus API, which is used for controlling the keyboard.
#[derive(Debug)]
pub struct SerialPortFocusApi(SerialPort);

impl FocusApiConnection for SerialPortFocusApi {
    async fn run_command(
        &mut self,
        command: &str,
        data: Option<&str>,
    ) -> Result<String, RunCommandError> {
        self.run_command(command, data).await.map_err(Into::into)
    }
}

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
    async fn run_command(
        &mut self,
        command: &str,
        data: Option<&str>,
    ) -> Result<String, SerialPortRunCommandError> {
        let port = &mut self.0;

        let data_to_send = serialize_command(command, data);

        port.write_all(data_to_send.as_bytes())
            .await
            .map_err(SerialPortRunCommandError::SendingCommand)?;

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

            return match data
                .parse::<parsing::focus_api::FocusApiCommandResponse>()
                .map(|res| res.into_inner())
            {
                Ok(res) => Ok(res),
                Err(parsing::focus_api::ParseResponseError::Incomplete) => continue,
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

impl FocusApiConnection for HidFocusApi {
    async fn run_command(
        &mut self,
        command: &str,
        data: Option<&str>,
    ) -> Result<String, RunCommandError> {
        self.run_command(command, data).await.map_err(Into::into)
    }
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

        eprintln!(
            "Warning:\n\
            You are connected to the keyboard via Bluetooth, \
            which is currently failing to send commands with large data payloads.\n\
            \n\
            Please reconnect with wired or wireless RF and try again if the \
            keyboard becomes unresponsive after this operation.\n\
            \n\
            Nothing bad will happen, just restart the keyboard if it becomes unresponsive."
        );
        Ok(Self {
            _device: device,
            reader,
            writer,
        })
    }

    /// Executes commands and returns their response.
    async fn run_command(
        &mut self,
        command: &str,
        data: Option<&str>,
    ) -> Result<String, HidRunCommandError> {
        let data_to_send = serialize_command(command, data).into_bytes();

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

            // Skip the first byte, as it's the report id
            res.extend_from_slice(&buf[1..bytes_read]);

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

            return match str_res
                .parse::<parsing::focus_api::FocusApiCommandResponse>()
                .map(|res| res.into_inner())
            {
                Ok(res) => Ok(res),
                Err(parsing::focus_api::ParseResponseError::Incomplete) => continue,
                Err(err) => return Err(HidRunCommandError::UnexpectedResponse(err)),
            };
        }
    }
}
