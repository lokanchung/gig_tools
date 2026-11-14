# Gig Tools
Some useful midi effects for keyboard players on stage
- midi_panic: sends note off message to all 16 channels and keys
- top_liner: detects top notes and filters out rest of the notes (introduces some latency for chord detection)
- program_change_wire: broadcasts Program Change events between tracks using Sender and Receiver plugins

## Licencing
- Both CLAP and VST3 versions use the ISC-licensed nice-plug framework.

## Building
Install the current stable Rust toolchain (Rust 1.88 or newer) and a platform C/C++ linker. Dependencies are downloaded from crates.io; no Git submodules are needed.

At workspace root, run
```
cargo xtask bundle [plugin_name] --release
```

## Program Change Wire

Build with `cargo xtask bundle program_change_wire --release`. The single
`program_change_wire.clap` bundle contains **Program Change Wire Sender** and
**Program Change Wire Receiver**. Install that bundle in your host's CLAP plugin
directory and rescan plugins.

Place a Sender on the track receiving MIDI Program Change (PC), and a Receiver
before the instrument or MIDI output that should receive it. All Senders broadcast
to all active Receivers in the same DAW process, preserving MIDI channel and
program number. Both plugins pass their local MIDI input through. Only PC events
are sent over the wire; bank select CCs, notes, and other messages stay local.
Receivers do not rebroadcast received or local events.

The wire delivers each PC at offset zero of the Receiver's next processing callback.
Track scheduling can add a processing block of delay; it is not sample accurate
across tracks. Duplicate PCs are preserved. Newly activated Receivers do not replay
earlier events, and reset/deactivation clears pending PCs. Keep Receiver tracks
active: bypassed or host-suspended plugins cannot output MIDI.

Communication is shared by instances of the same loaded CLAP library. Separate
host processes, sandboxed instances, and independently loaded copies of the
library do not share a wire. This plugin is CLAP-only because nice-plug's VST3
wrapper cannot receive PC events.

The bus supports up to 64 active Receivers per loaded library (additional Receivers
fail activation), with a 1024-event queue per Receiver. Full queues drop new PCs
without blocking other Receivers. A full host MIDI output buffer defers queued PCs
until the next callback. The audio callbacks use preallocated, lock-free queues.

Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`
to check the implementation.
