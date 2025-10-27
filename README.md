# Dygma SDK

This repo is the home of a community-maintained set of tools designed for
working with Dygma keyboards in a programatic fassion.

The goal of this project is to allow for feature parity of Bazecor
via a CLI, as well as a Rust SDK.

Currently, only the Defy is supported, but adding support for the Raise 1 and 2
is easy, though testing would not be, as I only have the Defy.

All connection methods are supported, including wired, wireless over
RF, and wireless over Bluetooth LE.

**Note**: Currently, sending large data payloads to the Defy is failing over
BLE, so try to avoid commands while connected over it until this message is
removed.

# Project structure

There are three main parts to this repo.

1. {CLI](/api/src/main.rs)
2. [Rust SDK](/api/src/lib.rs)
3. [Rust macro Keymap key code generator](/macros/src/lib.rs)

The only fancy thing going on in this project is the Rust macro crate that is
used to define and generate key kinds, which are used in keymaps, superkeys,
and macros.

You can find the [key definitions here](/api/src/parsing/keymap/keycode_tables.rs).

# Building and Running

To build and run the CLI, you will need to have Rust installed.

To install Rust, [follow these instructions](https://rust-lang.org/tools/install/).

Once you have Rust installed, run the following command to get documentation
on each available command:

```sh
cargo r -- --help
```

## Example command

The following command will read the keymap on your Defy and save it to a file
called `keymap.json`:

```sh
cargo r -- keymap new keymap.json
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
