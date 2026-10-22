//! The USB identities Dygma products present on each transport, and the
//! match tables that drive device discovery.
//!
//! Ground truth for these ids is Bazecor — the reference client whose
//! matching demonstrably works against real hardware — read first-hand
//! at commit `ff9e10b`. Its hardware definitions
//! (`src/api/hardware-dygma-*/index.ts`) are the operative source: its
//! serial scan (`src/api/comms/serial/SerialAPI.ts`) matches ports by
//! vendor id `0x35ef`/`0x1209` and an exact (vendor id, product id) pair
//! against those definitions — never by the manufacturer/product
//! strings the ports carry — and its HID scan
//! (`src/api/hid/hid.ts`, `src/main/setup/configureHID.ts`) matches
//! vendor id 13807 with product ids `[18, 33]` at usage page 65280,
//! usage id 1. The shipped backups' `neuron.device.usb` descriptors are
//! corroborating evidence; where one disagrees with the operative
//! definitions, both ids are matched (the union — see
//! [`RAISE2_ISO_RECORD`]).
//!
//! The firmware is the final authority, and no non-Defy id on this
//! table has been confirmed against real hardware: every non-Defy
//! identity is differential-validated only. Bootloader identities
//! (Defy wired 17, Defy wireless 19, Raise 8704, Raise 2 ANSI 32,
//! Raise 2 ISO 34, Sonsei 48 — each definition's second `usb` block)
//! are recorded here as documentation only: they belong to the
//! firmware-flashing workstream and nothing in this crate matches them.

/// The baud rate every Dygma product's serial connection uses.
///
/// Bazecor opens serial ports at this rate unconditionally
/// (`src/api/comms/serial/SerialAPI.ts`, `open`).
pub const BAUD_RATE: u32 = 115_200;

/// The USB vendor id of Dygma `0x35ef` (decimal 13807): the Defy,
/// Raise 2, and Sonsei all present it.
pub const VENDOR_DYGMA: u16 = 0x35ef;

/// The USB vendor id the first-generation Raise presents, `0x1209`
/// (decimal 4617; the pid.codes reservation).
pub const VENDOR_RAISE: u16 = 0x1209;

/// Which Dygma product a transport identity belongs to, at the
/// transport's own resolution.
///
/// This is the taxonomy the wire transports can prove: the Defy's
/// wired and wireless variants carry distinct product ids, while the
/// Raise generations' ANSI/ISO layouts share product ids and are
/// disambiguated only by probing the connected device (Bazecor's scan
/// probes `hardware.layout` for exactly this — see
/// `src/api/comms/serial/SerialAPI.ts`, `checkProperties`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeviceKind {
    /// The Defy, wired variant (product id 16 over serial).
    DefyWired,
    /// The Defy, wireless (RF/Bluetooth) variant — product id 18 on
    /// both transports.
    DefyWireless,
    /// The first-generation Raise (ANSI and ISO share one identity).
    Raise,
    /// The Raise 2 (ANSI and ISO share identities over the transports
    /// — see [`RAISE2_ISO_RECORD`] for the one attested exception).
    Raise2,
    /// The Sonsei (a single-body Bluetooth product whose Neuron this
    /// crate connects to over USB serial — see [`SONSEI`] for the
    /// raw-HID interface Bazecor's Lens opens on it).
    Sonsei,
}

impl DeviceKind {
    /// The product's name, as Bazecor's device descriptors spell it.
    pub fn name(self) -> &'static str {
        match self {
            Self::DefyWired => "Defy (wired)",
            Self::DefyWireless => "Defy (wireless)",
            Self::Raise => "Raise",
            Self::Raise2 => "Raise 2",
            Self::Sonsei => "Sonsei",
        }
    }
}

impl std::fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// One (vendor id, product id, product) triple a Dygma product presents
/// on a transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DeviceIdentity {
    /// The USB vendor id the device presents.
    pub vendor_id: u16,
    /// The USB product id the device presents.
    pub product_id: u16,
    /// The product this identity belongs to.
    pub kind: DeviceKind,
}

impl DeviceIdentity {
    /// Whether this identity names a device presenting the given
    /// vendor and product ids.
    pub fn matches(&self, vendor_id: u16, product_id: u16) -> bool {
        self.vendor_id == vendor_id && self.product_id == product_id
    }
}

/// The Defy, wired variant, over serial: vendor `0x35ef`, product id
/// `0x0010` (16). Bazecor `src/api/hardware-dygma-defy-wired/index.ts`,
/// first `usb` block. This project has never exercised this identity
/// against hardware — the wired Defy's live path matched product
/// strings until this pid table landed — so, like every non-HID-18
/// identity here, it rests on Bazecor's field-proven matching alone.
pub const DEFY_WIRED: DeviceIdentity = DeviceIdentity {
    vendor_id: VENDOR_DYGMA,
    product_id: 0x0010,
    kind: DeviceKind::DefyWired,
};

/// The Defy, wireless variant: vendor `0x35ef`, product id `0x0012`
/// (18) — the same id on both transports, so this one identity sits in
/// both match tables. Bazecor `src/api/hardware-dygma-defy-wireless/index.ts`,
/// first `usb` block; its HID scan matches the same id, and the shipped
/// Defy wireless backup's descriptor carries it
/// (`src/defaultBackups/Defy-wireless-backup.json`). Of every identity
/// on these tables, only this one's HID id has been exercised against
/// real hardware by this project (the owner's wireless Defy); its
/// serial side, like the rest of the tables, rests on Bazecor's
/// field-proven matching alone.
pub const DEFY_WIRELESS: DeviceIdentity = DeviceIdentity {
    vendor_id: VENDOR_DYGMA,
    product_id: 0x0012,
    kind: DeviceKind::DefyWireless,
};

/// The first-generation Raise (both layouts): vendor `0x1209`, product
/// id `0x2201` (8705). The ANSI and ISO definitions carry the same
/// pair (`src/api/hardware-dygma-raise-ansi/index.ts` and
/// `src/api/hardware-dygma-raise-iso/index.ts`, first `usb` blocks), as
/// do both shipped Raise backups. Serial only: vendor `0x1209` appears
/// on neither of Bazecor's raw-HID surfaces — its Focus scan's filter
/// spans vendor 13807 alone (`src/api/hid/hid.ts`), and its Lens
/// covers vendor-13807 products only (`src/lens/shared/constants.ts`,
/// `LENS_PRODUCTS`) — so the first-generation Raise connects over USB
/// serial in everything Bazecor does.
pub const RAISE: DeviceIdentity = DeviceIdentity {
    vendor_id: VENDOR_RAISE,
    product_id: 0x2201,
    kind: DeviceKind::Raise,
};

/// The Raise 2: vendor `0x35ef`, product id `0x0021` (33). This is the
/// id Bazecor matches in practice — both Raise 2 definitions carry it
/// (`src/api/hardware-dygma-raise2-ansi/index.ts` and
/// `src/api/hardware-dygma-raise2-iso/index.ts`, first `usb` blocks,
/// which therefore collide across the layouts) and the shipped ANSI
/// record agrees (`src/defaultBackups/Raise2ANSI.json`). Bazecor's HID
/// filter matches it too. Layout (ANSI/ISO) is not encoded in the id:
/// it is probed from the connected device.
pub const RAISE2: DeviceIdentity = DeviceIdentity {
    vendor_id: VENDOR_DYGMA,
    product_id: 0x0021,
    kind: DeviceKind::Raise2,
};

/// The Raise 2 ISO product id Bazecor's own sources disagree about:
/// vendor `0x35ef`, product id `0x0023` (35).
///
/// The operative ISO definition says `0x0021` — colliding with the ANSI
/// one (see [`RAISE2`]) — but the same file's header comment spells the
/// intended scheme out (`0 = bootloader ANSI, 1 = ANSI, 2 = bootloader
/// ISO, 3 = ISO`, i.e. `0x0023` for ISO), and the shipped ISO record
/// carries `"productId": 35` (`src/defaultBackups/Raise2ISO.json`).
/// Two of the three sources therefore attest 35 for a real ISO Raise 2,
/// so this crate matches the union of both ids: matching an id the
/// device does not present is a harmless no-op, while missing one it
/// does present would make the keyboard unconnectable. Needs hardware
/// to adjudicate; until then no claim is made about which is right.
pub const RAISE2_ISO_RECORD: DeviceIdentity = DeviceIdentity {
    vendor_id: VENDOR_DYGMA,
    product_id: 0x0023,
    kind: DeviceKind::Raise2,
};

/// The Sonsei: vendor `0x35ef`, product id `0x0031` (49). Bazecor
/// `src/api/hardware-dygma-sonsei-wireless/index.ts`, first `usb`
/// block; the shipped Sonsei backup's descriptor agrees
/// (`src/defaultBackups/Sonsei.json`). This crate matches it over
/// serial alone, mirroring Bazecor's Focus HID scan — which does not
/// include this id (`src/api/hid/hid.ts`). Absent from that scan is
/// not absent full stop: Bazecor's Lens opens a raw-HID interface at
/// this exact id on the same `0xff00` vendor usage page
/// (`src/lens/shared/constants.ts`, `LENS_PRODUCTS`, via
/// `src/lens/main/raw-hid-listener.ts`), so the Sonsei demonstrably
/// presents one — Focus-over-HID on a Sonsei is merely unattested in
/// Bazecor. If a Sonsei owner ever reports its Focus-over-HID ids,
/// this is the table entry that grows.
pub const SONSEI: DeviceIdentity = DeviceIdentity {
    vendor_id: VENDOR_DYGMA,
    product_id: 0x0031,
    kind: DeviceKind::Sonsei,
};

/// Every identity a Dygma product presents on the serial transport
/// (USB CDC, wired or the wireless Neuron's serial interface), matched
/// by exact (vendor id, product id) pair the way Bazecor's serial scan
/// matches ports.
pub const SERIAL_IDENTITIES: [DeviceIdentity; 6] = [
    DEFY_WIRED,
    DEFY_WIRELESS,
    RAISE,
    RAISE2,
    RAISE2_ISO_RECORD,
    SONSEI,
];

/// Every identity this crate matches on the HID transport (the
/// raw-HID interface the wireless products expose over Bluetooth),
/// matched the way Bazecor's Focus HID scan matches devices: vendor
/// 13807, one of these product ids, usage page 65280, usage id 1.
///
/// Deliberately narrower than [`SERIAL_IDENTITIES`]: Bazecor's Focus
/// HID scan opens product ids `[18, 33]` only (`src/api/hid/hid.ts`)
/// — plus the union's 35, see [`RAISE2_ISO_RECORD`] — and this table
/// mirrors that scan. Absent from it is not absent full stop: the
/// Sonsei demonstrably presents a raw-HID interface (Bazecor's Lens
/// opens one at its id on this very usage page — see [`SONSEI`]), so
/// Focus-over-HID on a Sonsei is unattested in Bazecor, not known to
/// be impossible; the table grows only on attested ids. The
/// first-generation Raise is the one true serial-only product: vendor
/// `0x1209` appears on neither of Bazecor's raw-HID surfaces (see
/// [`RAISE`]).
pub const HID_IDENTITIES: [DeviceIdentity; 3] = [DEFY_WIRELESS, RAISE2, RAISE2_ISO_RECORD];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_values_are_pinned_to_the_bazecor_sources() {
        let expected = [
            (DEFY_WIRED, VENDOR_DYGMA, 16, DeviceKind::DefyWired),
            (DEFY_WIRELESS, VENDOR_DYGMA, 18, DeviceKind::DefyWireless),
            (RAISE, VENDOR_RAISE, 8705, DeviceKind::Raise),
            (RAISE2, VENDOR_DYGMA, 33, DeviceKind::Raise2),
            (RAISE2_ISO_RECORD, VENDOR_DYGMA, 35, DeviceKind::Raise2),
            (SONSEI, VENDOR_DYGMA, 49, DeviceKind::Sonsei),
        ];

        for (identity, vendor_id, product_id, kind) in expected {
            assert_eq!(identity.vendor_id, vendor_id, "{identity:?} vendor");
            assert_eq!(identity.product_id, product_id, "{identity:?} product");
            assert_eq!(identity.kind, kind, "{identity:?} kind");
        }
    }

    #[test]
    fn match_tables_hold_no_duplicate_identity() {
        let tables: [&[DeviceIdentity]; 2] = [&SERIAL_IDENTITIES, &HID_IDENTITIES];

        for table in tables {
            for (i, a) in table.iter().enumerate() {
                for b in &table[i + 1..] {
                    assert_ne!(
                        (a.vendor_id, a.product_id),
                        (b.vendor_id, b.product_id),
                        "duplicate identity {a:?} / {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_serial_identity_of_a_wireless_product_is_also_a_hid_identity_where_bazecor_matches_one()
     {
        // The HID table mirrors Bazecor's Focus HID scan: vendor 13807,
        // product ids 18/33 — plus the union's 35. The Sonsei's
        // raw-HID interface demonstrably exists (Bazecor's Lens opens
        // it), but Focus-over-HID on a Sonsei is unattested there, so
        // its id stays out of this table until attested.
        assert!(HID_IDENTITIES.contains(&DEFY_WIRELESS));
        assert!(!HID_IDENTITIES.contains(&RAISE));
        assert!(!HID_IDENTITIES.contains(&SONSEI));
        assert!(!HID_IDENTITIES.contains(&DEFY_WIRED));
    }

    #[test]
    fn matches_compares_both_ids_and_nothing_else() {
        assert!(DEFY_WIRELESS.matches(VENDOR_DYGMA, 18));
        assert!(!DEFY_WIRELESS.matches(VENDOR_DYGMA, 16));
        assert!(!DEFY_WIRELESS.matches(VENDOR_RAISE, 18));
    }
}
