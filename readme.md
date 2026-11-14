# Gig Tools
Some useful midi effects for keyboard players on stage
- midi_panic: sends note off message to all 16 channels and keys
- top_liner: detects top notes and filters out rest of the notes (introduces some latency for chord detection)

## Licencing
- Both CLAP and VST3 versions use the ISC-licensed nice-plug framework.

## Building
Install the current stable Rust toolchain (Rust 1.88 or newer) and a platform C/C++ linker. Dependencies are downloaded from crates.io; no Git submodules are needed.

At workspace root, run
```
cargo xtask bundle [plugin_name] --release
```
