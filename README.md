<p align="center">
  <img src="./docs/images/logo.png" width="148" alt="VibeCon logo">
</p>

<h1 align="center">VibeCon</h1>

<p align="center">
  Turn an unused controller into an inspectable control surface for vibe coding.
</p>

<p align="center">
  <a href="#quick-start"><img src="https://img.shields.io/badge/platform-macOS%20first-111827?style=flat-square" alt="macOS first"></a>
  <a href="#current-capabilities"><img src="https://img.shields.io/badge/status-experimental-F97316?style=flat-square" alt="experimental"></a>
  <a href="#windows"><img src="https://img.shields.io/badge/Windows-planned-2563EB?style=flat-square" alt="Windows planned"></a>
  <a href="./README_CN.md"><img src="https://img.shields.io/badge/README-%E4%B8%AD%E6%96%87-0AB9E6?style=flat-square" alt="Chinese README"></a>
</p>

<p align="center"><a href="./README_CN.md">中文文档</a></p>

<p align="center">
  <img src="./docs/images/vibecon-debug-lab.png" width="860" alt="VibeCon's Joy-Con debug interface">
</p>

> **Early prototype.** Built and tested on macOS; the Tauri + Rust architecture is intentionally portable, but Windows input and window-switching have not yet been validated.

## Install the macOS preview

1. Download the `.dmg` for your Apple Silicon Mac from [GitHub Releases](https://github.com/CoderSerio/vibecon/releases).
2. Open it and drag `VibeCon.app` to **Applications**.
3. This preview is ad-hoc signed but is **not Apple-notarized** yet. macOS may show an "Apple cannot verify" warning. Run the following once in Terminal after moving the app to Applications:

   ```sh
   xattr -dr com.apple.quarantine "/Applications/VibeCon.app"
   ```

4. Open `VibeCon` from Applications, then grant Accessibility only when you enable the experimental window-switch mapping.

Only run this command for a release downloaded from this repository. Developer ID signing and Apple notarization are planned for a future public release.

## What is it?

VibeCon starts from a simple idea: a Joy-Con—or another controller you already own—can be a better physical control surface for AI-assisted coding than an expensive, opaque keyboard.

Before it automates anything, VibeCon makes the controller observable: raw HID reports, live sticks, button highlights, sampling, and local labels. Then you can opt into a small, reviewable mapping.

## Current capabilities

- Detect paired **Joy-Con (L)** and **Joy-Con (R)** devices through the native Tauri/Rust HID backend.
- Inspect one or both controllers at once; grouped logs keep a single timestamp with aligned L/R report rows.
- Decode native `0x30` and macOS compact `0x3F` Joy-Con reports, including button bitfields and the observed eight-way HAT profile.
- Visualize both Joy-Cons in CSS: sticks move live; held controls glow and taps briefly persist as afterglow.
- Choose a log policy: key operations, legacy 75 ms snapshots, 60/30/10 Hz samples, or every report; clear the visible log whenever needed.
- Label captured reports as stick positions or button press/release samples. Labels are stored locally in `~/.vibecon/annotations.jsonl` and shown again for matching reports.
- **Experimental macOS mapping:** while on the **Mappings** tab, firmly push the left stick right/left for next/previous `Cmd+Tab` window switching. Its opt-in configuration persists in `~/.vibecon/mapping-settings.json`, but is temporarily inactive while the Debug page is open.

### Observed Joy-Con notes

On the current macOS Bluetooth HID path, compact `0x3F` reports expose Joy-Con (L)'s stick as an eight-way HAT: values `0–7` are directions and `8` is neutral. Button fields are bitmasks—decode each byte with bitwise AND/OR, not as one additive HEX value.

![Observed macOS Joy-Con HAT reports](./docs/images/macos-joycon-hat-debug.png)

## Quick start

### 1. Pair a Joy-Con

1. Detach and charge the Joy-Con.
2. Hold the small **Sync** button until the player LEDs sweep.
3. Open **System Settings → Bluetooth** and connect `Joy-Con (L)` or `Joy-Con (R)`.
4. Pair both sides separately to inspect both simultaneously.

If it does not appear, repeat Sync and keep the Switch away or powered down so it cannot reconnect first.

### 2. Run the native app

Requires current Node.js, pnpm, and a Rust toolchain.

```sh
cd /Users/carbon/Desktop/vibecon
pnpm install
pnpm tauri dev
```

Click **Refresh controllers**, select one or both Joy-Cons, then move a stick or press a button.

> Do not use `pnpm dev` for controller testing. It starts only the browser UI, without Tauri's Rust backend or local HID access.

## Enable macOS window switching

The experimental mapping uses macOS `System Events` to send `Cmd+Tab`. macOS requires explicit permission.

1. Build a debug app once (this produces a stable app bundle):

   ```sh
   pnpm tauri build --debug
   ```

2. Open **System Settings → Privacy & Security → Accessibility**.
3. Click **+**, add and enable:

   ```text
   /Users/carbon/Desktop/vibecon/src-tauri/target/debug/bundle/macos/VibeCon.app
   ```

4. If macOS asks, allow VibeCon to control **System Events** under **Privacy & Security → Automation**.
5. Restart `pnpm tauri dev`, open **Mappings**, and explicitly enable the checkbox.

The mapping now posts the shortcut directly through macOS Quartz, so **Accessibility for VibeCon** is the permission that matters; it does not depend on a separate `osascript` Automation hop. The mapping is deliberately disabled while the Debug page is open to avoid corrupting controller inspection.

## Development

```sh
pnpm build                    # TypeScript + Vite
cd src-tauri && cargo check   # Rust/Tauri/HID backend
```

```text
src/App.vue             Vue debug and mapping UI
src/components/JoyCon.vue  Live CSS controller visualizer
src-tauri/src/lib.rs    HID stream, Joy-Con decoding, native commands
docs/images/            Logo and README screenshots
```

## Windows

The desktop UI and controller logic are not tied to Swift or macOS APIs. However, the current window mapping is macOS-only and Windows HID behavior still needs real-device testing. Windows support is a product goal—not a claim of current compatibility.

## Roadmap

- Profiles and calibration for Joy-Con, DualSense, Xbox, and 8BitDo.
- More deliberate mappings, with clear per-platform permissions.
- Spatial / motion input exploration.
- Exportable, shareable controller profiles.

## Privacy

Controller reports and annotations remain local. VibeCon sends no telemetry. The only automated action today is the explicitly enabled experimental macOS window switch; it does not execute shell commands or approve AI-agent actions.

## License

License selection is intentionally deferred until the first general-purpose controller profile is validated.
