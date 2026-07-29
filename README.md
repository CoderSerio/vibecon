<p align="center">
  <img src="./docs/images/logo.png" width="148" alt="VibeCon logo">
</p>

<h1 align="center">VibeCon</h1>

<p align="center">
  Turn an unused controller into an inspectable control surface for vibe coding.
</p>

<p align="center">
  <a href="#install-the-macos-preview"><img src="https://img.shields.io/badge/platform-macOS%20first-111827?style=flat-square" alt="macOS first"></a>
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

<p align="center">
  <a href="./docs/media/vibecon-window-switch-demo.mp4">
    <img src="./docs/media/vibecon-window-switch-demo.gif" width="720" alt="Joy-Con switching windows with VibeCon">
  </a><br>
  <sub>Joy-Con window switching in action. Click to view the source video.</sub>
</p>

## Current capabilities

- Detect paired **Joy-Con (L)** and **Joy-Con (R)** devices through the native Tauri/Rust HID backend.
- Inspect one or both controllers at once; grouped logs keep a single timestamp with aligned L/R report rows.
- Decode native `0x30` and macOS compact `0x3F` Joy-Con reports, including button bitfields and the observed eight-way HAT profile.
- Visualize both Joy-Cons in CSS: sticks move live; held controls glow and taps briefly persist as afterglow.
- Choose a log policy: key operations, legacy 75 ms snapshots, 60/30/10 Hz samples, or every report; clear the visible log whenever needed.
- Label captured reports as stick positions or button press/release samples. Labels are stored locally in `~/.vibecon/annotations.jsonl` and shown again for matching reports.
- **Experimental macOS mappings:** push the left stick right/left for next/previous `Cmd+Tab`, or focus Codex with Joy-Con (L) D-pad Up / Joy-Con (R) X. Each mapping is opt-in, persisted in `~/.vibecon/mapping-settings.json`, and inactive on the Debug page.

### Observed Joy-Con notes

On the current macOS Bluetooth HID path, compact `0x3F` reports expose Joy-Con (L)'s stick as an eight-way HAT: values `0–7` are directions and `8` is neutral. Button fields are bitmasks—decode each byte with bitwise AND/OR, not as one additive HEX value.

![Observed macOS Joy-Con HAT reports](./docs/images/macos-joycon-hat-debug.png)

## Run from source

Requires current Node.js, pnpm, and a Rust toolchain.

```sh
cd /Users/carbon/Desktop/vibecon
pnpm install
pnpm tauri dev
```

Pair a Joy-Con from **System Settings → Bluetooth** first. In VibeCon, click **Refresh controllers**, select one or both Joy-Cons, then move a stick or press a button.

> Do not use `pnpm dev` for controller testing. It starts only the browser UI, without Tauri's Rust backend or local HID access.

## Configure macOS mappings

1. On **Debug**, select the controller you want to use. Select both Joy-Cons if you want both Codex-focus shortcuts.
2. Open **Mappings** and enable the mapping you want. Debug rendering and logging pause while this page is active, but the HID reader remains active for mappings.
3. For window switching, grant **Accessibility** to VibeCon. The Mappings page can open the exact System Settings pane if it is missing.

Window switching posts a native macOS Quartz shortcut, so **Accessibility** is the only permission it requires. Focusing Codex uses the macOS application launcher and does not require Accessibility.

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

Controller reports and annotations remain local. VibeCon sends no telemetry. Its only automated actions are explicitly enabled window switching and focusing Codex; it does not execute shell commands or approve AI-agent actions.

## License

[MIT](./LICENSE) © 2026 CoderSerio.
