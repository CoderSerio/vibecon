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

## First controlled implementation

VibeCon now has an explicit **Test selected Joy-Con vibration** button on the
Mappings page. It is not automatic and does not run from a controller binding.
For each Joy-Con selected in Debug, it sends:

1. `0x01` with subcommand `0x48 0x01` to enable rumble;
2. one 70 ms low-intensity `0x10` rumble pulse;
3. an unconditional neutral `0x10` frame afterwards, even when the pulse write
   returns an error.

The HID reader and writer share one handle with a short mutex hold per I/O call,
so the test does not hold the reader lock while waiting for the pulse duration.

## What is deliberately not claimed

VibeCon has observed macOS compact `0x3F` input reports, but has **not** shown
that the same HID endpoint accepts Joy-Con Bluetooth output report `0x10`.
Some macOS HID paths expose an input-only interface or alter the report routing.
The test is opt-in precisely because macOS output compatibility must be checked
on real hardware. A write failure is surfaced in the UI and is never retried.

## Next controlled experiment

1. Test independently for Joy-Con (L), Joy-Con (R), single and paired states.
2. Record the exact macOS output error, if any, before changing the pulse.
3. Task-completion haptics come last: they require both a verified output path
   and a trustworthy explicit completion event source from Codex/CLI.
