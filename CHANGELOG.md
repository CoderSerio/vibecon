# Changelog

## 0.0.7

### Pointer control

- Add stick and motion pointer modes with one-handed click, drag, clutch, mode
  switching, and pose recenter controls for both Joy-Cons.
- Normalize motion to the active macOS display, add smooth speed-adaptive gain,
  and expose `30°–120°` full-screen sweep presets.
- Use SL/SR to adjust motion precision with HUD, vibration, and persisted local
  configuration feedback.
- Give the right Joy-Con a verified portrait projection and treat a short
  minus/plus press as a real orientation zero for the pointer and 3D preview.

### Debugging and development

- Keep enabled mappings active on Input logs by default, with a temporary
  **Pause mappings** switch that leaves HID inspection running.
- Package and sign `VibeCon Dev.app` under a stable development identity so
  macOS Accessibility permission survives ordinary local rebuilds.
- Document the pointer pipeline, permissions, installation, and real-device
  verification flow in English and Simplified Chinese.

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
