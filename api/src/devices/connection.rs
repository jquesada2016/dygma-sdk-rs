//! Connects to any supported Dygma product and identifies what it
//! reached: the multi-product front door for the product-agnostic wire
//! layer.
//!
//! A [`DygmaConnection`] is the honest baseline for talking to any
//! supported product: every Dygma product's firmware serves the same
//! raw Focus API (`help`, commands, reads and writes), so raw commands
//! work against all of them. What does NOT generalize to non-Defy
//! products is the device-level typed surface — the keymap, superkeys,
//! macros, palette, and colormap methods on
//! [`DefyKeyboard`](crate::devices::defy::DefyKeyboard) are
//! Defy-shaped, and this module deliberately offers no typed
//! equivalents for the others: no non-Defy product has device-level
//! APIs here, and nothing in this module can be used to imply that it
//! does.
//!
//! Identification mirrors Bazecor's flow. The transport identity
//! ([`focus_api::identities`]) names the product deterministically —
//! the same way Bazecor's scans match devices by (vendor id, product
//! id) pair — and, exactly where Bazecor needs more (the Raise
//! generations' shared ANSI/ISO identities), a best-effort
//! `hardware.layout` probe refines the answer. The probe is one of the
//! shared identity probes: a device that does not serve it, or answers
//! it with garbage, leaves the layout unknown and never fails the
//! connect.

pub use error::*;

use super::{conversions::Product, defy::DynFocusApi, probes};
use crate::focus_api::{
    self, CreateHidFocusApiError, FocusApiConnection, SerialPortFocusApi, identities, parsing,
};

mod error {
    use super::*;

    /// Error returned when creating a handle to any Dygma product's
    /// connection.
    ///
    /// The serial attempt is tried first; when no serial identity
    /// matched (or the port could not be opened), the HID attempt
    /// decides the outcome and its error is the one reported — the
    /// same reporting semantics
    /// [`DefyKeyboard::new`](crate::devices::defy::DefyKeyboard::new)
    /// has always had for its serial-then-HID fallback.
    #[derive(Debug, Display, From, Error)]
    #[display("failed to create handle to the Dygma keyboard: {_0}")]
    pub struct CreateDygmaConnectionError(CreateHidFocusApiError);
}

/// Which transport a connection rides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportKind {
    /// A serial-port connection over USB — the wired products'
    /// serial interface directly, and the wireless Neurons' serial
    /// interface over their USB link.
    Serial,
    /// A HID connection — the wireless products' raw-HID interface,
    /// carried over Bluetooth.
    Bluetooth,
}

impl std::fmt::Display for TransportKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Serial => "serial",
            Self::Bluetooth => "Bluetooth",
        })
    }
}

/// What a connection found on the other end, identified at connect
/// time.
///
/// The product is named by the transport identity — no probing needed
/// and none spent on it. The physical layout rides along only where
/// the transport identity cannot decide it (the Raise generations'
/// shared ANSI/ISO identities) and only when the device actually
/// answered the probe: it is evidence, not a guess, and Bazecor's own
/// default-to-ANSI-on-missing behavior is deliberately not replicated
/// (a guess would be reported as if the device had said it).
#[derive(Clone, Debug)]
pub struct ConnectedProduct {
    /// The transport-level identity the connection matched, as the
    /// device's own USB ids spell it.
    pub identity: identities::DeviceIdentity,
    /// The product the identity names.
    pub product: Product,
    /// The physical layout, when the device identified one
    /// (`hardware.layout` probe — the Raise generations only, where
    /// ANSI/ISO is a real wire difference and the transport identity
    /// cannot tell).
    pub layout: Option<parsing::settings::KeyboardLayout>,
    /// The transport the connection rides.
    pub transport: TransportKind,
}

impl std::fmt::Display for ConnectedProduct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.identity.kind)?;

        if let Some(layout) = self.layout {
            write!(f, " ({layout})")?;
        }

        write!(f, " over {}", self.transport)
    }
}

/// A connection to any supported Dygma product, with the product
/// identified at connect time.
///
/// This is the surface for the product-agnostic wire layer: raw
/// [`FocusApiConnection::run_command`] and
/// [`FocusApiConnection::available_commands`] work against every
/// product the connection can reach. It is NOT a typed device handle —
/// those are product-shaped, and only one exists: [`DefyKeyboard`] is
/// the Defy's, and no other product has one.
///
/// [`DefyKeyboard`]: super::defy::DefyKeyboard
#[derive(Debug, Deref, DerefMut)]
pub struct DygmaConnection {
    #[deref]
    #[deref_mut]
    api: DynFocusApi,
    /// The connect-time identification of the product on the other
    /// end.
    pub product: ConnectedProduct,
}

impl DygmaConnection {
    /// Opens a connection to the first supported Dygma product it can
    /// reach and identifies it.
    ///
    /// The serial transport is tried first (every product's serial
    /// identity — wired USB or the wireless Neuron's serial
    /// interface), then the HID transport (the wireless products'
    /// Bluetooth interface). Whichever connects, the matched transport
    /// identity names the product; for the Raise generations one
    /// best-effort `hardware.layout` probe refines the ANSI/ISO
    /// layout, exactly the probe Bazecor's scan needs for the same
    /// reason. The probe never fails the connect: an unserved or
    /// unusable answer leaves the layout unknown.
    ///
    /// Note that the wireless Defy carries the same product id on both
    /// transports, so a wireless Defy attached by USB cable connects
    /// over serial and identifies as the wireless variant; the same
    /// keyboard over Bluetooth connects over HID. Both are the
    /// identification Bazecor's own scan produces.
    pub async fn new() -> Result<Self, CreateDygmaConnectionError> {
        let serial =
            SerialPortFocusApi::new(&identities::SERIAL_IDENTITIES, identities::BAUD_RATE).await;

        let (api, identity, transport) = match serial {
            Ok((api, identity)) => (api.into(), identity, TransportKind::Serial),
            Err(serial_error) => {
                debug!(
                    "no serial identity connected ({serial_error:?}); \
                     trying the HID transport"
                );

                let (hid, identity) =
                    focus_api::HidFocusApi::new_matching(&identities::HID_IDENTITIES)
                        .await
                        .map_err(CreateDygmaConnectionError::from)?;

                (hid.into(), identity, TransportKind::Bluetooth)
            }
        };

        let mut connection = Self {
            api,
            product: ConnectedProduct {
                identity,
                product: product_of(identity.kind),
                layout: None,
                transport,
            },
        };

        connection.product.layout = probe_layout_over(&mut connection.api, identity.kind).await;

        Ok(connection)
    }
}

/// The product a transport-level [`identities::DeviceKind`] names.
///
/// Total and exhaustive: a kind added to the identity tables cannot
/// compile through here unnamed.
fn product_of(kind: identities::DeviceKind) -> Product {
    match kind {
        identities::DeviceKind::DefyWired | identities::DeviceKind::DefyWireless => Product::Defy,
        identities::DeviceKind::Raise => Product::Raise,
        identities::DeviceKind::Raise2 => Product::Raise2,
        identities::DeviceKind::Sonsei => Product::Sonsei,
    }
}

/// Probes the physical layout of the connected Raise-generation
/// keyboard, best-effort.
///
/// The Raise generations are the only products whose ANSI/ISO layout
/// is a real wire difference their transport identity cannot decide.
/// Bazecor probes `hardware.layout` on every serial-matched device
/// (`src/api/comms/serial/SerialAPI.ts`, `checkProperties`, alongside
/// `hardware.wireless` and `hardware.chip_id`) but consumes the
/// answer's layout half only for these: its hardware-definition match
/// bypasses the `keyboardType` condition for the Defy and the Sonsei
/// by product name in the very same `find`. This probe follows that
/// consumption — the Defy's and Sonsei's identity is complete without
/// the layout, so no probe is spent on them: `None` immediately,
/// without a single command sent.
///
/// The probe is one of the shared identity probes: unserved or
/// unusable answers log their reason and leave the layout unknown —
/// never an error.
///
/// Generic over the connection like the backup capture core, so
/// offline tests can drive the real identification through a scripted
/// double.
async fn probe_layout_over(
    connection: &mut impl FocusApiConnection,
    kind: identities::DeviceKind,
) -> Option<parsing::settings::KeyboardLayout> {
    if !matches!(
        kind,
        identities::DeviceKind::Raise | identities::DeviceKind::Raise2
    ) {
        return None;
    }

    let served = match connection.available_commands().await {
        Ok(served) => served,
        Err(err) => {
            debug!("product identification: `help` read failed: {err}");
            return None;
        }
    };

    match probes::probe_identity::<parsing::settings::KeyboardLayout>(
        connection,
        &served,
        super::defy::DefyKeyboard::HARDWARE_LAYOUT_COMMAND_NAME,
    )
    .await
    {
        Ok(layout) => Some(layout),
        Err(unavailable) => {
            debug!("product identification: {unavailable}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use crate::focus_api::{FocusApiConnection, RunCommandError};

    use super::*;

    /// A scripted Focus API connection: each `run_command` checks the
    /// command against the next script entry and hands back its
    /// response — the same offline double the capture tests drive,
    /// here over the identification probes.
    #[derive(Debug)]
    struct ScriptedConnection {
        script: VecDeque<(&'static str, Result<String, RunCommandError>)>,
    }

    impl ScriptedConnection {
        /// A `help` response serving the layout probe (plus filler).
        fn help_serving_layout() -> String {
            "keymap.custom\nhardware.layout\nmouse.speed\n".to_owned()
        }

        /// A `help` response that does not serve the layout probe.
        fn help_without_layout() -> String {
            "keymap.custom\nmouse.speed\n".to_owned()
        }

        fn ok(
            command: &'static str,
            response: &str,
        ) -> (&'static str, Result<String, RunCommandError>) {
            (command, Ok(response.to_owned()))
        }

        fn failed(command: &'static str) -> (&'static str, Result<String, RunCommandError>) {
            (
                command,
                Err(RunCommandError::ResponseStreamTerminatedPrematurely),
            )
        }

        /// The proof a test's script covered exactly the conversation
        /// the identification ran.
        fn assert_spent(&self) {
            assert!(
                self.script.is_empty(),
                "the script was not fully consumed, {:?} left",
                self.script
            );
        }
    }

    impl FocusApiConnection for ScriptedConnection {
        async fn run_command(
            &mut self,
            command: &str,
            _data: Option<&str>,
        ) -> Result<String, RunCommandError> {
            let (expected, response) = self
                .script
                .pop_front()
                .expect("the script must cover every command the identification runs");

            assert_eq!(
                command, expected,
                "the identification ran an unscripted command"
            );

            response
        }
    }

    #[tokio::test]
    async fn a_raise2_that_serves_layout_gets_its_layout_identified() {
        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok("help", &ScriptedConnection::help_serving_layout()),
                ScriptedConnection::ok("hardware.layout", "ISO"),
            ]
            .into(),
        };

        let layout = probe_layout_over(&mut connection, identities::DeviceKind::Raise2).await;

        assert_eq!(layout, Some(parsing::settings::KeyboardLayout::Iso));
        connection.assert_spent();
    }

    #[tokio::test]
    async fn an_unserved_layout_probe_leaves_the_layout_unknown_without_sending_it() {
        let mut connection = ScriptedConnection {
            script: [ScriptedConnection::ok(
                "help",
                &ScriptedConnection::help_without_layout(),
            )]
            .into(),
        };

        let layout = probe_layout_over(&mut connection, identities::DeviceKind::Raise).await;

        assert_eq!(layout, None);
        connection.assert_spent();
    }

    #[tokio::test]
    async fn a_garbage_layout_answer_leaves_the_layout_unknown() {
        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok("help", &ScriptedConnection::help_serving_layout()),
                ScriptedConnection::ok("hardware.layout", "QWERTY-ish"),
            ]
            .into(),
        };

        let layout = probe_layout_over(&mut connection, identities::DeviceKind::Raise2).await;

        assert_eq!(layout, None);
        connection.assert_spent();
    }

    #[tokio::test]
    async fn an_empty_layout_answer_leaves_the_layout_unknown() {
        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok("help", &ScriptedConnection::help_serving_layout()),
                ScriptedConnection::ok("hardware.layout", ""),
            ]
            .into(),
        };

        let layout = probe_layout_over(&mut connection, identities::DeviceKind::Raise2).await;

        assert_eq!(layout, None);
        connection.assert_spent();
    }

    #[tokio::test]
    async fn a_failed_layout_read_leaves_the_layout_unknown() {
        let mut connection = ScriptedConnection {
            script: [
                ScriptedConnection::ok("help", &ScriptedConnection::help_serving_layout()),
                ScriptedConnection::failed("hardware.layout"),
            ]
            .into(),
        };

        let layout = probe_layout_over(&mut connection, identities::DeviceKind::Raise2).await;

        assert_eq!(layout, None);
        connection.assert_spent();
    }

    #[tokio::test]
    async fn a_failed_help_read_leaves_the_layout_unknown() {
        let mut connection = ScriptedConnection {
            script: [ScriptedConnection::failed("help")].into(),
        };

        let layout = probe_layout_over(&mut connection, identities::DeviceKind::Raise2).await;

        assert_eq!(layout, None);
        connection.assert_spent();
    }

    #[tokio::test]
    async fn defy_and_sonsei_kinds_are_never_probed() {
        for kind in [
            identities::DeviceKind::DefyWired,
            identities::DeviceKind::DefyWireless,
            identities::DeviceKind::Sonsei,
        ] {
            // No script entries at all: if a command went out, the
            // double panics on the empty script.
            let mut connection = ScriptedConnection {
                script: VecDeque::new(),
            };

            let layout = probe_layout_over(&mut connection, kind).await;

            assert_eq!(layout, None, "{kind:?} must not be probed");
            connection.assert_spent();
        }
    }

    #[test]
    fn the_identity_tables_name_the_four_products() {
        assert_eq!(product_of(identities::DEFY_WIRED.kind), Product::Defy);
        assert_eq!(product_of(identities::DEFY_WIRELESS.kind), Product::Defy);
        assert_eq!(product_of(identities::RAISE.kind), Product::Raise);
        assert_eq!(product_of(identities::RAISE2.kind), Product::Raise2);
        assert_eq!(
            product_of(identities::RAISE2_ISO_RECORD.kind),
            Product::Raise2
        );
        assert_eq!(product_of(identities::SONSEI.kind), Product::Sonsei);
    }

    #[test]
    fn connected_product_displays_the_truth_about_what_was_reached() {
        let rendered = |identity: identities::DeviceIdentity,
                        layout: Option<parsing::settings::KeyboardLayout>,
                        transport: TransportKind| {
            ConnectedProduct {
                identity,
                product: product_of(identity.kind),
                layout,
                transport,
            }
            .to_string()
        };

        assert_eq!(
            rendered(identities::DEFY_WIRED, None, TransportKind::Serial),
            "Defy (wired) over serial"
        );
        assert_eq!(
            rendered(identities::DEFY_WIRELESS, None, TransportKind::Bluetooth),
            "Defy (wireless) over Bluetooth"
        );
        assert_eq!(
            rendered(identities::SONSEI, None, TransportKind::Serial),
            "Sonsei over serial"
        );
        assert_eq!(
            rendered(
                identities::RAISE2,
                Some(parsing::settings::KeyboardLayout::Iso),
                TransportKind::Serial
            ),
            "Raise 2 (ISO) over serial"
        );
        assert_eq!(
            rendered(identities::RAISE, None, TransportKind::Serial),
            "Raise over serial"
        );
    }
}
