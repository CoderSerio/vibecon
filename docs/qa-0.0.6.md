# VibeCon 0.0.6 manual QA

This checklist covers the distributable fused-orientation preview. Release
builds intentionally exclude local third-party GLBs and use the procedural
Three.js Joy-Con instead.

## Install

1. Open `VibeCon_0.0.6_aarch64.dmg` on an Apple Silicon Mac.
2. Move the app to Applications and follow the README's Gatekeeper instructions
   if macOS quarantines the ad-hoc-signed preview.
3. Pair one or both Joy-Cons over Bluetooth, then start VibeCon.

## Packaged 3D fallback

- Both cards report **Procedural model · live input preview** in a clean release
  build; neither canvas is blank.
- The application bundle contains no stand-alone `.glb` files.
- Sticks tilt as complete assemblies and pressed face, rail, and shoulder
  controls highlight the corresponding procedural geometry.

## Fused orientation

- Hold a Joy-Con upright in portrait orientation, enable motion following, and
  press **Reset** once.
- Rotating around the controller's width, front normal, and long axis moves the
  model around the matching axis without translating it away from center.
- Slow rotations remain visible, while a stationary controller settles without
  continuous drift severe enough to obscure deliberate input.
- Repeat the check for Joy-Con (L) and Joy-Con (R); their physical axes must use
  the same portrait convention despite the mirrored right-hand IMU chip.

## Existing behavior

- Enabled Codex Cowork bindings still switch windows and focus Codex after the
  required macOS permission is granted.
- Returning to Debug pauses mappings so controller input cannot disrupt
  inspection.
- Manual vibration testing produces one short pulse or reports a clear HID
  write error without retrying indefinitely.

## Automated release gate

```sh
pnpm install --frozen-lockfile
pnpm test:motion
pnpm build
cd src-tauri
cargo fmt --check
cargo test --lib
cargo check
```

The tag may be published after these checks, a clean build contains no GLBs,
and the packaged procedural models have been visually confirmed.
