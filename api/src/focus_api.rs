//! Low-level functions for interacting with the Focus API provided with Dygma
//! firmware.

use crate::{
    parsing,
    serial_port::{OpenSerialPortError, SerialPort},
};
use bytes::Bytes;
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;

/// Error returned when running commands.
#[derive(Debug, Display, Error)]
pub enum RunCommandError {
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
pub struct GetCommandsError(RunCommandError);

/// Error creating Focus API struct.
#[derive(Debug, Display, Error, From)]
#[display("failed to create FocusAPI: {_0}")]
pub struct CreateFocusApiError(OpenSerialPortError);

/// Abstracts over a serial port connection to provide the firmware's
/// Focus API, which is used for controlling the keyboard.
#[derive(Debug)]
pub struct FocusApi(SerialPort);

impl FocusApi {
    const MANUFACTURER_NAME: &str = "DYGMA";

    /// Creates a Focus API instance.
    pub async fn new(product_name: &str, baud_rate: u32) -> Result<Self, CreateFocusApiError> {
        let sp = SerialPort::connect(Self::MANUFACTURER_NAME, product_name, baud_rate).await?;

        Ok(Self(sp))
    }

    /// Executes commands and returning their response.
    pub async fn run_command(
        &mut self,
        command: &str,
        data: Option<&str>,
    ) -> Result<String, RunCommandError> {
        let port = &mut self.0;

        let mut write = async |str: &str| {
            port.write_all(str.as_bytes())
                .await
                .map_err(RunCommandError::SendingCommand)
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
            let chunk = chunk_res.map_err(RunCommandError::RecievingResponse)?;

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
                Err(err) => return Err(RunCommandError::UnexpectedResponse(err)),
            };
        }

        Err(RunCommandError::ResponseStreamTerminatedPrematurely)
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
