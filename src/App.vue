<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import JoyCon from "./components/JoyCon.vue";
import DebugPage from "./components/DebugPage.vue";
import MappingsPage from "./components/MappingsPage.vue";
import { isAppLocale, LOCALE_STORAGE_KEY, type AppLocale } from "./i18n";
import type {
  Controller,
  ImuSample,
  InputReport,
  Label,
  LogEntry,
  MappingConfig,
  OrientationFrame,
  PointerMoveTestResult,
  PointerRuntimeStatus,
  Stick,
} from "./types";
import { RingBuffer } from "./utils/ring-buffer";
type Annotation = {
  version: number;
  created_at_ms: number;
  controller: { vendor_id: number; product_id: number; orientation: string };
  previous_report?: { report_id: number; bytes: number[] };
  report: { report_id: number; bytes: number[] };
  label: Label;
};
type StreamEvent = {
  device_id: string;
  report: InputReport;
  orientation: OrientationFrame | null;
};

function defaultMappingConfig(): MappingConfig {
  return {
    version: 3,
    activePresetId: "codex-cowork",
    presets: [
      {
        id: "codex-cowork",
        name: "Codex Cowork",
        enabled: true,
        bindings: [
          {
            id: "window-previous",
            control: "joycon_left.stick_left",
            action: "window_previous",
            enabled: true,
          },
          {
            id: "window-next",
            control: "joycon_left.stick_right",
            action: "window_next",
            enabled: true,
          },
          {
            id: "focus-codex-left",
            control: "joycon_left.dpad_up",
            action: "focus_codex",
            enabled: true,
          },
          {
            id: "focus-codex-right",
            control: "joycon_right.x",
            action: "focus_codex",
            enabled: true,
          },
        ],
      },
      {
        id: "inspect-only",
        name: "Inspect Only",
        enabled: false,
        bindings: [],
      },
    ],
    pointer: {
      enabled: false,
      mode: "stick",
      modeSwitchHoldMs: 600,
      stick: { deadzone: 0.12, maxSpeed: 1400, acceleration: 1.6 },
      motion: { sweepDegrees: 60, verticalRatio: 0.85, noiseThreshold: 0.015 },
    },
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
const logBuffer = new RingBuffer<LogEntry>(32);
const logVersion = ref(0);
const annotations = ref<Annotation[]>([]);
const sampleRate = ref("key");
const leftStick = ref<Stick | null>(null);
const rightStick = ref<Stick | null>(null);
const leftImu = ref<ImuSample | null>(null);
const rightImu = ref<ImuSample | null>(null);
const leftOrientation = ref<OrientationFrame | null>(null);
const rightOrientation = ref<OrientationFrame | null>(null);
const activeControlsBySide = ref({
  left: [] as string[],
  right: [] as string[],
});
const activeControls = computed(() => [
  ...activeControlsBySide.value.left,
  ...activeControlsBySide.value.right,
]);
const mappingInputStatus = computed(() => {
  if (!selectedControllers.value.length) return t("mapping.noController");
  return t("mapping.selected", {
    name: selectedControllers.value.map(({ name }) => name).join(" + "),
  });
});
const buttonsReadout = ref("D-pad 00 · waiting for input");
const dialog = ref<HTMLDialogElement>();
const selectedLog = ref<LogEntry>();
const annotationKind = ref<"stick" | "button">("stick");
const annotationTarget = ref<string>();
const mappingFeedback = ref(t("mapping.initial"));
const accessibilityGranted = ref(false);
const pointerRuntimeStatus = ref<PointerRuntimeStatus | null>(null);
const pointerHud = ref("");
const pauseMappingsOnDebug = ref(false);
const leftMotionResetKey = ref(0);
const rightMotionResetKey = ref(0);
const buttonPhase = ref<"pressed" | "released">("pressed");
const stickPhase = ref<"moved" | "reset">("moved");
let unlistenInput: UnlistenFn | undefined;
let unlistenError: UnlistenFn | undefined;
let unlistenPointerMode: UnlistenFn | undefined;
let unlistenPointerError: UnlistenFn | undefined;
let unlistenPointerSweep: UnlistenFn | undefined;
let unlistenPointerStickSpeed: UnlistenFn | undefined;
let unlistenPointerRecenter: UnlistenFn | undefined;
let unlistenPointerGesture: UnlistenFn | undefined;
let pointerHudTimer: number | undefined;
let controllerPollTimer: number | undefined;
let refreshingControllers = false;
const knownControllerIds = new Set<string>();
const lastPresentedAt = new Map<string, number>();
const lastKeyOperation = new Map<string, string>();
const lastIncomingReport = new Map<string, InputReport>();

function showPointerHud(message: string) {
  pointerHud.value = message;
  if (pointerHudTimer !== undefined) window.clearTimeout(pointerHudTimer);
  pointerHudTimer = window.setTimeout(() => {
    pointerHud.value = "";
    pointerHudTimer = undefined;
  }, 1300);
}

watch(activePage, (page) => {
  if (page === "mappings") void checkMappingAccessibility();
  void syncMappingRuntime();
});
watch(pauseMappingsOnDebug, () => void syncMappingRuntime());
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
  // Fixed slots are not deeply reactive. One counter invalidates this view.
  logVersion.value;
  const groups: Array<{ timestamp: Date; entries: LogEntry[] }> = [];
  for (const entry of logBuffer.newestFirst()) {
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
function renderImu(imu: ImuSample | null) {
  if (!imu) return t("debug.noImu");
  const format = ([x, y, z]: [number, number, number]) =>
    `x ${x}\ny ${y}\nz ${z}`;
  return `accel\n${format(imu.acceleration)}\n\ngyro\n${format(imu.gyroscope)}`;
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
      isRight
        ? [
            { byte: 3, mask: 0x01, target: "joycon_right.y" },
            { byte: 3, mask: 0x02, target: "joycon_right.x" },
            { byte: 3, mask: 0x04, target: "joycon_right.b" },
            { byte: 3, mask: 0x08, target: "joycon_right.a" },
            { byte: 3, mask: 0x10, target: "joycon_right.sr" },
            { byte: 3, mask: 0x20, target: "joycon_right.sl" },
            { byte: 3, mask: 0x40, target: "joycon_right.r" },
            { byte: 3, mask: 0x80, target: "joycon_right.zr" },
            { byte: 4, mask: 0x02, target: "joycon_right.plus" },
            { byte: 4, mask: 0x08, target: "joycon_right.stick_press" },
            { byte: 4, mask: 0x10, target: "joycon_right.home" },
          ]
        : [
            { byte: 4, mask: 0x01, target: "joycon_left.minus" },
            { byte: 4, mask: 0x08, target: "joycon_left.stick_press" },
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
function applyReport(
  report: InputReport,
  controller: Controller,
  orientation: OrientationFrame | null,
) {
  const side = controller.product_id === 0x2007 ? "right" : "left";
  // IMU samples feed Motion Lab on every page. The previous guard below made
  // the visualizer retain only the sample present when the page was mounted.
  if (side === "left") {
    leftStick.value = report.left_stick;
    leftImu.value = report.imu;
    if (orientation) leftOrientation.value = orientation;
  } else {
    rightStick.value = report.right_stick;
    rightImu.value = report.imu;
    if (orientation) rightOrientation.value = orientation;
  }
  // Keep the shared Joy-Con stage live on both tabs. Only raw log retention is
  // paused on Mappings; the visual input state remains useful while tuning.
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
          "joycon_left.stick_press": 0x08,
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
  try {
    const result = await invoke<PointerRuntimeStatus>("pointer_runtime_status");
    pointerRuntimeStatus.value = result;
    accessibilityGranted.value = result.accessibilityGranted;
    mappingFeedback.value = result.accessibilityGranted
      ? t("mapping.accessibilityReady")
      : t("mapping.accessibilityBlocked", { path: result.executablePath });
  } catch (error) {
    mappingFeedback.value = `Could not check Accessibility: ${String(error)}`;
  }
}
function requestAccessibilityPermission() {
  mappingFeedback.value = t("mapping.requestingAccessibility");
  void invoke<boolean>("request_accessibility_permission")
    .then((granted) => {
      accessibilityGranted.value = granted;
      mappingFeedback.value = granted
        ? t("mapping.accessibilityReady")
        : t("mapping.opened");
      void checkMappingAccessibility();
    })
    .catch((error) => {
      mappingFeedback.value = `Could not request Accessibility: ${String(error)}`;
    });
}
function testPointerMovement() {
  mappingFeedback.value = t("mapping.pointerTestSending");
  void invoke<PointerMoveTestResult>("test_pointer_move")
    .then((result) => {
      mappingFeedback.value = t("mapping.pointerTestSent", {
        x: result.actualX.toFixed(0),
        y: result.actualY.toFixed(0),
      });
      void checkMappingAccessibility();
    })
    .catch((error) => {
      mappingFeedback.value = t("mapping.pointerTestFailed", {
        error: String(error),
      });
      void checkMappingAccessibility();
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
function testJoyConVibration() {
  if (!selectedControllers.value.length) {
    mappingFeedback.value = t("mapping.vibrationNoController");
    return;
  }
  mappingFeedback.value = t("mapping.vibrationSending");
  void Promise.all(
    selectedControllers.value.map(({ id }) =>
      invoke("test_joycon_vibration", { id }),
    ),
  )
    .then(() => {
      mappingFeedback.value = t("mapping.vibrationSent", {
        count: selectedControllers.value.length,
      });
    })
    .catch((error) => {
      mappingFeedback.value = `Vibration test failed: ${String(error)}`;
      showError(error);
    });
}
async function syncMappingRuntime() {
  if (!isTauriDesktop || !mappingConfig.value.presets.length) return;
  try {
    await invoke("set_mapping_runtime", {
      config: mappingConfig.value,
      active: activePage.value === "mappings" || !pauseMappingsOnDebug.value,
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
async function copyAgentPrompt() {
  const prompt = `You are editing VibeCon's local configuration at ~/.vibecon/mappings.json. Keep version 3 and preserve the pointer block. Pointer modes are stick and motion; the default mode switch is a 600 ms hold on Joy-Con (L) minus or Joy-Con (R) plus. In stick mode, L/R is left click and ZL/ZR is right click. In motion mode, L/R is left click, ZL+L or ZR+R is right click, holding ZL/ZR freezes then recenters on release, tapping minus/plus hard-recenters the current pose, and SL/SR select a slower/faster full-screen sweep preset. Keep motion.sweepDegrees at one of 30, 45, 60, 90, or 120. Shortcut bindings may only use the existing controls and safe actions window_previous, window_next, and focus_codex. Preserve valid JSON and unique ids, then explain the change. Do not add shell commands or arbitrary automation.`;
  try {
    await navigator.clipboard.writeText(prompt);
    mappingFeedback.value = t("mapping.copied");
  } catch (error) {
    mappingFeedback.value = `Could not copy the agent prompt: ${String(error)}`;
  }
}
function setLocale(next: string) {
  if (!isAppLocale(next)) return;
  locale.value = next as AppLocale;
  localStorage.setItem(LOCALE_STORAGE_KEY, next);
}
function setActiveControls(next: string[], side: "left" | "right") {
  const current = activeControlsBySide.value[side];
  if (
    current.length === next.length &&
    current.every((target, index) => target === next[index])
  ) {
    return;
  }
  activeControlsBySide.value[side] = next;
}
function showError(error: unknown) {
  status.value = String(error);
  statusKind.value = "error";
}
function clearLog() {
  logBuffer.clear();
  logVersion.value += 1;
}
function setSampleRate(value: string) {
  sampleRate.value = value;
  lastPresentedAt.clear();
  lastKeyOperation.clear();
}
function appendLog(
  device_id: string,
  report: InputReport,
  previous_report?: InputReport,
) {
  logBuffer.push({ timestamp: new Date(), device_id, previous_report, report });
  logVersion.value += 1;
}
function selectController(controller: Controller) {
  const selected = selectedControllers.value.some(
    ({ id }) => id === controller.id,
  );
  if (selected) {
    selectedControllers.value = selectedControllers.value.filter(
      ({ id }) => id !== controller.id,
    );
    if (controller.product_id === 0x2007) {
      rightOrientation.value = null;
      rightImu.value = null;
    } else {
      leftOrientation.value = null;
      leftImu.value = null;
    }
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
  if (refreshingControllers) return;
  if (!isTauriDesktop) {
    showError("Browser preview detected. Run `pnpm tauri dev` for HID access.");
    return;
  }
  refreshingControllers = true;
  status.value = "Checking HID devices…";
  try {
    const discovered = await invoke<Controller[]>("list_joycons");
    const discoveredIds = new Set(discovered.map(({ id }) => id));
    const disconnected = selectedControllers.value.filter(
      ({ id }) => !discoveredIds.has(id),
    );
    for (const controller of disconnected) {
      await invoke("stop_joycon_stream", { id: controller.id });
      if (controller.product_id === 0x2007) {
        rightOrientation.value = null;
        rightImu.value = null;
      } else {
        leftOrientation.value = null;
        leftImu.value = null;
      }
      lastKeyOperation.delete(controller.id);
      lastIncomingReport.delete(controller.id);
    }

    controllers.value = discovered;
    selectedControllers.value = selectedControllers.value.filter(({ id }) =>
      discoveredIds.has(id),
    );

    // Auto-select only devices that appeared since the previous scan. This
    // keeps an intentional manual deselection intact while still making a
    // newly connected Joy-Con immediately usable.
    for (const controller of discovered) {
      const isNew = !knownControllerIds.has(controller.id);
      const isSelected = selectedControllers.value.some(
        ({ id }) => id === controller.id,
      );
      if (isNew && !isSelected && selectedControllers.value.length < 2) {
        selectedControllers.value = [...selectedControllers.value, controller];
        lastKeyOperation.delete(controller.id);
        lastIncomingReport.delete(controller.id);
      }
      // Starting an already active stream is idempotent in Rust. Retrying it
      // on every discovery pass also reconnects a stream whose Bluetooth HID
      // reader exited while the device itself remained discoverable.
      if (
        selectedControllers.value.some(({ id }) => id === controller.id)
      ) {
        await invoke("start_joycon_stream", { id: controller.id }).catch(
          showError,
        );
      }
      knownControllerIds.add(controller.id);
    }
    for (const id of [...knownControllerIds]) {
      if (!discoveredIds.has(id)) knownControllerIds.delete(id);
    }
    if (selectedControllers.value.length) {
      status.value = `Streaming ${selectedControllers.value
        .map(({ name }) => name)
        .join(" + ")}. Move a stick or press a button.`;
      statusKind.value = "connected";
    } else {
      status.value = "Choose one or two controllers to start streaming.";
      statusKind.value = "";
    }
  } catch (error) {
    showError(error);
  } finally {
    refreshingControllers = false;
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
  const loadedMappingConfig = await invoke<MappingConfig>(
    "load_mapping_config",
  ).catch((error) => {
    showError(error);
    return mappingConfig.value;
  });
  mappingConfig.value = loadedMappingConfig;
  await syncMappingRuntime();
  unlistenInput = await listen<StreamEvent>("joycon-input", ({ payload }) => {
    const controller = selectedControllers.value.find(
      ({ id }) => id === payload.device_id,
    );
    if (!controller) return;
    applyReport(payload.report, controller, payload.orientation);
    if (activePage.value !== "debug") return;
    const previous = lastIncomingReport.get(payload.device_id);
    lastIncomingReport.set(payload.device_id, payload.report);
    if (shouldLog(payload.report, payload.device_id))
      appendLog(payload.device_id, payload.report, previous);
  });
  unlistenError = await listen<string>("joycon-stream-error", ({ payload }) =>
    showError(payload),
  );
  unlistenPointerMode = await listen<"stick" | "motion">(
    "pointer-mode-changed",
    ({ payload }) => {
      mappingConfig.value.pointer.mode = payload;
      mappingFeedback.value = t(
        payload === "stick"
          ? "mapping.modeChangedStick"
          : "mapping.modeChangedMotion",
      );
    },
  );
  unlistenPointerSweep = await listen<number>(
    "pointer-sweep-changed",
    ({ payload }) => {
      mappingConfig.value.pointer.motion.sweepDegrees = payload;
      const message = t("mapping.sweepChanged", {
        degrees: payload.toFixed(0),
      });
      mappingFeedback.value = message;
      showPointerHud(message);
    },
  );
  unlistenPointerStickSpeed = await listen<number>(
    "pointer-stick-speed-changed",
    ({ payload }) => {
      mappingConfig.value.pointer.stick.maxSpeed = payload;
      const message = t("mapping.stickSpeedChanged", {
        speed: payload.toFixed(0),
      });
      mappingFeedback.value = message;
      showPointerHud(message);
    },
  );
  unlistenPointerRecenter = await listen<number>(
    "pointer-recentered",
    ({ payload }) => {
      if (payload === 0x2007) rightMotionResetKey.value += 1;
      else leftMotionResetKey.value += 1;
      const message = t("mapping.pointerRecentered");
      mappingFeedback.value = message;
      showPointerHud(message);
    },
  );
  unlistenPointerGesture = await listen<"stick-click" | "scroll-ready">(
    "pointer-gesture",
    ({ payload }) => {
      const message =
        payload === "stick-click"
          ? t("mapping.pointerStickClickDetected")
          : t("mapping.pointerScrollReady");
      mappingFeedback.value = message;
      showPointerHud(message);
    },
  );
  unlistenPointerError = await listen<string>(
    "pointer-runtime-error",
    ({ payload }) => {
      accessibilityGranted.value = false;
      mappingFeedback.value = payload;
      void checkMappingAccessibility();
    },
  );
  await refreshControllers();
  controllerPollTimer = window.setInterval(() => {
    void refreshControllers();
  }, 5000);
});
onBeforeUnmount(() => {
  unlistenInput?.();
  unlistenError?.();
  unlistenPointerMode?.();
  unlistenPointerError?.();
  unlistenPointerSweep?.();
  unlistenPointerStickSpeed?.();
  unlistenPointerRecenter?.();
  unlistenPointerGesture?.();
  if (pointerHudTimer !== undefined) window.clearTimeout(pointerHudTimer);
  if (controllerPollTimer !== undefined) window.clearInterval(controllerPollTimer);
  if (isTauriDesktop) void invoke("stop_joycon_stream");
});
</script>

<template>
  <main class="app-shell">
    <Transition name="pointer-hud">
      <div v-if="pointerHud" class="pointer-hud" role="status">
        {{ pointerHud }}
      </div>
    </Transition>
    <header class="app-header">
      <div>
        <p class="eyebrow">{{ t("app.eyebrow") }}</p>
        <h1 class="app-title">VibeCon</h1>
        <p class="subtitle">{{ t("app.subtitle") }}</p>
      </div>
      <div class="header-actions">
        <label class="language-picker"
          ><span class="sr-only">{{ t("app.language") }}</span
          ><select
            :value="locale"
            @change="setLocale(($event.target as HTMLSelectElement).value)"
          >
            <option value="en">EN</option>
            <option value="zh-CN">中文</option>
          </select></label
        >
        <button class="app-button" @click="refreshControllers">
          {{ t("app.refresh") }}
        </button>
      </div>
    </header>
    <DebugPage
      :show-logs="activePage === 'debug'"
      :active-page="activePage"
      :controllers="controllers"
      :selected-controllers="selectedControllers"
      :status="status"
      :status-kind="statusKind"
      :active-controls="activeControls"
      :left-stick="leftStick"
      :right-stick="rightStick"
      :left-imu="leftImu"
      :right-imu="rightImu"
      :left-orientation="leftOrientation"
      :right-orientation="rightOrientation"
      :left-motion-reset-key="leftMotionResetKey"
      :right-motion-reset-key="rightMotionResetKey"
      :mappings-paused="pauseMappingsOnDebug"
      :buttons-readout="buttonsReadout"
      :sample-rate="sampleRate"
      :grouped-logs="groupedLogs"
      :render-stick="renderStick"
      :render-imu="renderImu"
      :fingerprint="fingerprint"
      :format-report="formatReport"
      :built-in-labels="builtInLabels"
      :saved-annotation="savedAnnotation"
      :label-text="labelText"
      @navigate="activePage = $event"
      @select-controller="selectController"
      @update-mappings-paused="pauseMappingsOnDebug = $event"
      @update-sample-rate="setSampleRate"
      @clear="clearLog"
      @annotate="openAnnotation"
    />
    <MappingsPage
      v-if="activePage === 'mappings'"
      :config="mappingConfig"
      :input-status="mappingInputStatus"
      :feedback="mappingFeedback"
      :accessibility-granted="accessibilityGranted"
      :pointer-runtime-status="pointerRuntimeStatus"
      @select-preset="selectPreset"
      @update-config="mappingConfig = $event"
      @copy-prompt="copyAgentPrompt"
      @test-window="testWindowSwitch"
      @test-vibration="testJoyConVibration"
      @test-pointer="testPointerMovement"
      @open-accessibility="requestAccessibilityPermission"
      @reset="resetMappingConfig"
    />
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
