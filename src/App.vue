<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import JoyCon from "./components/JoyCon.vue";
import type { AppLocale } from "./i18n";

type Controller = {
  id: string;
  name: string;
  product_id: number;
  transport: string;
};
type Stick = {
  x: number;
  y: number;
  normalized_x: number;
  normalized_y: number;
};
type InputReport = {
  report_id: number;
  bytes: number[];
  left_stick: Stick | null;
  right_stick: Stick | null;
  buttons: [number, number, number] | null;
};
type LogEntry = {
  device_id: string;
  timestamp: Date;
  previous_report?: InputReport;
  report: InputReport;
};
type Label = {
  kind: "stick" | "button";
  target: string;
  phase?: "pressed" | "released" | "moved" | "reset";
};
type Annotation = {
  version: number;
  created_at_ms: number;
  controller: { vendor_id: number; product_id: number; orientation: string };
  previous_report?: { report_id: number; bytes: number[] };
  report: { report_id: number; bytes: number[] };
  label: Label;
};
type StreamEvent = { device_id: string; report: InputReport };
type MappingBinding = {
  id: string;
  control: string;
  action: "window_previous" | "window_next" | "focus_codex";
  enabled: boolean;
};
type MappingPreset = {
  id: string;
  name: string;
  enabled: boolean;
  bindings: MappingBinding[];
};
type MappingConfig = {
  version: number;
  activePresetId: string;
  presets: MappingPreset[];
};

function defaultMappingConfig(): MappingConfig {
  return {
    version: 1,
    activePresetId: "codex-cowork",
    presets: [
      {
        id: "code",
        name: "Code",
        enabled: true,
        bindings: [
          { id: "window-previous", control: "joycon_left.stick_left", action: "window_previous", enabled: true },
          { id: "window-next", control: "joycon_left.stick_right", action: "window_next", enabled: true },
        ],
      },
      {
        id: "codex-cowork",
        name: "Codex Cowork",
        enabled: true,
        bindings: [
          { id: "window-previous", control: "joycon_left.stick_left", action: "window_previous", enabled: true },
          { id: "window-next", control: "joycon_left.stick_right", action: "window_next", enabled: true },
          { id: "focus-codex-left", control: "joycon_left.dpad_up", action: "focus_codex", enabled: true },
          { id: "focus-codex-right", control: "joycon_right.x", action: "focus_codex", enabled: true },
        ],
      },
      { id: "inspect-only", name: "Inspect Only", enabled: false, bindings: [] },
    ],
  };
}

const isTauriDesktop = "__TAURI_INTERNALS__" in window;
const { t, locale } = useI18n();
const activePage = ref<"debug" | "mappings">("debug");
const mappingConfig = ref<MappingConfig>(defaultMappingConfig());
const controllers = ref<Controller[]>([]);
const selectedControllers = ref<Controller[]>([]);
const status = ref("Looking for Nintendo HID devices…");
const statusKind = ref<"" | "connected" | "error">("");
const logs = ref<LogEntry[]>([]);
const annotations = ref<Annotation[]>([]);
const sampleRate = ref("key");
const leftStick = ref<Stick | null>(null);
const rightStick = ref<Stick | null>(null);
const activeControlsBySide = ref({
  left: [] as string[],
  right: [] as string[],
});
const activeControls = computed(() => [
  ...activeControlsBySide.value.left,
  ...activeControlsBySide.value.right,
]);
const mappingInputStatus = computed(() => {
  const leftController = selectedControllers.value.find(
    ({ product_id }) => product_id === 0x2006,
  );
  if (!leftController)
    return t("mapping.noLeft");
  return t("mapping.selected", { name: leftController.name });
});
const activePreset = computed(() =>
  mappingConfig.value.presets.find(
    ({ id }) => id === mappingConfig.value.activePresetId,
  ),
);
const recentControls = ref<string[]>([]);
const buttonsReadout = ref("D-pad 00 · waiting for input");
const dialog = ref<HTMLDialogElement>();
const selectedLog = ref<LogEntry>();
const annotationKind = ref<"stick" | "button">("stick");
const annotationTarget = ref<string>();
const mappingFeedback = ref(t("mapping.initial"));
const accessibilityGranted = ref(false);
const buttonPhase = ref<"pressed" | "released">("pressed");
const stickPhase = ref<"moved" | "reset">("moved");
const previewTarget = ref<string>();
let unlistenInput: UnlistenFn | undefined;
let unlistenError: UnlistenFn | undefined;
const lastPresentedAt = new Map<string, number>();
const lastKeyOperation = new Map<string, string>();
const lastIncomingReport = new Map<string, InputReport>();
let recentControlsTimer: ReturnType<typeof setTimeout> | undefined;

watch(activePage, (page) => {
  if (page === "mappings") void checkMappingAccessibility();
  void syncMappingRuntime();
});
watch(
  mappingConfig,
  () => {
    if (!isTauriDesktop || !mappingConfig.value.presets.length) return;
    void persistMappingConfig();
  },
  { deep: true },
);

const stickTargets = [
  "center",
  ...["n", "ne", "e", "se", "s", "sw", "w", "nw"].flatMap((direction) => [
    `inner-${direction}`,
    `outer-${direction}`,
  ]),
];
const buttonTargets = [
  "joycon_left.stick_press",
  "joycon_left.dpad_up",
  "joycon_left.dpad_right",
  "joycon_left.dpad_down",
  "joycon_left.dpad_left",
  "joycon_left.minus",
  "joycon_left.capture",
  "joycon_left.sl",
  "joycon_left.sr",
  "joycon_left.l",
  "joycon_left.zl",
];
const buttonNames: Record<string, string> = {
  "joycon_left.stick_press": "Stick press",
  "joycon_left.dpad_up": "D-pad up",
  "joycon_left.dpad_right": "D-pad right",
  "joycon_left.dpad_down": "D-pad down",
  "joycon_left.dpad_left": "D-pad left",
  "joycon_left.minus": "Minus",
  "joycon_left.capture": "Capture",
  "joycon_left.sl": "SL",
  "joycon_left.sr": "SR",
  "joycon_left.l": "L",
  "joycon_left.zl": "ZL",
};

const nubTransform = computed(() => {
  return stickTransform(leftStick.value);
});
const rightNubTransform = computed(() =>
  stickTransform(rightStick.value, true),
);

function stickTransform(stick: Stick | null, invertY = false) {
  if (!stick) return "translate(0, 0)";
  const x = Math.max(-1, Math.min(1, stick.normalized_x)) * 16;
  const y =
    Math.max(-1, Math.min(1, stick.normalized_y)) * (invertY ? -16 : 16);
  return `translate(${x.toFixed(1)}px, ${y.toFixed(1)}px)`;
}
const selectedReportText = computed(() =>
  selectedLog.value
    ? `report 0x${hex(selectedLog.value.report.report_id)} · ${selectedLog.value.report.bytes.map(hex).join(" ")}`
    : "",
);
const annotationChoice = computed(() =>
  !annotationTarget.value
    ? t("annotation.chooseTarget")
    : t("annotation.selected", {
        target:
          annotationKind.value === "stick"
            ? annotationTarget.value
            : buttonNames[annotationTarget.value],
        phase:
          annotationKind.value === "stick"
            ? stickPhase.value
            : buttonPhase.value,
      }),
);
const groupedLogs = computed(() => {
  const groups: Array<{ timestamp: Date; entries: LogEntry[] }> = [];
  for (const entry of logs.value) {
    const group = groups.at(-1);
    if (
      group &&
      Math.abs(group.timestamp.getTime() - entry.timestamp.getTime()) <= 24 &&
      !group.entries.some(({ device_id }) => device_id === entry.device_id)
    ) {
      group.entries.push(entry);
    } else {
      groups.push({ timestamp: entry.timestamp, entries: [entry] });
    }
  }
  for (const group of groups) {
    group.entries.sort((a, b) => {
      const product = (id: string) =>
        controllers.value.find((controller) => controller.id === id)
          ?.product_id;
      return (
        (product(a.device_id) === 0x2006 ? 0 : 1) -
        (product(b.device_id) === 0x2006 ? 0 : 1)
      );
    });
  }
  return groups;
});

function hex(byte: number) {
  return byte.toString(16).padStart(2, "0").toUpperCase();
}
function fingerprint(report: { report_id: number; bytes: number[] }) {
  return `${report.report_id}:${report.bytes.map(hex).join("")}`;
}
function formatReport(report: InputReport) {
  const chunks = report.bytes
    .map(hex)
    .reduce<string[]>((lines, byte, index) => {
      const line = Math.floor(index / 26);
      lines[line] = `${lines[line] ? `${lines[line]} ` : ""}${byte}`;
      return lines;
    }, []);
  return `report 0x${hex(report.report_id)} ${chunks.join("\n              ")}`;
}
function renderStick(stick: Stick | null) {
  return stick
    ? `x ${stick.normalized_x.toFixed(3)}\ny ${stick.normalized_y.toFixed(3)}\nraw ${stick.x}, ${stick.y}`
    : "No decoded value";
}
function labelText(label: Label, legacy = false) {
  const text =
    label.kind === "stick"
      ? `Stick · ${label.target} · ${label.phase ?? "moved"}`
      : `Button · ${label.target.replace(/^joycon_(left|right)\./, "$1 · ")} · ${label.phase ?? "pressed"}`;
  return legacy ? `${text} · legacy raw` : text;
}

function macOSJoyConHatLabel(entry: LogEntry): Label | undefined {
  const { report } = entry;
  if (report.report_id !== 0x3f || report.bytes.length < 4) return undefined;
  const targets = [
    "outer-e",
    "outer-se",
    "outer-s",
    "outer-sw",
    "outer-w",
    "outer-nw",
    "outer-n",
    "outer-ne",
    "center",
  ];
  const hat = report.bytes[3];
  if (hat > 8) return undefined;
  if (hat < 8) return { kind: "stick", target: targets[hat], phase: "moved" };
  if (
    entry.previous_report?.report_id === 0x3f &&
    entry.previous_report.bytes.length >= 4 &&
    entry.previous_report.bytes[3] < 8
  )
    return { kind: "stick", target: "center", phase: "reset" };
  return undefined;
}
const macOSButtonBits: Array<{ byte: 1 | 2; mask: number; target: string }> = [
  { byte: 1, mask: 0x01, target: "joycon_left.dpad_left" },
  { byte: 1, mask: 0x02, target: "joycon_left.dpad_down" },
  { byte: 1, mask: 0x04, target: "joycon_left.dpad_up" },
  { byte: 1, mask: 0x08, target: "joycon_left.dpad_right" },
  { byte: 1, mask: 0x10, target: "joycon_left.sl" },
  { byte: 1, mask: 0x20, target: "joycon_left.sr" },
  { byte: 2, mask: 0x01, target: "joycon_left.minus" },
  { byte: 2, mask: 0x04, target: "joycon_left.stick_press" },
  { byte: 2, mask: 0x20, target: "joycon_left.capture" },
  { byte: 2, mask: 0x40, target: "joycon_left.l" },
  { byte: 2, mask: 0x80, target: "joycon_left.zl" },
];
function macOSJoyConButtonLabels(entry: LogEntry): Label[] {
  const isRight =
    controllers.value.find(({ id }) => id === entry.device_id)?.product_id ===
    0x2007;
  if (entry.report.report_id === 0x3f && isRight) {
    const bits = [
      { byte: 1 as const, mask: 0x01, target: "joycon_right.y" },
      { byte: 1 as const, mask: 0x02, target: "joycon_right.x" },
      { byte: 1 as const, mask: 0x04, target: "joycon_right.b" },
      { byte: 1 as const, mask: 0x08, target: "joycon_right.a" },
      { byte: 1 as const, mask: 0x10, target: "joycon_right.sr" },
      { byte: 1 as const, mask: 0x20, target: "joycon_right.sl" },
      { byte: 1 as const, mask: 0x40, target: "joycon_right.r" },
      { byte: 1 as const, mask: 0x80, target: "joycon_right.zr" },
    ];
    return bits.flatMap(({ byte, mask, target }) => {
      const current = entry.report.bytes[byte];
      const prior =
        entry.previous_report?.report_id === 0x3f
          ? entry.previous_report.bytes[byte]
          : 0;
      return current & mask
        ? [{ kind: "button" as const, target, phase: "pressed" as const }]
        : prior & mask
          ? [{ kind: "button" as const, target, phase: "released" as const }]
          : [];
    });
  }
  if (entry.report.report_id === 0x30 && entry.report.bytes.length >= 6) {
    const nativeBits: Array<{ byte: 3 | 4 | 5; mask: number; target: string }> =
      [
        { byte: 3, mask: 0x01, target: "joycon_right.y" },
        { byte: 3, mask: 0x02, target: "joycon_right.x" },
        { byte: 3, mask: 0x04, target: "joycon_right.b" },
        { byte: 3, mask: 0x08, target: "joycon_right.a" },
        { byte: 3, mask: 0x10, target: "joycon_right.sr" },
        { byte: 3, mask: 0x20, target: "joycon_right.sl" },
        { byte: 3, mask: 0x40, target: "joycon_right.r" },
        { byte: 3, mask: 0x80, target: "joycon_right.zr" },
        { byte: 4, mask: 0x01, target: "joycon_left.minus" },
        { byte: 4, mask: 0x02, target: "joycon_right.plus" },
        { byte: 4, mask: 0x04, target: "joycon_left.stick_press" },
        { byte: 4, mask: 0x08, target: "joycon_right.stick_press" },
        { byte: 4, mask: 0x10, target: "joycon_right.home" },
        { byte: 4, mask: 0x20, target: "joycon_left.capture" },
        { byte: 5, mask: 0x01, target: "joycon_left.dpad_down" },
        { byte: 5, mask: 0x02, target: "joycon_left.dpad_up" },
        { byte: 5, mask: 0x04, target: "joycon_left.dpad_right" },
        { byte: 5, mask: 0x08, target: "joycon_left.dpad_left" },
        { byte: 5, mask: 0x10, target: "joycon_left.sr" },
        { byte: 5, mask: 0x20, target: "joycon_left.sl" },
        { byte: 5, mask: 0x40, target: "joycon_left.l" },
        { byte: 5, mask: 0x80, target: "joycon_left.zl" },
      ];
    return nativeBits.flatMap(({ byte, mask, target }) => {
      const current = entry.report.bytes[byte];
      const prior =
        entry.previous_report?.report_id === 0x30
          ? entry.previous_report.bytes[byte]
          : 0;
      return current & mask
        ? [{ kind: "button" as const, target, phase: "pressed" as const }]
        : prior & mask
          ? [{ kind: "button" as const, target, phase: "released" as const }]
          : [];
    });
  }
  if (entry.report.report_id !== 0x3f || entry.report.bytes.length < 3)
    return [];
  return macOSButtonBits.flatMap(({ byte, mask, target }) => {
    const current = entry.report.bytes[byte];
    const prior =
      entry.previous_report?.report_id === 0x3f &&
      entry.previous_report.bytes.length > byte
        ? entry.previous_report.bytes[byte]
        : 0;
    return current & mask
      ? [{ kind: "button" as const, target, phase: "pressed" as const }]
      : prior & mask
        ? [{ kind: "button" as const, target, phase: "released" as const }]
        : [];
  });
}
function builtInLabels(entry: LogEntry) {
  return [
    ...macOSJoyConButtonLabels(entry),
    ...[macOSJoyConHatLabel(entry)].filter((label): label is Label =>
      Boolean(label),
    ),
  ];
}
function savedAnnotation(entry: LogEntry) {
  const key = fingerprint(entry.report);
  return annotations.value
    .filter((annotation) =>
      annotation.previous_report
        ? Boolean(entry.previous_report) &&
          fingerprint(annotation.previous_report) ===
            fingerprint(entry.previous_report!) &&
          fingerprint(annotation.report) === key
        : fingerprint(annotation.report) === key,
    )
    .at(-1);
}

function stickBucket(stick: Stick | null) {
  if (!stick) return "unknown";
  const { normalized_x: x, normalized_y: y } = stick;
  const radius = Math.hypot(x, y);
  if (radius < 0.2) return "center";
  const ring = radius < 0.7 ? "inner" : "outer";
  const sectors = ["e", "se", "s", "sw", "w", "nw", "n", "ne"];
  return `${ring}-${sectors[(Math.round(Math.atan2(y, x) / (Math.PI / 4)) + 8) % 8]}`;
}
function shouldLog(report: InputReport, deviceId: string) {
  if (sampleRate.value === "all") return true;
  if (sampleRate.value === "key") {
    const current = `${report.buttons?.join(":") ?? "none"}|${stickBucket(report.left_stick)}|${stickBucket(report.right_stick)}`;
    if (current === lastKeyOperation.get(deviceId)) return false;
    lastKeyOperation.set(deviceId, current);
    return true;
  }
  const interval =
    sampleRate.value === "75" ? 75 : 1000 / Number(sampleRate.value);
  if (performance.now() - (lastPresentedAt.get(deviceId) ?? 0) < interval)
    return false;
  lastPresentedAt.set(deviceId, performance.now());
  return true;
}
function applyReport(report: InputReport, controller: Controller) {
  const side = controller.product_id === 0x2007 ? "right" : "left";
  // Mapping only needs the stick direction. Keep the full reactive visualizer,
  // button decoder, and debug readouts dormant outside the Debug page.
  if (activePage.value !== "debug") return;

  if (side === "left") leftStick.value = report.left_stick;
  else rightStick.value = report.right_stick;
  if (!report.buttons) {
    activeControlsBySide.value[side] = [];
    buttonsReadout.value = "No decoded button data";
    return;
  }
  const [buttonMask, extraButtons, hat] = report.buttons;
  if (report.report_id === 0x30) {
    // Native Joy-Con reports are a combined layout, not the compact macOS
    // 0x3f layout. Byte 3 belongs to R, byte 5 belongs to L, and byte 4
    // contains their shared system buttons.
    const mappings: Array<[number, Record<string, number>]> = [
      [
        buttonMask,
        {
          "joycon_right.y": 0x01,
          "joycon_right.x": 0x02,
          "joycon_right.b": 0x04,
          "joycon_right.a": 0x08,
          "joycon_right.sr": 0x10,
          "joycon_right.sl": 0x20,
          "joycon_right.r": 0x40,
          "joycon_right.zr": 0x80,
        },
      ],
      [
        extraButtons,
        {
          "joycon_left.minus": 0x01,
          "joycon_right.plus": 0x02,
          "joycon_left.stick_press": 0x04,
          "joycon_right.stick_press": 0x08,
          "joycon_right.home": 0x10,
          "joycon_left.capture": 0x20,
        },
      ],
      [
        hat,
        {
          "joycon_left.dpad_down": 0x01,
          "joycon_left.dpad_up": 0x02,
          "joycon_left.dpad_right": 0x04,
          "joycon_left.dpad_left": 0x08,
          "joycon_left.sr": 0x10,
          "joycon_left.sl": 0x20,
          "joycon_left.l": 0x40,
          "joycon_left.zl": 0x80,
        },
      ],
    ];
    setActiveControls(
      mappings
        .flatMap(([bits, controls]) =>
          Object.entries(controls)
            .filter(([, mask]) => bits & mask)
            .map(([target]) => target),
        )
        .filter((target) => target.startsWith(`joycon_${side}.`)),
      side,
    );
    buttonsReadout.value = `Native 0x30 · R 0x${hex(buttonMask)} · shared 0x${hex(extraButtons)} · L 0x${hex(hat)}`;
    return;
  }
  if (side === "right") {
    const mappings: Array<[number, Record<string, number>]> = [
      [
        buttonMask,
        {
          "joycon_right.y": 0x01,
          "joycon_right.x": 0x02,
          "joycon_right.b": 0x04,
          "joycon_right.a": 0x08,
          "joycon_right.sr": 0x10,
          "joycon_right.sl": 0x20,
          "joycon_right.r": 0x40,
          "joycon_right.zr": 0x80,
        },
      ],
      [
        extraButtons,
        {
          "joycon_right.plus": 0x01,
          "joycon_right.stick_press": 0x04,
          "joycon_right.home": 0x20,
        },
      ],
    ];
    setActiveControls(
      mappings.flatMap(([bits, controls]) =>
        Object.entries(controls)
          .filter(([, mask]) => bits & mask)
          .map(([target]) => target),
      ),
      side,
    );
    buttonsReadout.value = `R compact 0x3F · buttons 0x${hex(buttonMask)} · extra 0x${hex(extraButtons)}`;
    return;
  }
  const mappings: Array<[number, Record<string, number>]> = [
    [
      buttonMask,
      {
        "joycon_left.dpad_left": 1,
        "joycon_left.dpad_down": 2,
        "joycon_left.dpad_up": 4,
        "joycon_left.dpad_right": 8,
        "joycon_left.sl": 16,
        "joycon_left.sr": 32,
      },
    ],
    [
      extraButtons,
      {
        "joycon_left.minus": 1,
        "joycon_left.stick_press": 4,
        "joycon_left.capture": 32,
        "joycon_left.l": 64,
        "joycon_left.zl": 128,
      },
    ],
  ];
  setActiveControls(
    mappings.flatMap(([bits, controls]) =>
      Object.entries(controls)
        .filter(([, mask]) => bits & mask)
        .map(([target]) => target),
    ),
    side,
  );
  buttonsReadout.value = `D-pad 0x${hex(buttonMask)} · stick HAT ${hat === 8 ? "neutral" : hat} · extra 0x${hex(extraButtons)}`;
}
async function checkMappingAccessibility() {
  if (!isTauriDesktop) return;
  const result = await invoke<string>("mapping_accessibility_status").catch(
    (error) => `Could not check Accessibility: ${String(error)}`,
  );
  accessibilityGranted.value = result.startsWith("Accessibility: granted");
  mappingFeedback.value = result;
}
function openAccessibilitySettings() {
  void invoke("open_accessibility_settings")
    .then(() => {
      mappingFeedback.value = t("mapping.opened");
    })
    .catch((error) => {
      mappingFeedback.value = `Could not open Accessibility settings: ${String(error)}`;
    });
}
function testWindowSwitch() {
  mappingFeedback.value = t("mapping.testSending");
  void invoke("switch_window", { direction: "next" })
    .then(() => {
      mappingFeedback.value = t("mapping.testSent");
    })
    .catch((error) => {
      mappingFeedback.value = `Test failed: ${String(error)}`;
      showError(error);
    });
}
async function syncMappingRuntime() {
  if (!isTauriDesktop || !mappingConfig.value.presets.length) return;
  try {
    await invoke("set_mapping_runtime", {
      config: mappingConfig.value,
      active: activePage.value === "mappings",
    });
  } catch (error) {
    showError(error);
  }
}
async function persistMappingConfig() {
  try {
    await invoke("save_mapping_config", { config: mappingConfig.value });
    await syncMappingRuntime();
  } catch (error) {
    showError(error);
  }
}
function selectPreset(id: string) {
  mappingConfig.value.activePresetId = id;
}
function resetMappingConfig() {
  void invoke<MappingConfig>("reset_mapping_config")
    .then((config) => {
      mappingConfig.value = config;
      mappingFeedback.value = t("mapping.resetDone");
    })
    .catch(showError);
}
function controlName(control: string) {
  const names: Record<string, string> = {
    "joycon_left.stick_left": "L stick ←",
    "joycon_left.stick_right": "L stick →",
    "joycon_left.dpad_up": "L D-pad ↑",
    "joycon_left.dpad_down": "L D-pad ↓",
    "joycon_left.dpad_left": "L D-pad ←",
    "joycon_left.dpad_right": "L D-pad →",
    "joycon_right.x": "R X",
    "joycon_right.y": "R Y",
    "joycon_right.a": "R A",
    "joycon_right.b": "R B",
  };
  return names[control] ?? control.replace("joycon_", "").replace(".", " · ");
}
function actionName(action: MappingBinding["action"]) {
  return t(
    `mapping.${action === "window_previous" ? "previousWindow" : action === "window_next" ? "nextWindow" : "focusCodex"}`,
  );
}
function controlPreviewTarget(control: string) {
  return control.replace(/\.stick_(left|right)$/, ".stick_press");
}
async function copyAgentPrompt() {
  const prompt = `You are editing VibeCon's mapping configuration. Read ~/.vibecon/mappings.json and keep version 1. You may only use the existing controls and safe actions: window_previous, window_next, focus_codex. Preserve valid JSON, unique preset and binding ids, then explain the change. Do not add shell commands or arbitrary automation.`;
  try {
    await navigator.clipboard.writeText(prompt);
    mappingFeedback.value = t("mapping.copied");
  } catch (error) {
    mappingFeedback.value = `Could not copy the agent prompt: ${String(error)}`;
  }
}
function setLocale(next: string) {
  if (next === "en" || next === "zh-CN") locale.value = next as AppLocale;
}
function setActiveControls(next: string[], side: "left" | "right") {
  const newlyPressed = next.filter(
    (target) => !activeControlsBySide.value[side].includes(target),
  );
  activeControlsBySide.value[side] = next;
  if (!newlyPressed.length) return;
  recentControls.value = [
    ...new Set([...recentControls.value, ...newlyPressed]),
  ];
  if (recentControlsTimer) clearTimeout(recentControlsTimer);
  recentControlsTimer = setTimeout(() => {
    recentControls.value = [];
    recentControlsTimer = undefined;
  }, 220);
}
function showError(error: unknown) {
  status.value = String(error);
  statusKind.value = "error";
}
function clearLog() {
  logs.value = [];
}
function appendLog(
  device_id: string,
  report: InputReport,
  previous_report?: InputReport,
) {
  logs.value = [
    { timestamp: new Date(), device_id, previous_report, report },
    ...logs.value,
  ].slice(0, 160);
}
function selectController(controller: Controller) {
  const selected = selectedControllers.value.some(
    ({ id }) => id === controller.id,
  );
  if (selected) {
    selectedControllers.value = selectedControllers.value.filter(
      ({ id }) => id !== controller.id,
    );
    void invoke("stop_joycon_stream", { id: controller.id });
  } else {
    selectedControllers.value = [...selectedControllers.value, controller];
    void invoke("start_joycon_stream", { id: controller.id }).catch(showError);
  }
  lastKeyOperation.delete(controller.id);
  lastIncomingReport.delete(controller.id);
  status.value = selectedControllers.value.length
    ? `Streaming ${selectedControllers.value.map(({ name }) => name).join(" + ")}. Move a stick or press a button.`
    : "Choose one or two controllers to start streaming.";
  statusKind.value = "connected";
}
async function refreshControllers() {
  if (!isTauriDesktop) {
    showError("Browser preview detected. Run `pnpm tauri dev` for HID access.");
    return;
  }
  status.value = "Checking HID devices…";
  try {
    controllers.value = await invoke<Controller[]>("list_joycons");
    if (!selectedControllers.value.length && controllers.value.length)
      selectController(controllers.value[0]);
  } catch (error) {
    showError(error);
  }
}
function openAnnotation(entry: LogEntry) {
  selectedLog.value = entry;
  annotationKind.value = "stick";
  annotationTarget.value = undefined;
  buttonPhase.value = "pressed";
  stickPhase.value = "moved";
  previewTarget.value = undefined;
  dialog.value?.showModal();
}
function setKind(kind: "stick" | "button") {
  annotationKind.value = kind;
  annotationTarget.value = undefined;
  previewTarget.value = undefined;
}
function chooseTarget(target: string) {
  annotationTarget.value = target;
}
async function saveAnnotation() {
  if (!selectedLog.value || !selectedLog.value || !annotationTarget.value)
    return;
  try {
    const annotation = await invoke<Annotation>("save_annotation", {
      draft: {
        controller: {
          vendor_id: 0x057e,
          product_id:
            controllers.value.find(
              ({ id }) => id === selectedLog.value!.device_id,
            )?.product_id ?? 0,
          orientation: "portrait",
        },
        previous_report: selectedLog.value.previous_report
          ? {
              report_id: selectedLog.value.previous_report.report_id,
              bytes: selectedLog.value.previous_report.bytes,
            }
          : undefined,
        report: {
          report_id: selectedLog.value.report.report_id,
          bytes: selectedLog.value.report.bytes,
        },
        label: {
          kind: annotationKind.value,
          target: annotationTarget.value,
          phase:
            annotationKind.value === "button"
              ? buttonPhase.value
              : stickPhase.value,
        },
      },
    });
    annotations.value.push(annotation);
    dialog.value?.close();
  } catch (error) {
    showError(error);
  }
}

onMounted(async () => {
  if (!isTauriDesktop) {
    mappingConfig.value = defaultMappingConfig();
    await refreshControllers();
    return;
  }
  annotations.value = await invoke<Annotation[]>("load_annotations").catch(
    (error) => {
      showError(error);
      return [];
    },
  );
  const loadedMappingConfig = await invoke<MappingConfig>("load_mapping_config").catch(
    (error) => {
      showError(error);
      return mappingConfig.value;
    },
  );
  mappingConfig.value = loadedMappingConfig;
  await syncMappingRuntime();
  unlistenInput = await listen<StreamEvent>("joycon-input", ({ payload }) => {
    const controller = selectedControllers.value.find(
      ({ id }) => id === payload.device_id,
    );
    if (!controller) return;
    applyReport(payload.report, controller);
    if (activePage.value !== "debug") return;
    const previous = lastIncomingReport.get(payload.device_id);
    lastIncomingReport.set(payload.device_id, payload.report);
    if (shouldLog(payload.report, payload.device_id))
      appendLog(payload.device_id, payload.report, previous);
  });
  unlistenError = await listen<string>("joycon-stream-error", ({ payload }) =>
    showError(payload),
  );
  await refreshControllers();
});
onBeforeUnmount(() => {
  unlistenInput?.();
  unlistenError?.();
  if (recentControlsTimer) clearTimeout(recentControlsTimer);
  if (isTauriDesktop) void invoke("stop_joycon_stream");
});
</script>

<template>
  <main class="app-shell">
    <header class="app-header">
      <div>
        <p class="eyebrow">{{ t("app.eyebrow") }}</p>
        <h1 class="app-title">VibeCon</h1>
        <p class="subtitle">{{ t("app.subtitle") }}</p>
      </div>
      <div class="header-actions">
        <label class="language-picker"><span class="sr-only">{{ t("app.language") }}</span><select :value="locale" @change="setLocale(($event.target as HTMLSelectElement).value)"><option value="en">EN</option><option value="zh-CN">中文</option></select></label>
        <button class="app-button" @click="refreshControllers">{{ t("app.refresh") }}</button>
      </div>
    </header>
    <nav class="app-tabs" aria-label="Application pages">
      <button
        class="app-button tab"
        :class="{ selected: activePage === 'debug' }"
        @click="activePage = 'debug'"
      >
        {{ t("app.debug") }}</button
      ><button
        class="app-button tab"
        :class="{ selected: activePage === 'mappings' }"
        @click="activePage = 'mappings'"
      >
        {{ t("app.mappings") }}
      </button>
    </nav>
    <section v-if="activePage === 'mappings'" class="panel mapping-panel">
      <div class="section-heading">
        <div>
          <p class="eyebrow">{{ t("mapping.eyebrow") }}</p>
          <h2 class="section-title">{{ t("mapping.title") }}</h2>
          <p class="hint">{{ t("mapping.subtitle") }}</p>
        </div>
        <button class="secondary" @click="copyAgentPrompt">{{ t("mapping.copyPrompt") }}</button>
      </div>
      <div class="preset-picker" role="tablist" aria-label="Mapping presets">
        <button
          v-for="preset in mappingConfig.presets"
          :key="preset.id"
          class="preset-card"
          :class="{ selected: preset.id === mappingConfig.activePresetId }"
          role="tab"
          :aria-selected="preset.id === mappingConfig.activePresetId"
          @click="selectPreset(preset.id)"
        >
          <strong>{{ preset.name }}</strong>
          <span>{{ preset.bindings.length ? t("mapping.bindings", { count: preset.bindings.length }) : t("mapping.noAutomation") }}</span>
        </button>
      </div>
      <template v-if="activePreset">
        <div class="mapping-toolbar">
          <div>
            <p class="mapping-preset-title">{{ activePreset.name }}</p>
            <p class="mapping-input-status">{{ mappingInputStatus }}</p>
          </div>
          <label class="switch-control">
            <input v-model="activePreset.enabled" type="checkbox" />
            <span class="switch-track" aria-hidden="true"></span>
            <span>{{ activePreset.enabled ? t("mapping.enabled") : t("mapping.disabled") }}</span>
          </label>
        </div>
        <div v-if="activePreset.bindings.length" class="mapping-layout">
          <JoyCon side="left" :preview-target="previewTarget" />
          <div class="binding-list">
            <article v-for="binding in activePreset.bindings" :key="binding.id" class="binding-card">
              <div>
                <p class="binding-control">{{ controlName(binding.control) }}</p>
                <p class="binding-action">{{ actionName(binding.action) }}</p>
              </div>
              <label class="switch-control compact" @mouseenter="previewTarget = controlPreviewTarget(binding.control)" @mouseleave="previewTarget = undefined">
                <input v-model="binding.enabled" type="checkbox" />
                <span class="switch-track" aria-hidden="true"></span>
              </label>
            </article>
          </div>
          <JoyCon side="right" :preview-target="previewTarget" />
        </div>
        <p v-else class="empty-preset">{{ t("mapping.inspectOnly") }}</p>
      </template>
      <div class="mapping-actions">
        <button class="app-button mapping-test" @click="testWindowSwitch">{{ t("mapping.test") }}</button>
        <button v-if="!accessibilityGranted" class="app-button mapping-test" @click="openAccessibilitySettings">{{ t("mapping.openAccessibility") }}</button>
        <button class="secondary" @click="resetMappingConfig">{{ t("mapping.reset") }}</button>
      </div>
      <p class="mapping-feedback">{{ mappingFeedback }}</p>
    </section>
    <section v-show="activePage === 'debug'" class="panel">
      <div class="section-heading">
        <h2 class="section-title">{{ t("debug.paired") }}</h2>
        <span class="status" :class="statusKind">{{ status }}</span>
      </div>
      <div class="controllers">
        <template v-if="controllers.length"
          ><button
            v-for="controller in controllers"
            :key="controller.id"
            class="controller"
            :class="{
              'selected-controller': selectedControllers.some(
                ({ id }) => id === controller.id,
              ),
            }"
            :aria-pressed="
              selectedControllers.some(({ id }) => id === controller.id)
            "
            @click="selectController(controller)"
          >
            <span class="controller-check" aria-hidden="true">{{
              selectedControllers.some(({ id }) => id === controller.id)
                ? "✓"
                : ""
            }}</span
            ><strong>{{ controller.name }}</strong
            ><span class="controller-meta"
              >product 0x{{ controller.product_id.toString(16) }} ·
              {{ controller.transport }}</span
            >
          </button></template
        ><span v-else
          >{{ t("debug.noController") }}</span
        >
      </div>
    </section>
    <section v-show="activePage === 'debug'" class="visualizer panel">
      <div class="section-heading">
        <div>
          <h2 class="section-title">{{ t("debug.live") }}</h2>
          <p class="hint">
            {{ t("debug.visualizerHint") }}
          </p>
        </div>
        <output class="raw-buttons">{{ buttonsReadout }}</output>
      </div>
      <div class="joycon-stage">
        <JoyCon
          :active-controls="activeControls"
          :recent-controls="recentControls"
          :nub-transform="nubTransform"
        />
        <div class="axis-readout">
          <span class="readout-label">{{ t("debug.primaryStick") }}</span
          ><output class="axis-output">{{ renderStick(leftStick) }}</output
          ><span class="readout-label">{{ t("debug.secondaryAxes") }}</span
          ><output class="axis-output">{{ renderStick(rightStick) }}</output>
        </div>
        <JoyCon
          side="right"
          :active-controls="activeControls"
          :recent-controls="recentControls"
          :nub-transform="rightNubTransform"
        />
      </div>
    </section>
    <section v-show="activePage === 'debug'" class="panel transcript-panel">
      <div class="section-heading log-heading">
        <div>
          <h2 class="section-title">{{ t("debug.reports") }}</h2>
          <p class="hint">
            {{ t("debug.reportsHint") }}
          </p>
        </div>
        <div class="log-actions">
          <label
            >{{ t("debug.logPolicy") }}<select
              v-model="sampleRate"
              @change="
                lastPresentedAt.clear();
                lastKeyOperation.clear();
              "
            >
              <option value="key">{{ t("debug.keyOperations") }}</option>
              <option value="75">{{ t("debug.snapshots") }}</option>
              <option value="60">60 Hz</option>
              <option value="30">30 Hz</option>
              <option value="10">10 Hz</option>
              <option value="all">{{ t("debug.allReports") }}</option>
            </select></label
          ><button class="secondary" @click="clearLog">{{ t("debug.clear") }}</button>
        </div>
      </div>
      <div class="transcript">
        <template v-if="groupedLogs.length"
          ><div
            v-for="group in groupedLogs"
            :key="`${group.timestamp.getTime()}-${group.entries.map(({ device_id }) => device_id).join()}`"
            class="log-group"
          >
            <time>{{ group.timestamp.toLocaleTimeString() }}</time>
            <div class="log-lines">
              <button
                v-for="entry in group.entries"
                :key="`${entry.device_id}-${fingerprint(entry.report)}`"
                class="log-row"
                @click="openAnnotation(entry)"
              >
                <code
                  ><b>{{
                    controllers.find(({ id }) => id === entry.device_id)
                      ?.product_id === 0x2007
                      ? "R"
                      : "L"
                  }}</b>
                  {{ formatReport(entry.report) }}</code
                ><span
                  v-if="builtInLabels(entry).length"
                  class="annotation-tags"
                  ><span
                    v-for="label in builtInLabels(entry)"
                    :key="`${label.target}-${label.phase}`"
                    class="annotation-tag built-in"
                    >{{ labelText(label) }}</span
                  ></span
                ><span
                  v-else-if="savedAnnotation(entry)"
                  class="annotation-tag"
                  :class="{ legacy: !savedAnnotation(entry)?.previous_report }"
                  >{{
                    labelText(
                      savedAnnotation(entry)!.label,
                      !savedAnnotation(entry)?.previous_report,
                    )
                  }}</span
                ><span v-else class="label-prompt">{{ t("debug.label") }}</span>
              </button>
            </div>
          </div></template
        ><span v-else>{{ t("debug.chooseController") }}</span>
      </div>
    </section>
  </main>
  <dialog ref="dialog" class="annotation-modal">
    <form method="dialog" class="modal-card" @submit.prevent>
      <header>
        <div>
          <p class="eyebrow">{{ t("annotation.eyebrow") }}</p>
          <h2 class="section-title">{{ t("annotation.title") }}</h2>
        </div>
        <button class="secondary" type="button" @click="dialog?.close()">
          {{ t("annotation.close") }}
        </button>
      </header>
      <p class="selected-report">{{ selectedReportText }}</p>
      <div class="annotation-kinds">
        <button
          type="button"
          class="app-button kind"
          :class="{ active: annotationKind === 'stick' }"
          @click="setKind('stick')"
        >
          {{ t("annotation.stick") }}</button
        ><button
          type="button"
          class="app-button kind"
          :class="{ active: annotationKind === 'button' }"
          @click="setKind('button')"
        >
          {{ t("annotation.button") }}
        </button>
      </div>
      <template v-if="annotationKind === 'stick'"
        ><div class="phase-picker">
          <button
            type="button"
            class="app-button"
            :class="{ selected: stickPhase === 'moved' }"
            @click="
              stickPhase = 'moved';
              annotationTarget = undefined;
            "
          >
            {{ t("annotation.moved") }}</button
          ><button
            type="button"
            class="app-button"
            :class="{ selected: stickPhase === 'reset' }"
            @click="
              stickPhase = 'reset';
              chooseTarget('center');
            "
          >
            {{ t("annotation.reset") }}
          </button>
        </div>
        <div class="stick-radar">
          <button
            v-for="target in stickTargets"
            :key="target"
            type="button"
            class="radar-point"
            :class="[target, { selected: annotationTarget === target }]"
            :disabled="stickPhase === 'reset' && target !== 'center'"
            @click="chooseTarget(target)"
          >
            {{ target === "center" ? "•" : "" }}
          </button>
        </div></template
      >
      <div v-else class="button-picker-layout">
        <div class="annotation-joycon-wrap">
          <JoyCon
            class="annotation-joycon"
            :preview-target="previewTarget"
            :selected-target="annotationTarget"
          />
        </div>
        <div class="button-picker-column">
          <div class="phase-picker">
            <button
              type="button"
              class="app-button"
              :class="{ selected: buttonPhase === 'pressed' }"
              @click="buttonPhase = 'pressed'"
            >
              {{ t("annotation.pressed") }}</button
            ><button
              type="button"
              class="app-button"
              :class="{ selected: buttonPhase === 'released' }"
              @click="buttonPhase = 'released'"
            >
              {{ t("annotation.released") }}
            </button>
          </div>
          <div class="button-picker">
            <button
              v-for="target in buttonTargets"
              :key="target"
              type="button"
              class="app-button"
              :class="{ selected: annotationTarget === target }"
              @mouseenter="previewTarget = target"
              @mouseleave="previewTarget = undefined"
              @click="chooseTarget(target)"
            >
              {{ buttonNames[target] }}
            </button>
          </div>
        </div>
      </div>
      <p class="annotation-choice">{{ annotationChoice }}</p>
      <footer>
        <button
          class="app-button"
          type="button"
          :disabled="!annotationTarget"
          @click="saveAnnotation"
        >
          {{ t("annotation.save") }}
        </button>
      </footer>
    </form>
  </dialog>
</template>
