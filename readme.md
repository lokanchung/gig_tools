# Gig Tools
Some useful midi effects for keyboard players on stage
- midi_panic: sends note off message to all 16 channels and keys
- top_liner: detects top notes and filters out rest of the notes (introduces some latency for chord detection)

## Licencing
- VST3 version falls under GPLv3, CLAP version is ISC

## Building
At workspace root, run
```
cargo xtask bundle [plugin_name] --release
```
