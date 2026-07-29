# Joy-Con output / rumble research

This note records the boundary before VibeCon writes to a paired controller.
Input reports are already useful and safe; output reports can change controller
state, so they need a separate real-device test rather than being inferred from
the input decoder.

## What is established

- VibeCon's current `hidapi` dependency (`2.6.6`) exposes synchronous
  `HidDevice::write(&[u8])`.
- Nintendo Switch reverse-engineering captures use Bluetooth output report
  `0x10` for rumble data.
- The captured neutral/stop payload is eight rumble bytes:

  ```text
  00 01 40 40 00 01 40 40
  ```

  In a report this appears as `10 <packet-counter> 00 01 40 40 00 01 40 40`.
- Rumble frequency and amplitude are encoded fields, not a single boolean.
  The public encoding notes and capture are in
  [Nintendo_Switch_Reverse_Engineering](https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering).

## What is deliberately not claimed

VibeCon has observed macOS compact `0x3F` input reports, but has **not** shown
that the same HID endpoint accepts Joy-Con Bluetooth output report `0x10`.
Some macOS HID paths expose an input-only interface or alter the report routing.
Therefore no rumble packet is emitted by the app yet.

## Next controlled experiment

1. Add a hidden/manual native command that opens a selected Joy-Con by its
   exact HID path and sends only one known, low-amplitude `0x10` pulse followed
   immediately by the neutral packet.
2. Log the `write()` byte count and OS error without retrying automatically.
3. Test independently for Joy-Con (L), Joy-Con (R), single and paired states.
4. Only after that succeeds, expose a visible **Test vibration** control.
5. Task-completion haptics come last: they require both a verified output path
   and a trustworthy explicit completion event source from Codex/CLI.
