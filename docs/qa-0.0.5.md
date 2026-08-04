# VibeCon 0.0.5 manual QA

This checklist records the real-device release gate for the fused-orientation
preview. It separates deterministic build checks from hardware checks.

## Install

1. Open `VibeCon_0.0.5_aarch64.dmg` on an Apple Silicon Mac.
2. Move the app to Applications and follow the README's Gatekeeper instructions
   if macOS quarantines the ad-hoc-signed preview.
3. Pair one or both Joy-Cons over Bluetooth, then start VibeCon.

## Input and 3D controls

- Debug detects each selected Joy-Con and updates sticks, buttons, and the raw
  report ring buffer without progressively slowing down.
- Pressed face, rail, and shoulder controls highlight the matching part of the
  3D model; releasing a control restores its base material.
- Analogue stick movement tilts the complete stick assembly rather than only
  rotating its cap.

## Fused orientation

- Hold a Joy-Con upright in portrait orientation, enable motion following, and
  press **Reset** once.
- Rotating around the controller's width, front normal, and long axis moves the
  model around the matching axis without translating it away from center.
- Slow rotations remain visible, while a stationary controller settles without
  continuous drift severe enough to obscure deliberate input.
- The diagnostic line reports `fusion-ahrs`, gyro XYZ, sample period, bias
  state, and accelerometer state.
- Repeat the check for Joy-Con (L) and Joy-Con (R); their physical axes must use
  the same portrait convention despite the mirrored right-hand IMU chip.

## Existing behavior

- On Mappings, the enabled Codex Cowork bindings still switch windows and focus
  Codex after the required macOS permission is granted.
- Returning to Debug pauses mappings so controller input cannot disrupt
  inspection.
- Manual vibration testing produces one short pulse or reports a clear HID
  write error without retrying indefinitely.

## Automated release gate

Run before tagging:

```sh
pnpm install --frozen-lockfile
pnpm test:motion
pnpm build
cd src-tauri
cargo fmt --check
cargo test --lib
cargo check
```

The `v0.0.5` tag may be published after these checks pass and the real-device
orientation checks above have been confirmed.
