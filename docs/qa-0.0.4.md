# VibeCon 0.0.4 manual QA

This is a short real-device check for the release candidate. It deliberately
separates verified build behavior from hardware-dependent behavior.

## Install

1. Open `VibeCon_0.0.4_aarch64.dmg` on an Apple Silicon Mac.
2. Move the app to Applications and follow the README's Gatekeeper instructions
   if macOS quarantines the ad-hoc signed preview.
3. Pair one or both Joy-Cons in Bluetooth, then start VibeCon.

## Input and mappings

- Debug detects the selected Joy-Con(s), updates the stick visualizer, and adds
  report rows without app-wide lag.
- In Mappings, select each preset; moving the controllers must not append Debug
  logs while the mapping page is open.
- **Code:** left stick left/right sends previous/next window shortcuts after
  Accessibility has been granted.
- **Codex Cowork:** L D-pad Up and R X focus Codex.
- **Keyboard Focus:** after explicitly enabling the preset, L D-pad Up/Down
  sends Shift+Tab/Tab; R A sends Space to the foreground app.
- Return to Debug and confirm all automation is paused.

## Hardware experiments

- Select a controller in Debug, then click **Test selected Joy-Con vibration**
  on Mappings. Record whether the controller makes one short pulse or the UI
  reports an HID write error.
- If a native `0x30` report appears, confirm that raw L/R IMU values update.
  Seeing “No native 0x30 IMU sample” on compact macOS `0x3F` input is expected.

## Release gate

Do not tag or publish until the chosen controller configuration has passed the
checks above. A vibration failure is not a failure of the mapping release; keep
the experimental output button documented as unsupported on that HID path.
