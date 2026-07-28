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
- keep one HID device handle open and forward input reports to the desktop UI;
- keep the visualizer at full input rate while choosing a log policy: key
  operations (the default), the original 75ms snapshots, 60/30/10 Hz samples,
  or every raw report;
- click a report to label it as a fixed stick position or Joy-Con control;
- append labels to `~/.vibecon/annotations.jsonl`, then show matching labels
  next to later reports.
- retain the latest 80 reports for copying and diagnosis.

The raw report remains visible even when a controller uses an unexpected report
mode. That is intentional: we should map controls from observed input, rather
than guessing a device layout.

### Observed macOS Joy-Con (L) HAT profile

On the current macOS Bluetooth HID path, `0x3f` report byte 3 is a complete
eight-way stick HAT: values `0` through `7` are the eight outer directions and
`8` is neutral. VibeCon includes this portrait-orientation mapping as a built-in
profile, so these reports do not need manual per-direction labels.

![VibeCon showing the observed 0-8 HAT values in raw Joy-Con reports](./docs/images/macos-joycon-hat-debug.png)

The same report has two independent button bitfields. Decode each byte with
bitwise AND/OR; do not treat the complete report as one additive number.

| Byte | Confirmed flags |
| --- | --- |
| 1 | `01` D-pad left, `02` down, `04` up, `08` right, `10` SL, `20` SR |
| 2 | `01` Minus, `04` stick press, `20` Capture, `40` L, `80` ZL |
| 3 | HAT `0–7` outer stick directions; `8` neutral |

For example, `3F 08 40 08 ...` means **D-pad right + L**. The values share no
bits, so bitwise OR happens to have the same numeric result as addition; OR is
the correct operation and remains correct for every combination.

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

Annotations are local samples, not executable mappings. **Clear** only clears
the current visible log; it never deletes `~/.vibecon/annotations.jsonl`.

## License

License selection is intentionally deferred until the first usable controller
profile exists.
