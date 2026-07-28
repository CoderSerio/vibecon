<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import JoyCon from "./components/JoyCon.vue";

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

const isTauriDesktop = "__TAURI_INTERNALS__" in window;
const controllers = ref<Controller[]>([]);
const selectedController = ref<Controller>();
const status = ref("Looking for Nintendo HID devices…");
const statusKind = ref<"" | "connected" | "error">("");
const logs = ref<LogEntry[]>([]);
const annotations = ref<Annotation[]>([]);
const sampleRate = ref("key");
const leftStick = ref<Stick | null>(null);
const rightStick = ref<Stick | null>(null);
const activeControls = ref<string[]>([]);
const recentControls = ref<string[]>([]);
const buttonsReadout = ref("D-pad 00 · waiting for input");
const dialog = ref<HTMLDialogElement>();
const selectedLog = ref<LogEntry>();
const annotationKind = ref<"stick" | "button">("stick");
const annotationTarget = ref<string>();
const buttonPhase = ref<"pressed" | "released">("pressed");
const stickPhase = ref<"moved" | "reset">("moved");
const previewTarget = ref<string>();
let unlistenInput: UnlistenFn | undefined;
let unlistenError: UnlistenFn | undefined;
let lastPresentedAt = 0;
let lastKeyOperation: string | undefined;
let lastIncomingReport: InputReport | undefined;
let recentControlsTimer: ReturnType<typeof setTimeout> | undefined;

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
const rightNubTransform = computed(() => stickTransform(rightStick.value));

function stickTransform(stick: Stick | null) {
  if (!stick) return "translate(0, 0)";
  const x = Math.max(-1, Math.min(1, stick.normalized_x)) * 16;
  const y = Math.max(-1, Math.min(1, stick.normalized_y)) * 16;
  return `translate(${x.toFixed(1)}px, ${y.toFixed(1)}px)`;
}
const selectedReportText = computed(() =>
  selectedLog.value
    ? `report 0x${hex(selectedLog.value.report.report_id)} · ${selectedLog.value.report.bytes.map(hex).join(" ")}`
    : "",
);
const annotationChoice = computed(() =>
  !annotationTarget.value
    ? "Choose a fixed target."
    : `Selected: ${annotationKind.value === "stick" ? `${annotationTarget.value} · ${stickPhase.value}` : `${buttonNames[annotationTarget.value]} · ${buttonPhase.value}`}`,
);

function hex(byte: number) {
  return byte.toString(16).padStart(2, "0").toUpperCase();
}
function fingerprint(report: { report_id: number; bytes: number[] }) {
  return `${report.report_id}:${report.bytes.map(hex).join("")}`;
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
function shouldLog(report: InputReport) {
  if (sampleRate.value === "all") return true;
  if (sampleRate.value === "key") {
    const current = `${report.buttons?.join(":") ?? "none"}|${stickBucket(report.left_stick)}|${stickBucket(report.right_stick)}`;
    if (current === lastKeyOperation) return false;
    lastKeyOperation = current;
    return true;
  }
  const interval =
    sampleRate.value === "75" ? 75 : 1000 / Number(sampleRate.value);
  if (performance.now() - lastPresentedAt < interval) return false;
  lastPresentedAt = performance.now();
  return true;
}
function applyReport(report: InputReport) {
  leftStick.value = report.left_stick;
  rightStick.value = report.right_stick;
  if (!report.buttons) {
    activeControls.value = [];
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
      mappings.flatMap(([bits, controls]) =>
        Object.entries(controls)
          .filter(([, mask]) => bits & mask)
          .map(([target]) => target),
      ),
    );
    buttonsReadout.value = `Native 0x30 · R 0x${hex(buttonMask)} · shared 0x${hex(extraButtons)} · L 0x${hex(hat)}`;
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
  );
  buttonsReadout.value = `D-pad 0x${hex(buttonMask)} · stick HAT ${hat === 8 ? "neutral" : hat} · extra 0x${hex(extraButtons)}`;
}
function setActiveControls(next: string[]) {
  const newlyPressed = next.filter(
    (target) => !activeControls.value.includes(target),
  );
  activeControls.value = next;
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
function appendLog(report: InputReport, previous_report?: InputReport) {
  logs.value = [
    { timestamp: new Date(), previous_report, report },
    ...logs.value,
  ].slice(0, 160);
}
function selectController(controller: Controller) {
  selectedController.value = controller;
  lastKeyOperation = undefined;
  lastIncomingReport = undefined;
  status.value = `Streaming ${controller.name}. Move a stick or press a button.`;
  statusKind.value = "connected";
  void invoke("start_joycon_stream", { id: controller.id }).catch(showError);
}
async function refreshControllers() {
  if (!isTauriDesktop) {
    showError("Browser preview detected. Run `pnpm tauri dev` for HID access.");
    return;
  }
  status.value = "Checking HID devices…";
  try {
    controllers.value = await invoke<Controller[]>("list_joycons");
    if (controllers.value.length) selectController(controllers.value[0]);
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
  if (
    !selectedLog.value ||
    !selectedController.value ||
    !annotationTarget.value
  )
    return;
  try {
    const annotation = await invoke<Annotation>("save_annotation", {
      draft: {
        controller: {
          vendor_id: 0x057e,
          product_id: selectedController.value.product_id,
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
    await refreshControllers();
    return;
  }
  annotations.value = await invoke<Annotation[]>("load_annotations").catch(
    (error) => {
      showError(error);
      return [];
    },
  );
  unlistenInput = await listen<StreamEvent>("joycon-input", ({ payload }) => {
    if (payload.device_id !== selectedController.value?.id) return;
    applyReport(payload.report);
    const previous = lastIncomingReport;
    lastIncomingReport = payload.report;
    if (shouldLog(payload.report)) appendLog(payload.report, previous);
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
        <p class="eyebrow">READ-ONLY HID INSPECTOR</p>
        <h1 class="app-title">VibeCon</h1>
        <p class="subtitle">See the Joy-Con before assigning it an action.</p>
      </div>
      <button class="app-button" @click="refreshControllers">
        Refresh controllers
      </button>
    </header>
    <section class="panel">
      <div class="section-heading">
        <h2 class="section-title">Paired controllers</h2>
        <span class="status" :class="statusKind">{{ status }}</span>
      </div>
      <div class="controllers">
        <template v-if="controllers.length"
          ><button
            v-for="controller in controllers"
            :key="controller.id"
            class="controller"
            :class="{
              'selected-controller': selectedController?.id === controller.id,
            }"
            @click="selectController(controller)"
          >
            <strong>{{ controller.name }}</strong
            ><span class="controller-meta"
              >product 0x{{ controller.product_id.toString(16) }} ·
              {{ controller.transport }}</span
            >
          </button></template
        ><span v-else
          >No Nintendo controller found. Confirm Bluetooth pairing, then click
          Refresh.</span
        >
      </div>
    </section>
    <section class="visualizer panel">
      <div class="section-heading">
        <div>
          <h2 class="section-title">Live Joy-Con</h2>
          <p class="hint">
            Blue controls are detected; dim controls still need a confirmed bit
            mapping.
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
          <span class="readout-label">Primary stick (macOS HAT)</span
          ><output class="axis-output">{{ renderStick(leftStick) }}</output
          ><span class="readout-label">Secondary axes</span
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
    <section class="panel transcript-panel">
      <div class="section-heading log-heading">
        <div>
          <h2 class="section-title">Raw input reports</h2>
          <p class="hint">
            Click any report to label it. Clearing the view never deletes saved
            labels.
          </p>
        </div>
        <div class="log-actions">
          <label
            >Log policy<select
              v-model="sampleRate"
              @change="
                lastPresentedAt = 0;
                lastKeyOperation = undefined;
              "
            >
              <option value="key">Key operations</option>
              <option value="75">75 ms snapshots (original)</option>
              <option value="60">60 Hz</option>
              <option value="30">30 Hz</option>
              <option value="10">10 Hz</option>
              <option value="all">All reports</option>
            </select></label
          ><button class="secondary" @click="clearLog">Clear</button>
        </div>
      </div>
      <div class="transcript">
        <template v-if="logs.length"
          ><button
            v-for="entry in logs"
            :key="`${entry.timestamp.getTime()}-${fingerprint(entry.report)}`"
            class="log-row"
            @click="openAnnotation(entry)"
          >
            <time>{{ entry.timestamp.toLocaleTimeString() }}</time
            ><code
              >report 0x{{ hex(entry.report.report_id) }}
              {{ entry.report.bytes.map(hex).join(" ") }}</code
            ><span v-if="builtInLabels(entry).length" class="annotation-tags"
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
            ><span v-else class="label-prompt">Label</span>
          </button></template
        ><span v-else
          >Choose a Joy-Con, then move a stick or press a button.</span
        >
      </div>
    </section>
  </main>
  <dialog ref="dialog" class="annotation-modal">
    <form method="dialog" class="modal-card" @submit.prevent>
      <header>
        <div>
          <p class="eyebrow">ANNOTATE SAMPLE</p>
          <h2 class="section-title">Give this report a meaning</h2>
        </div>
        <button class="secondary" value="cancel">Close</button>
      </header>
      <p class="selected-report">{{ selectedReportText }}</p>
      <div class="annotation-kinds">
        <button
          type="button"
          class="app-button kind"
          :class="{ active: annotationKind === 'stick' }"
          @click="setKind('stick')"
        >
          Stick operation</button
        ><button
          type="button"
          class="app-button kind"
          :class="{ active: annotationKind === 'button' }"
          @click="setKind('button')"
        >
          Button operation
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
            Moved</button
          ><button
            type="button"
            class="app-button"
            :class="{ selected: stickPhase === 'reset' }"
            @click="
              stickPhase = 'reset';
              chooseTarget('center');
            "
          >
            Reset to center
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
              Pressed</button
            ><button
              type="button"
              class="app-button"
              :class="{ selected: buttonPhase === 'released' }"
              @click="buttonPhase = 'released'"
            >
              Released
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
          Save annotation
        </button>
      </footer>
    </form>
  </dialog>
</template>
