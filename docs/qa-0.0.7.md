# VibeCon 0.0.7 manual QA

This checklist covers the first practical Joy-Con pointer-control preview.
Release builds continue to use the distributable procedural Three.js Joy-Con;
local third-party reference GLBs are intentionally excluded.

## Install and identity

1. Open `VibeCon_0.0.7_aarch64.dmg` on an Apple Silicon Mac and move the app
   to Applications.
2. Follow the README's Gatekeeper command if macOS quarantines the ad-hoc-signed
   preview.
3. Confirm the release bundle identifier remains `io.coderserio.vibecon`.
4. Pair one or both Joy-Cons over Bluetooth, start VibeCon, and grant
   Accessibility to this installed application before testing system output.

## Stick pointer mode

- Either Joy-Con stick moves the cursor continuously with a center deadzone and
  acceleration curve.
- L/R sends left click and supports dragging; ZL/ZR sends right click.
- Holding minus/plus for about 600 ms switches to Motion mode once, without
  also firing the short-press recenter action.

## Motion pointer mode

- Left and right Joy-Cons move the cursor in the same physical up/down
  direction; the right Joy-Con applies only its required horizontal inversion.
- A short minus/plus press establishes the current physical pose as the new
  zero without jumping the cursor; the 3D preview recenters at the same time.
- Holding ZL/ZR freezes the cursor while the hand is repositioned, then resumes
  without a release jump.
- L/R sends left click and drag; ZL+L or ZR+R sends right click.
- Slow rotation remains precise, ordinary rotation remains proportional, and a
  fast sweep can cross the display without requiring an extreme wrist angle.

## Motion range feedback

- The default range is `60° / screen width`.
- SL steps toward the more precise `90°` and `120°` ranges; SR steps toward the
  faster `45°` and `30°` ranges.
- Each accepted step shows a temporary HUD, sends one gentle vibration pulse,
  and persists `motion.sweepDegrees` in `~/.vibecon/mappings.json`.

## Input logs safety switch

- **Pause mappings** is visible only on Input logs and is off after a fresh app
  launch.
- With the switch off, enabled shortcuts and pointer control continue while raw
  reports and the 3D preview remain live.
- With the switch on, shortcuts and pointer output stop while raw HID logging,
  button highlights, sticks, and orientation sampling continue.
- Controls always follow their saved configuration on the Controls page.

## Automated release gate

```sh
pnpm install --frozen-lockfile
pnpm test:motion
pnpm build
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Publish the tag only after these checks pass, the packaged bundle contains no
stand-alone GLBs, and the release and development Bundle IDs remain
`io.coderserio.vibecon` and `io.coderserio.vibecon.dev` respectively.
