//! Functions for interacting with the Focus API provided with Dygma
//! firmware.

pub mod parsing;
mod serial_port;

use std::time::Duration;

use async_hid::{AsyncHidRead, AsyncHidWrite};
use bytes::Bytes;
use serial_port::{OpenSerialPortError, SerialPort};
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, timeout};
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
    /// The device did not complete its response within the read deadline.
    #[display("timed out waiting for the response to complete")]
    ResponseTimedOut,
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
            HidRunCommandError::ResponseTimedOut => Self::ResponseTimedOut,
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
pub enum CreateHidFocusApiError {
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
    ///
    /// A failure mid-command surfaces here with the previously written
    /// chunks already delivered to the device (a command is at most a
    /// few dozen chunks); there is no resync — the caller should treat
    /// the connection's command state as undefined.
    SendingCommand(async_hid::HidError),
    /// Something went wrong receiving data from the device.
    RecievingResponse(async_hid::HidError),
    /// The response from the device could not be interpreted.
    #[display("received an unexpected response:\n{_0}")]
    UnexpectedResponse(parsing::focus_api::ParseResponseError),
    /// The device did not complete its response within the read
    /// deadline of 8000 ms (`RESPONSE_DEADLINE_MS`, Bazecor's
    /// per-command read timeout). Mid-command silence of several
    /// seconds is legitimate — the device parses each number with a
    /// 1000 ms timeout — but silence past the deadline means the
    /// response is not coming; without the deadline the read loop
    /// would hang forever.
    #[display("timed out waiting for the device to complete its response")]
    ResponseTimedOut,
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

/// The report id of the Defy's raw-HID endpoint: prepended to every
/// output report and expected at the start of every input report
/// belonging to a response.
const HID_REPORT_ID: u8 = 5;

/// Payload bytes carried by one HID output report.
const HID_MAX_SEND_SIZE: usize = 200;

/// Full size of a HID report on the wire: the report id byte plus the
/// payload.
const HID_REPORT_SIZE: usize = HID_MAX_SEND_SIZE + 1;

/// A full-size HID output report: report id byte + payload.
type OutputReport = [u8; HID_REPORT_SIZE];

/// Milliseconds to pause between consecutive output-report writes of a
/// single command.
///
/// The wireless firmware processes one command per main-loop iteration
/// and dequeues incoming BLE reports only when that loop yields; its
/// scheduler queue is ten deep and a queue overflow reboots the
/// keyboard. A blast of reports arriving faster than the loop drains
/// them can therefore silently reset the device mid-transfer. 20 ms is
/// a deliberately conservative floor: Bazecor, the proven reference
/// client, ships ZERO inter-chunk delay and works because the OS radio
/// stack paces its writes — so if live validation shows the
/// conservative value to be unnecessary, this may be tuned down to 0.
/// The cost of keeping it is small and bounded (a full ~3.2 KB keymap
/// write spends well under a second pacing); the cost of dropping it
/// unvalidated is a possible mid-transfer reboot.
const INTER_CHUNK_DELAY_MS: u64 = 20;

/// Milliseconds to wait for a command's response before giving up.
///
/// 8000 ms matches Bazecor's per-command read timeout. Mid-command
/// silence of several seconds is legitimate (a lost chunk makes the
/// device sit on its 1000 ms per-number parse timeout, and a
/// multi-hundred-byte response takes one input report per main-loop
/// iteration), but silence past this deadline means the response is
/// not coming; without a deadline the read loop hangs forever.
///
/// Boundary: this deadline covers the read loop only — writes are not
/// deadline-bounded (the same exposure as the serial path's
/// `write_all`).
const RESPONSE_DEADLINE_MS: u64 = 8000;

/// Splits serialized command bytes into full-size HID output reports.
///
/// Every report carries [`HID_REPORT_ID`] as its first byte followed by
/// a [`HID_MAX_SEND_SIZE`]-byte slice of `data`, zero-padded to full
/// size — including the final report. The wireless firmware's receive
/// handler reads each incoming chunk with `strnlen_s` (the first zero
/// byte ends the chunk), so a short final chunk would make it consume
/// uninitialized stack memory past the transmitted data; padding every
/// chunk to full size — as Bazecor does — makes the transfer exact.
///
/// Returns one report per `ceil(data.len() / HID_MAX_SEND_SIZE)`
/// chunks. Empty input produces no reports: nothing is written for
/// nothing.
fn build_output_reports(data: &[u8]) -> Vec<OutputReport> {
    let mut reports = Vec::with_capacity(data.len().div_ceil(HID_MAX_SEND_SIZE));

    for chunk in data.chunks(HID_MAX_SEND_SIZE) {
        let mut report = [0; HID_REPORT_SIZE];
        report[0] = HID_REPORT_ID;
        report[1..1 + chunk.len()].copy_from_slice(chunk);
        reports.push(report);
    }

    reports
}

/// The HID wire transport behind [`HidFocusApi`].
///
/// Generic over async-hid's [`AsyncHidRead`] / [`AsyncHidWrite`] traits
/// so offline tests can drive [`HidTransport::run_command`] with mock
/// readers and writers instead of real hardware.
#[derive(Debug)]
struct HidTransport<R, W> {
    #[debug(ignore)]
    reader: R,
    #[debug(ignore)]
    writer: W,
}

impl<R: AsyncHidRead, W: AsyncHidWrite> HidTransport<R, W> {
    /// Executes commands and returns their response.
    async fn run_command(
        &mut self,
        command: &str,
        data: Option<&str>,
    ) -> Result<String, HidRunCommandError> {
        let data_to_send = serialize_command(command, data).into_bytes();

        for (i, report) in build_output_reports(&data_to_send).into_iter().enumerate() {
            if i > 0 {
                sleep(Duration::from_millis(INTER_CHUNK_DELAY_MS)).await;
            }

            self.writer
                .write_output_report(&report)
                .await
                .map_err(HidRunCommandError::SendingCommand)?;
        }

        let read_response = async {
            let mut response = Vec::new();

            // HID_MAX_SEND_SIZE + 1 because of the leading report id byte
            let mut buf = [0; HID_REPORT_SIZE];

            let mut skipped_reports = 0usize;

            loop {
                let bytes_read = self
                    .reader
                    .read_input_report(&mut buf)
                    .await
                    .map_err(HidRunCommandError::RecievingResponse)?;

                // Every report of a response carries the report id;
                // anything else is noise from another report stream.
                // Skip it (counting, for diagnostics) and keep waiting.
                if buf[0] != HID_REPORT_ID {
                    skipped_reports += 1;
                    debug!(
                        "skipped input report with unexpected id {} ({skipped_reports} so far)",
                        buf[0]
                    );
                    continue;
                }

                // Skip the first byte, as it's the report id. The
                // device zero-pads each report to full size, so the
                // trailing padding is stripped before the payload joins
                // the response. Stripping is safe: device-generated
                // content is ASCII text, and while a user-set Bluetooth
                // device name can echo back as arbitrary UTF-8, UTF-8
                // contains a zero byte only for U+0000 itself — no
                // realistic path stores one in a device name — and a
                // NUL inside content is untouched by this trailing-only
                // strip (`rposition` sees past it). So every stripped
                // byte is padding, never content. (Bazecor's
                // purgeUselessCharsEnd makes the same class of
                // assumption — it drops NULs anywhere.)
                let payload = &buf[1..bytes_read];
                let payload_end = payload
                    .iter()
                    .rposition(|&byte| byte != 0)
                    .map_or(0, |last_nonzero| last_nonzero + 1);
                response.extend_from_slice(&payload[..payload_end]);

                let str_res = match str::from_utf8(response.as_ref()) {
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
                    Err(err) => Err(HidRunCommandError::UnexpectedResponse(err)),
                };
            }
        };

        match timeout(Duration::from_millis(RESPONSE_DEADLINE_MS), read_response).await {
            Ok(response) => response,
            Err(_elapsed) => Err(HidRunCommandError::ResponseTimedOut),
        }
    }
}

/// Abstracts over a HID connection to provide the firmware's
/// Focus API, which is used for controlling the keyboard.
///
/// Note that this works also over BTLE.
#[derive(Debug)]
pub struct HidFocusApi {
    _device: async_hid::Device,
    transport: HidTransport<async_hid::DeviceReader, async_hid::DeviceWriter>,
}

impl FocusApiConnection for HidFocusApi {
    async fn run_command(
        &mut self,
        command: &str,
        data: Option<&str>,
    ) -> Result<String, RunCommandError> {
        self.transport
            .run_command(command, data)
            .await
            .map_err(Into::into)
    }
}

impl HidFocusApi {
    const VENDOR_ID: u16 = 13807;
    const USAGE_ID: u16 = 1;
    const USAGE_PAGE: u16 = 65280;

    /// Opens a connection to the requested Dygma device.
    pub async fn new(product_id: u16) -> Result<Self, CreateHidFocusApiError> {
        let backend = async_hid::HidBackend::default();

        let device_stream = backend
            .enumerate()
            .await
            .map_err(CreateHidFocusApiError::EnumeratingFailure)?
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
            .ok_or(CreateHidFocusApiError::DeviceNotFound)?;

        let (reader, writer) = device
            .open()
            .await
            .map_err(CreateHidFocusApiError::ConnectingToDevice)?;

        eprintln!(
            "Warning:\n\
            You are connected to the keyboard via Bluetooth.\n\
            \n\
            Large data payloads over Bluetooth used to fail: the final \
            chunk of every transfer went out short and unpadded, which \
            the wireless firmware misreads. This build zero-pads every \
            chunk to full size and paces the transfer, but that fix is \
            NOT yet validated against real hardware over the air.\n\
            \n\
            If the keyboard becomes unresponsive after a large \
            transfer, restart it and reconnect over wired or wireless \
            RF. This warning is removed once the fix is live-validated."
        );

        Ok(Self {
            _device: device,
            transport: HidTransport { reader, writer },
        })
    }
}

#[cfg(test)]
mod hid_tests {
    use std::collections::VecDeque;
    use std::future::Future;
    use std::sync::{Arc, Mutex};

    use async_hid::{HidError, HidResult};

    use super::*;

    /// An input report as it sits on the wire: report id byte first.
    fn input_report(id: u8, payload: &[u8]) -> Vec<u8> {
        let mut report = Vec::with_capacity(1 + payload.len());
        report.push(id);
        report.extend_from_slice(payload);
        report
    }

    /// An input report carrying `payload` zero-padded to full size —
    /// the shape the device actually sends.
    fn padded_input_report(payload: &[u8]) -> Vec<u8> {
        let mut report = input_report(HID_REPORT_ID, payload);
        report.resize(HID_REPORT_SIZE, 0);
        report
    }

    /// Full `keymap.custom` write magnitude: 10 layers of 80
    /// three-digit codes (~3.2 KB, 17 chunks).
    fn keymap_write_data() -> String {
        "123 ".repeat(80 * 10)
    }

    /// Serves a scripted sequence of input reports, then panics — a
    /// test reading past its scripted response is a test bug.
    ///
    /// The reports are served synchronously: the operation needs no
    /// suspension, so the future is always ready.
    #[derive(Default)]
    struct MockReader {
        reports: VecDeque<Vec<u8>>,
    }

    impl AsyncHidRead for MockReader {
        fn read_input_report<'a>(
            &'a mut self,
            buf: &'a mut [u8],
        ) -> impl Future<Output = HidResult<usize>> + Send + 'a {
            let report = self
                .reports
                .pop_front()
                .expect("mock input report queue exhausted");
            buf[..report.len()].copy_from_slice(&report);
            std::future::ready(Ok(report.len()))
        }
    }

    /// A reader that never yields a report — the device has gone
    /// completely silent.
    struct SilentReader;

    impl AsyncHidRead for SilentReader {
        fn read_input_report<'a>(
            &'a mut self,
            _buf: &'a mut [u8],
        ) -> impl Future<Output = HidResult<usize>> + Send + 'a {
            std::future::pending()
        }
    }

    /// Records every output report written, in order.
    #[derive(Debug, Default)]
    struct MockWriter {
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl AsyncHidWrite for MockWriter {
        fn write_output_report<'a>(
            &'a mut self,
            buf: &'a [u8],
        ) -> impl Future<Output = HidResult<()>> + Send + 'a {
            self.writes
                .lock()
                .expect("writer lock poisoned")
                .push(buf.to_vec());
            std::future::ready(Ok(()))
        }
    }

    /// Fails every write.
    struct FailingWriter;

    impl AsyncHidWrite for FailingWriter {
        fn write_output_report<'a>(
            &'a mut self,
            _buf: &'a [u8],
        ) -> impl Future<Output = HidResult<()>> + Send + 'a {
            std::future::ready(Err(HidError::Disconnected))
        }
    }

    /// A transport whose writes are recorded and whose reads are
    /// scripted, plus the shared write log for assertions.
    struct RecordingTransport {
        transport: HidTransport<MockReader, MockWriter>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    fn recording_transport(responses: Vec<Vec<u8>>) -> RecordingTransport {
        let writer = MockWriter::default();
        let writes = writer.writes.clone();
        RecordingTransport {
            transport: HidTransport {
                reader: MockReader {
                    reports: responses.into_iter().collect(),
                },
                writer,
            },
            writes,
        }
    }

    /// Asserts the transport-level chunk contract over an already
    /// recorded write log: one id-prefixed full-size report per chunk,
    /// payloads reassembling `expected` with zero padding on the last.
    fn assert_full_size_padded_reports<R: AsRef<[u8]>>(reports: &[R], expected: &[u8]) {
        assert_eq!(reports.len(), expected.len().div_ceil(HID_MAX_SEND_SIZE));

        for (i, report) in reports.iter().enumerate() {
            assert_eq!(
                report.as_ref().len(),
                HID_REPORT_SIZE,
                "report {i} not full size"
            );
            assert_eq!(
                report.as_ref()[0],
                HID_REPORT_ID,
                "report {i} missing report id"
            );
        }

        let concatenated: Vec<u8> = reports
            .iter()
            .flat_map(|report| report.as_ref()[1..].iter().copied())
            .collect();
        assert_eq!(
            &concatenated[..expected.len()],
            expected,
            "chunk payloads must reassemble the command bytes"
        );
        assert!(
            concatenated[expected.len()..].iter().all(|&byte| byte == 0),
            "padding beyond the command bytes must be zero"
        );
    }

    #[test]
    fn chunker_covers_full_magnitude_command_exactly() {
        let command = serialize_command("keymap.custom", Some(&keymap_write_data()));
        let command_bytes = command.into_bytes();

        let reports = build_output_reports(&command_bytes);

        assert_full_size_padded_reports(&reports, &command_bytes);
    }

    #[test]
    fn chunker_holds_at_every_boundary_length() {
        for len in [0, 1, 199, 200, 201, 399, 400, 401] {
            let data = vec![b'x'; len];

            assert_full_size_padded_reports(&build_output_reports(&data), &data);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn write_loop_sends_zero_padded_full_size_reports_in_order() {
        let command_bytes = serialize_command("keymap.custom", Some(&keymap_write_data()));
        let command_bytes = command_bytes.into_bytes();

        let RecordingTransport {
            mut transport,
            writes,
        } = recording_transport(vec![padded_input_report(b"ok\r\n.")]);

        let response = transport
            .run_command("keymap.custom", Some(&keymap_write_data()))
            .await
            .unwrap();
        assert_eq!(response, "ok");

        let writes = writes.lock().expect("writer lock poisoned");
        assert_full_size_padded_reports(&writes, &command_bytes);
    }

    #[tokio::test(start_paused = true)]
    async fn silent_device_times_out_instead_of_hanging() {
        let mut transport = HidTransport {
            reader: SilentReader,
            writer: MockWriter::default(),
        };

        // The guard deadline only fires if the production response
        // deadline below never does, turning a regression into a fast
        // failure instead of a hung suite.
        let bounded = tokio::time::timeout(
            Duration::from_secs(30),
            transport.run_command("version", None),
        )
        .await;

        assert!(
            matches!(bounded, Ok(Err(HidRunCommandError::ResponseTimedOut))),
            "expected the response deadline to fire first, got {bounded:?}"
        );
    }

    #[tokio::test]
    async fn read_loop_skips_foreign_report_ids_and_nul_padding() {
        let RecordingTransport { mut transport, .. } = recording_transport(vec![
            input_report(3, b"noise from another report stream"),
            padded_input_report(b"keymap.custom 1 2\r\n"),
            // Foreign id carrying invalid utf8: the id filter must drop
            // it before the response bytes are ever inspected.
            input_report(7, &[0x80]),
            padded_input_report(b"."),
        ]);

        let response = transport.run_command("keymap.custom", None).await.unwrap();

        assert_eq!(response, "keymap.custom 1 2");
    }

    #[tokio::test]
    async fn read_loop_keeps_reading_through_partial_reports() {
        let RecordingTransport { mut transport, .. } = recording_transport(vec![
            input_report(HID_REPORT_ID, b"par"),
            input_report(HID_REPORT_ID, b"tial respo"),
            input_report(HID_REPORT_ID, b"nse\r\n."),
        ]);

        let response = transport.run_command("keymap.custom", None).await.unwrap();

        assert_eq!(response, "partial response");
    }

    #[tokio::test]
    async fn write_failure_surfaces_as_a_sending_error() {
        let mut transport = HidTransport {
            reader: MockReader::default(),
            writer: FailingWriter,
        };

        let result = transport.run_command("version", None).await;

        assert!(matches!(result, Err(HidRunCommandError::SendingCommand(_))));
    }

    #[test]
    fn timeout_error_message_is_pinned() {
        assert_eq!(
            HidRunCommandError::ResponseTimedOut.to_string(),
            "timed out waiting for the device to complete its response"
        );
    }
}
