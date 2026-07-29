# Changelog

## 0.0.4

### New features

- Add developer-oriented mapping presets: Code, Codex Cowork, Inspect Only,
  and the opt-in Keyboard Focus experiment.
- Persist safe, agent-editable mapping JSON at `~/.vibecon/mappings.json`,
  with per-preset and per-binding switches.
- Add English and Simplified Chinese UI support.
- Add a manual, short Joy-Con vibration test and raw native `0x30` IMU sample
  inspection.

### Safety and release tooling

- Pause all mappings on the Debug page and validate mapping control/action ids.
- Add Semifold changeset/release configuration and an explicit Tauri version
  synchronization script.

## 0.0.2

- Initial macOS Joy-Con inspector preview.
