//! The app-side neuron names store.
//!
//! The device stores numbers only: layer, macro, and superkey *names*
//! live app-side, keyed by the Neuron's `hardware.chip_id` — exactly
//! where Bazecor keeps them (its per-app `neurons` store,
//! `bazecor/src/renderer/utils/Store.ts`). This module holds the same
//! store for this crate: a map from chip id to the neuron object, which
//! is the same object a backup file carries as its `neuron`
//! ([`DefyBackup::neuron`]). Because the backup file's neuron object
//! *is* the names store entry, importing a backup's names and producing
//! a backup that carries names are the same operation: the store holds
//! [`BackupNeuron`] values verbatim.
//!
//! The CLI persists the store as one JSON object keyed by chip id, by
//! default at `$HOME/.dygma-sdk/names.json` (override with the
//! `--store` flag on the `names` commands and `backup restore`). A
//! missing file loads as an empty store; entries are ordered by chip id
//! so the file is deterministic across writes.
//!
//! [`DefyBackup::neuron`]:
//!     crate::focus_api::parsing::backup::DefyBackup::neuron

use std::collections::BTreeMap;

use crate::focus_api::parsing::backup::{BackupNeuron, DefyBackup};

/// The neuron names store: one [`BackupNeuron`] entry per Neuron, keyed
/// by the Neuron's `hardware.chip_id`.
///
/// Entries are replaced wholesale when a backup is applied — Bazecor's
/// restore overwrite behavior (`Backup.ts` `restoreBackup`:
/// `localNeurons[index] = backup.neuron`, with the entry's `id` forced
/// back to the device's chip id). The chip id a backup is applied
/// under is always explicit at the call site: a device-backed
/// `backup restore` passes the connected Neuron's probed chip id, so
/// names travel to the Neuron the backup was restored on — which need
/// not be the Neuron that captured it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NeuronNameStore(BTreeMap<String, BackupNeuron>);

impl NeuronNameStore {
    /// An empty store.
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// The stored entry for a Neuron chip id, if any.
    pub fn get(&self, chip_id: &str) -> Option<&BackupNeuron> {
        self.0.get(chip_id)
    }

    /// Iterates over the stored `(chip id, entry)` pairs, ordered by
    /// chip id.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &BackupNeuron)> {
        self.0
            .iter()
            .map(|(chip_id, neuron)| (chip_id.as_str(), neuron))
    }

    /// The number of entries in the store.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the store holds no entries.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Applies a backup's neuron object to the store under `chip_id`:
    /// the entry keyed by `chip_id` is replaced wholesale with the
    /// backup's neuron, its `id` forced to `chip_id` — Bazecor's
    /// restore overwrite. The entry's names come from the backup, its
    /// key does not: `backup restore` passes the connected device's
    /// probed `hardware.chip_id`, so a backup captured from one Neuron
    /// carries its names to the Neuron it is restored on, while the
    /// offline `names import` passes the backup's own `neuronID`.
    pub fn apply_backup(&mut self, backup: &DefyBackup, chip_id: &str) {
        self.0.insert(
            chip_id.to_owned(),
            BackupNeuron {
                id: chip_id.to_owned(),
                ..backup.neuron.clone()
            },
        );
    }

    /// Whether applying `backup` under `chip_id` would replace a
    /// stored entry that carries names with one that carries none.
    ///
    /// This is the warn-before-applying case the CLI surfaces: backups
    /// captured by this crate before names were stored carry the empty
    /// neuron shape, and applying one would silently erase a named
    /// entry (layer names, macros and superkeys names, and the
    /// keyboard's name). The check runs on the same `chip_id` the
    /// apply targets.
    pub fn replaces_named_entry(&self, backup: &DefyBackup, chip_id: &str) -> bool {
        self.0
            .get(chip_id)
            .is_some_and(|stored| stored.has_names() && !backup.neuron.has_names())
    }

    /// Sets the keyboard's user-chosen name on a Neuron's entry,
    /// creating the entry if the store does not hold one yet.
    pub fn set_keyboard_name(&mut self, chip_id: &str, name: &str) {
        self.entry_mut(chip_id).name = name.to_owned();
    }

    /// Sets one layer's name on a Neuron's entry, creating the entry if
    /// needed and upserting the `{id, name}` layer entry — the shape
    /// every shipped Bazecor store carries. Layer ids are 0-based, as
    /// the device and Bazecor number them.
    ///
    /// An existing layer entry is matched by its numeric `id` field and
    /// only its `name` is rewritten, so foreign extra fields on that
    /// entry survive.
    pub fn set_layer_name(&mut self, chip_id: &str, layer_id: u8, name: &str) {
        let entry = self.entry_mut(chip_id);

        match entry.layers.iter_mut().find(|layer| {
            layer.get("id").and_then(serde_json::Value::as_i64) == Some(i64::from(layer_id))
        }) {
            Some(layer) => layer["name"] = name.into(),
            None => entry
                .layers
                .push(serde_json::json!({ "id": layer_id, "name": name })),
        }
    }

    /// The mutable entry for a chip id, created (with its `id` set to
    /// the chip id, as every store entry carries) if absent.
    fn entry_mut(&mut self, chip_id: &str) -> &mut BackupNeuron {
        let entry = self.0.entry(chip_id.to_owned()).or_default();

        if entry.id.is_empty() {
            entry.id = chip_id.to_owned();
        }

        entry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::focus_api::parsing::backup::BackupCommand;

    /// A trimmed copy of the shipped Defy wireless backup's neuron
    /// object — the real store entry shape a backup import feeds the
    /// store (the same JSON the phase-1 interop fixture carries).
    const SILVER_DEFY_BACKUP_JSON: &str = r#"{
      "neuronID": "8ed7f33db024a5b5",
      "neuron": {
        "id": "8ed7f33db024a5b5",
        "name": "Silver Defy",
        "layers": [
          { "id": 0, "name": "Base" },
          { "id": 1, "name": "Navigation" }
        ],
        "macros": [],
        "superkeys": [],
        "device": { "info": { "product": "Defy" } }
      },
      "versions": { "bazecor": "v1.1.0" },
      "backup": [
        { "command": "keymap.custom", "data": "41 30 31" }
      ]
    }"#;

    /// A name-less backup like [`DefyKeyboard::create_backup`] produces:
    /// the empty neuron store shape with only the chip id and a device
    /// descriptor.
    ///
    /// [`DefyKeyboard::create_backup`]:
    ///     crate::devices::defy::DefyKeyboard::create_backup
    fn nameless_backup(neuron_id: &str) -> DefyBackup {
        DefyBackup {
            neuron_id: neuron_id.to_owned(),
            neuron: BackupNeuron {
                id: neuron_id.to_owned(),
                ..BackupNeuron::default()
            },
            versions: Default::default(),
            backup: vec![BackupCommand::new(
                "keymap.custom".to_owned(),
                "41 30 31".to_owned(),
            )],
            partial: None,
        }
    }

    #[test]
    fn store_round_trips_as_one_object_keyed_by_chip_id() {
        let mut store = NeuronNameStore::new();
        store.set_keyboard_name("8ed7f33db024a5b5", "Silver Defy");
        store.set_layer_name("8ed7f33db024a5b5", 0, "Base");

        let json = serde_json::to_string_pretty(&store).unwrap();

        // The file shape: one JSON object, chip id keys, Bazecor's
        // neuron object as each value.
        assert_eq!(
            json,
            r#"{
  "8ed7f33db024a5b5": {
    "id": "8ed7f33db024a5b5",
    "name": "Silver Defy",
    "layers": [
      {
        "id": 0,
        "name": "Base"
      }
    ],
    "macros": [],
    "superkeys": []
  }
}"#
        );

        let reparsed = serde_json::from_str::<NeuronNameStore>(&json).unwrap();

        assert_eq!(reparsed, store);
    }

    #[test]
    fn empty_object_loads_as_an_empty_store() {
        let store = serde_json::from_str::<NeuronNameStore>("{}").unwrap();

        assert!(store.is_empty());
        assert_eq!(store.get("anything"), None);
    }

    #[test]
    fn apply_backup_inserts_keyed_by_the_given_chip_id_with_the_id_forced() {
        let mut store = NeuronNameStore::new();
        let backup = serde_json::from_str::<DefyBackup>(SILVER_DEFY_BACKUP_JSON).unwrap();

        // The offline `names import` behavior: keyed by the backup
        // file's own `neuronID`.
        store.apply_backup(&backup, &backup.neuron_id);

        assert_eq!(store.len(), 1);

        let entry = store.get("8ed7f33db024a5b5").unwrap();

        // The imported names survived, and the entry's id is the chip
        // id it is keyed by.
        assert_eq!(entry.id, "8ed7f33db024a5b5");
        assert_eq!(entry.name, "Silver Defy");
        assert_eq!(entry.layers[0]["name"], "Base");
    }

    #[test]
    fn apply_backup_overwrites_wholesale_like_bazecor() {
        // Bazecor's restore replaces the stored neuron with the
        // backup's, keeping only the chip id: applying the Silver Defy
        // backup onto a store entry with other names replaces every
        // name.
        let mut store = NeuronNameStore::new();
        store.set_keyboard_name("8ed7f33db024a5b5", "My workshop board");
        store.set_layer_name("8ed7f33db024a5b5", 0, "Workshop");

        let backup = serde_json::from_str::<DefyBackup>(SILVER_DEFY_BACKUP_JSON).unwrap();
        store.apply_backup(&backup, "8ed7f33db024a5b5");

        let entry = store.get("8ed7f33db024a5b5").unwrap();

        assert_eq!(entry.name, "Silver Defy");
        assert_eq!(entry.layers.len(), 2);
        assert_eq!(entry.layers[0]["name"], "Base");
    }

    #[test]
    fn apply_backup_can_carry_names_across_neurons() {
        // A backup captured from one Neuron, applied under the chip id
        // of the Neuron it is restored on (Bazecor keys the entry by
        // the device's chip id, whatever the backup's own id says —
        // the backup's `neuronID` stays untouched here).
        let mut store = NeuronNameStore::new();
        let backup = serde_json::from_str::<DefyBackup>(SILVER_DEFY_BACKUP_JSON).unwrap();

        store.apply_backup(&backup, "another-chip-id");

        let entry = store.get("another-chip-id").unwrap();

        assert_eq!(entry.id, "another-chip-id");
        assert_eq!(entry.name, "Silver Defy");
        assert_eq!(store.get("8ed7f33db024a5b5"), None);
    }

    #[test]
    fn restore_applies_names_under_the_connected_chip_id_not_the_file_id() {
        // The `backup restore` flow, at the store level: the connected
        // device's chip id is probed and BOTH the overwrite warning and
        // the apply run under it. Keying by the backup file's own id
        // instead (the round-1 defect) rewrote the capturing Neuron's
        // entry and left the restored-on Neuron nameless — the explicit
        // key keeps the two apart.
        let mut store = NeuronNameStore::new();
        store.set_keyboard_name("device-chip", "My board");
        store.set_layer_name("device-chip", 0, "Base");

        let backup = nameless_backup("backup-file-id");

        // The warn and the apply key on the device, not the file: a
        // name-less backup would erase the connected Neuron's names
        // even though the file carries a different id, and the file's
        // id has no entry to warn about.
        assert!(store.replaces_named_entry(&backup, "device-chip"));
        assert!(!store.replaces_named_entry(&backup, "backup-file-id"));

        store.apply_backup(&backup, "device-chip");

        let entry = store.get("device-chip").unwrap();

        assert_eq!(entry.id, "device-chip");
        assert_eq!(entry.name, "");
        assert_eq!(entry.layers.len(), 0);
        // The capturing Neuron's entry was never touched.
        assert_eq!(store.get("backup-file-id"), None);
    }

    #[test]
    fn replaces_named_entry_warns_only_for_nameless_replacements() {
        let mut store = NeuronNameStore::new();
        store.set_keyboard_name("named", "Silver Defy");

        let named = nameless_backup("named");
        let nameless_missing = nameless_backup("not-in-store");
        let mut named_incoming = nameless_backup("named");
        named_incoming.neuron.name = "Renamed".to_owned();

        // A name-less backup replacing the named entry is the warn
        // case.
        assert!(store.replaces_named_entry(&named, "named"));
        // Restoring onto a Neuron the store does not know is not.
        assert!(!store.replaces_named_entry(&nameless_missing, "not-in-store"));
        // A backup carrying its own names replaces freely.
        assert!(!store.replaces_named_entry(&named_incoming, "named"));

        // An entry without names is not "named", so nothing to lose.
        let mut store = NeuronNameStore::new();
        store.apply_backup(&nameless_backup("bare"), "bare");

        assert!(!store.replaces_named_entry(&nameless_backup("bare"), "bare"));
    }

    #[test]
    fn set_keyboard_name_creates_the_entry_with_the_chip_id() {
        let mut store = NeuronNameStore::new();
        store.set_keyboard_name("8ed7f33db024a5b5", "Silver Defy");

        let entry = store.get("8ed7f33db024a5b5").unwrap();

        assert_eq!(entry.id, "8ed7f33db024a5b5");
        assert_eq!(entry.name, "Silver Defy");
        assert!(entry.layers.is_empty());
    }

    #[test]
    fn set_layer_name_upserts_by_numeric_id() {
        let mut store = NeuronNameStore::new();

        // Creating on an entry-less store.
        store.set_layer_name("chip", 0, "Base");
        store.set_layer_name("chip", 1, "Navigation");

        let entry = store.get("chip").unwrap();

        assert_eq!(
            entry.layers,
            vec![
                serde_json::json!({ "id": 0, "name": "Base" }),
                serde_json::json!({ "id": 1, "name": "Navigation" }),
            ]
        );

        // Renaming an existing layer rewrites only its name.
        store.set_layer_name("chip", 0, "Workshop");

        let entry = store.get("chip").unwrap();

        assert_eq!(
            entry.layers[0],
            serde_json::json!({ "id": 0, "name": "Workshop" })
        );
        assert_eq!(entry.layers.len(), 2);
    }

    #[test]
    fn set_layer_name_on_a_foreign_entry_rewrites_only_the_name() {
        // A real store entry shape with an extra field (the shipped
        // Sonsei superkey entries carry extras): renaming keeps it.
        let mut store = serde_json::from_str::<NeuronNameStore>(
            r#"{ "chip": { "id": "chip", "name": "", "layers": [ { "id": 0, "name": "Base", "custom": true } ], "macros": [], "superkeys": [] } }"#,
        )
        .unwrap();

        store.set_layer_name("chip", 0, "Workshop");

        assert_eq!(
            store.get("chip").unwrap().layers[0],
            serde_json::json!({ "id": 0, "name": "Workshop", "custom": true })
        );
    }

    #[test]
    fn store_orders_entries_by_chip_id() {
        let mut store = NeuronNameStore::new();
        store.set_keyboard_name("zzz", "Later");
        store.set_keyboard_name("aaa", "Earlier");

        let chip_ids: Vec<&str> = store.iter().map(|(chip_id, _)| chip_id).collect();

        assert_eq!(chip_ids, ["aaa", "zzz"]);
    }
}
