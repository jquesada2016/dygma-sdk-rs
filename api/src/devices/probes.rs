//! The best-effort identity probes shared by every connect-time read.
//!
//! One rule lives here: a probe never aborts the operation it serves.
//! A command the device's `help` output does not list is never sent —
//! the device has advertised that it does not serve it — and a served
//! probe that fails (a transport error, an empty response, a response
//! that does not parse) is recorded as unavailable instead of failing.
//! Backup captures ([`DefyKeyboard::create_backup`]) and connect-time
//! product identification ([`DygmaConnection::new`]) both run their
//! identity reads through [`probe_identity`].
//!
//! [`DefyKeyboard::create_backup`]: crate::devices::defy::DefyKeyboard::create_backup
//! [`DygmaConnection::new`]: crate::devices::connection::DygmaConnection::new

use crate::focus_api::FocusApiConnection;

/// Why a best-effort identity probe produced no value.
///
/// The probes are never allowed to interrupt the operation they serve
/// (a capture, a connect): the reason is recorded instead, and the
/// operation carries whatever identity it did obtain.
#[derive(Clone, Copy, Debug, Display, Error, PartialEq, Eq)]
pub enum IdentityProbeUnavailable {
    /// The device's `help` output — its own capability
    /// advertisement — does not list the probe's command, so the probe
    /// was never sent: the device has said it does not serve it. The
    /// wireless Defy's firmware, for example, serves no
    /// `hardware.wireless` command and answers any attempt with an
    /// empty response.
    #[display("the device does not serve `{_0}`")]
    NotServed(#[error(not(source))] &'static str),
    /// The command is served, but the read did not produce a usable
    /// value: a transport error, an empty response, or a response that
    /// does not parse.
    #[display("the `{_0}` read failed or returned nothing usable")]
    ReadFailed(#[error(not(source))] &'static str),
}

/// Runs one best-effort identity probe: the single place the
/// "a probe never aborts the operation" rule lives.
///
/// A command the device's `help` output does not list is never sent —
/// the device has advertised that it does not serve it
/// ([`IdentityProbeUnavailable::NotServed`]). A served probe that
/// fails — a transport error, an empty response, or a response that
/// does not parse — is [`IdentityProbeUnavailable::ReadFailed`].
/// Either way the reason is logged at `debug` level (the empty
/// responses included, since a served-but-silent command is its own
/// diagnostic) and returned; nothing here can abort a capture or a
/// connect.
pub(crate) async fn probe_identity<T>(
    connection: &mut impl FocusApiConnection,
    served_commands: &[String],
    command: &'static str,
) -> Result<T, IdentityProbeUnavailable>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    if !served_commands.iter().any(|served| served == command) {
        debug!(
            "identity probe: `{command}` is not in the device's `help` \
             output, the probe is skipped"
        );

        return Err(IdentityProbeUnavailable::NotServed(command));
    }

    match connection.run_command(command, None).await {
        Ok(response) if !response.trim().is_empty() => T::from_str(&response).map_err(|err| {
            debug!("identity probe: `{command}` response did not parse: {err}");

            IdentityProbeUnavailable::ReadFailed(command)
        }),
        Ok(_) => {
            debug!("identity probe: `{command}` returned an empty response");

            Err(IdentityProbeUnavailable::ReadFailed(command))
        }
        Err(err) => {
            debug!("identity probe: `{command}` read failed: {err}");

            Err(IdentityProbeUnavailable::ReadFailed(command))
        }
    }
}
