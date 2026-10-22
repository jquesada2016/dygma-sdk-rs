# Dygma SDK

This repo is the home of a community-maintained set of tools designed for
working with Dygma keyboards in a programmatic fashion.

The goal of this project is to allow for feature parity of Bazecor
via a CLI, as well as a Rust SDK.

## Supported products

The SDK matches and connects to every product Bazecor does — the Defy,
Raise, Raise 2, and Sonsei — over the transports each of them presents
(USB serial for all of them; Bluetooth for the Defy wireless and the
Raise 2, which the SDK also matches over HID — the Sonsei connects
over USB serial only as far as this SDK's matching goes). The
transports carry full large-payload transfers (keymaps, backups,
colormaps) — a fix live-validated on the Defy that the other products'
transports share in code but not in validation.

Support is not the same depth for every product, and this README says
so plainly:

- **Defy** — the reference product. It carries the device-level typed
  APIs (keymap, superkeys, macros, LEDs, settings, wireless,
  backup/restore) and it is the only product validated against real
  hardware.
- **Raise, Raise 2, Sonsei** — transport-level and format-level
  support only, validated *differentially* against Bazecor's own
  sources (its hardware definitions, its parsers, and its shipped
  backups): connection and product identification, backup file
  parsing and conversion between products, restore validation, and
  default configurations. No device-level typed APIs exist for them;
  raw Focus commands (`command list` / `command run`) are the
  supported way to talk to one. When the CLI connects to a non-Defy
  product it says which one it reached and prints this caveat.
- **No non-Defy hardware exists in this project's validation.** If
  you own a Raise, Raise 2, or Sonsei, real-hardware evidence would
  be valuable to the project — `command list` output and a
  `backup restore --dry-run` of one of your Bazecor backups
  especially. Please open an issue with whatever you find.

# Project structure

There are three main parts to this repo.

1. [CLI](/api/src/main.rs)
2. [Rust SDK](/api/src/lib.rs)
3. [Rust macro Keymap key code generator](/macros/src/lib.rs)

The only fancy thing going on in this project is the Rust macro crate that is
used to define and generate key kinds, which are used in keymaps, superkeys,
and macros.

You can find the [key definitions here](/api/src/focus_api/parsing/keymap/keycode_tables.rs).

# Building and Running

To build and run the CLI, you will need to have Rust installed.

To install Rust, [follow these instructions](https://rust-lang.org/tools/install/).

Once you have Rust installed, run the following command to get documentation
on each available command:

```sh
cargo r -- --help
```

## Example commands

The following command will read the keymap on your Defy and save it to a file
called `keymap.json`:

```sh
cargo r -- keymap new keymap.json
```

The following command will get a list of available commands on the device
itself (via the Focus API which is part of the firmware):

```sh
cargo r -- command list
```

**Node**: the `--` between `cargo r` and `keymap new keymap.json` is only
required if you run the CLI using cargo. If you run the binary directly, you
should omit the `--`. It is used by `cargo` to disambiguate between it's own
flags and those of the binary that will be executed.

# Rust Developer Documentation

This project will eventually be released on [crates.io](https://crates.io/),
and once published there, will have documentation available on [docs.rs](https://docs.rs).

In the meantime, you can build and view the docs locally by running
the following command.

```sh
cargo doc
```
