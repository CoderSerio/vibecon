# VibeCon

**Use a controller as an inspectable control surface for AI-assisted coding.**

VibeCon begins with a small but deliberate first step: a native desktop lab for
observing a paired Nintendo Joy-Con before mapping it to any action. It is being
built and validated on macOS, while its Tauri and Rust architecture keeps a
native Windows build in scope.

> Status: early prototype. It is **read-only**. It does not move the pointer,
> inject keystrokes, switch windows, execute commands, or approve agent actions.

[中文文档](./README_CN.md)

## What works today

Run the Tauri desktop app to:

- find connected Nintendo HID devices, including `Joy-Con (L)` and `Joy-Con (R)`;
- select a controller and stream its latest raw HID reports;
- show decoded stick values and button bytes for native Joy-Con `0x30` reports
  and macOS generic-HID `0x3f` reports (macOS currently quantizes Joy-Con (L)
  stick movement into an eight-way HAT);
- render an original CSS Joy-Con visualizer whose primary stick follows the
  live axis and whose confirmed D-pad HAT states glow;
- retain the latest 80 reports for copying and diagnosis.

The raw report remains visible even when a controller uses an unexpected report
mode. That is intentional: we should map controls from observed input, rather
than guessing a device layout.

## Quick start — macOS

### 1. Pair a Joy-Con

1. Detach and charge the Joy-Con.
2. Hold its small **Sync** button until the player LEDs sweep.
3. Open **System Settings → Bluetooth** and connect `Joy-Con (L)` or
   `Joy-Con (R)`.
4. Pair each side separately if you want to inspect both.

If it does not appear, repeat the Sync step and temporarily keep the Switch
away or powered down so it does not reconnect there first.

### 2. Start the native desktop shell

Prerequisites: current Node.js + pnpm and a Rust toolchain.

```sh
cd /Users/carbon/Desktop/vibecon
pnpm install
pnpm tauri dev
```

Click **Refresh controllers**, select a Joy-Con, then move a stick or press a
button. The Raw input reports panel should begin to update.

### Important: do not use `pnpm dev` for hardware testing

`pnpm dev` launches only Vite in a web browser. Browser preview is useful for
styling, but it has no Tauri Rust backend and therefore no `invoke()` or local
HID access. Use **`pnpm tauri dev`** to launch the native macOS window.

## Development

```sh
pnpm build                    # TypeScript + Vite production build
cd src-tauri && cargo check   # Rust/Tauri/HID backend
```

Project layout:

```text
src/                    TypeScript input dashboard
src-tauri/src/lib.rs    Rust commands and Joy-Con HID decoding
src-tauri/              Tauri configuration, capabilities, desktop resources
README_CN.md            Chinese documentation
```

## Why Tauri + Rust

- **macOS first:** develop against the Joy-Con and its actual Bluetooth/HID
  behaviour on the author's machine.
- **Windows possible:** the desktop UI and controller core are not tied to
  Swift/macOS APIs. Windows must still be built and tested on Windows or CI.
- **Trust before automation:** first expose physical input clearly; only then
  add deliberate, reviewable mappings such as stick-left/right → window switch.

## Planned, not implemented

1. profile discovery for Joy-Con, DualSense, Xbox, and 8BitDo controllers;
2. controller calibration and a visual button/axis map;
3. opt-in platform adapters for actions such as next/previous window;
4. explicit, physical confirmation for any future agent approval action.

## Privacy and safety

VibeCon currently reads local controller input only. It sends no telemetry and
does not automate keyboard, mouse, shell, or agent approval actions.

## License

License selection is intentionally deferred until the first usable controller
profile exists.
