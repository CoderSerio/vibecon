<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import ThreeJoyCon from "./ThreeJoyCon.vue";
import type {
  Controller,
  ImuSample,
  InputReport,
  Label,
  LogEntry,
  OrientationFrame,
  Stick,
} from "../types";

type LogGroup = { timestamp: Date; entries: LogEntry[] };
const props = defineProps<{
  controllers: Controller[];
  selectedControllers: Controller[];
  status: string;
  statusKind: "" | "connected" | "error";
  activeControls: string[];
  leftStick: Stick | null;
  rightStick: Stick | null;
  leftImu: ImuSample | null;
  rightImu: ImuSample | null;
  leftOrientation: OrientationFrame | null;
  rightOrientation: OrientationFrame | null;
  buttonsReadout: string;
  sampleRate: string;
  groupedLogs: LogGroup[];
  renderStick: (stick: Stick | null) => string;
  renderImu: (imu: ImuSample | null) => string;
  fingerprint: (report: InputReport) => string;
  formatReport: (report: InputReport) => string;
  builtInLabels: (entry: LogEntry) => Label[];
  savedAnnotation: (entry: LogEntry) => { previous_report?: unknown; label: Label } | undefined;
  labelText: (label: Label, legacy?: boolean) => string;
}>();

const emit = defineEmits<{
  selectController: [controller: Controller];
  updateSampleRate: [value: string];
  clear: [];
  annotate: [entry: LogEntry];
}>();
const { t } = useI18n();
const threeFollowMotion = ref(false);
const threeResetKey = ref(0);
const inspectionView = ref<"front" | "rail" | "shoulder">("front");
const storedSensitivity = Number(localStorage.getItem("vibecon.motion.pointerSensitivity"));
const motionSensitivity = ref(
  Number.isFinite(storedSensitivity) && storedSensitivity >= 1 && storedSensitivity <= 30
    ? storedSensitivity
    : 8,
);
watch(motionSensitivity, (value) => {
  localStorage.setItem("vibecon.motion.pointerSensitivity", String(value));
});
const maxVisibleLogEntries = 32;
const totalLogEntries = computed(() =>
  props.groupedLogs.reduce((total, group) => total + group.entries.length, 0),
);
const hasFusionOrientation = computed(() =>
  Boolean(props.leftOrientation || props.rightOrientation),
);
const visibleLogGroups = computed(() => {
  let remaining = maxVisibleLogEntries;
  const groups: LogGroup[] = [];
  for (const group of props.groupedLogs) {
    if (remaining <= 0) break;
    const entries = group.entries.slice(0, remaining);
    groups.push({ timestamp: group.timestamp, entries });
    remaining -= entries.length;
  }
  return groups;
});
const visibleLogEntries = computed(() =>
  visibleLogGroups.value.reduce((total, group) => total + group.entries.length, 0),
);

function resetThreePose() {
  threeResetKey.value += 1;
}
</script>

<template>
  <section class="panel">
    <div class="section-heading">
      <h2 class="section-title">{{ t("debug.paired") }}</h2>
      <span class="status" :class="statusKind">{{ status }}</span>
    </div>
    <div class="controllers">
      <template v-if="controllers.length">
        <button
          v-for="controller in controllers"
          :key="controller.id"
          class="controller"
          :class="{ 'selected-controller': selectedControllers.some(({ id }) => id === controller.id) }"
          :aria-pressed="selectedControllers.some(({ id }) => id === controller.id)"
          @click="emit('selectController', controller)"
        >
          <span class="controller-check" aria-hidden="true">{{ selectedControllers.some(({ id }) => id === controller.id) ? "✓" : "" }}</span>
          <strong>{{ controller.name }}</strong>
          <span class="controller-meta">product 0x{{ controller.product_id.toString(16) }} · {{ controller.transport }}</span>
        </button>
      </template>
      <span v-else>{{ t("debug.noController") }}</span>
    </div>
  </section>

  <section class="visualizer panel three-debug-panel">
    <div class="section-heading">
      <div>
        <h2 class="section-title">{{ t("debug.live") }}</h2>
        <p class="hint">{{ t("debug.threeHint") }}</p>
      </div>
      <div class="three-header-tools">
        <output class="raw-buttons">{{ buttonsReadout }}</output>
        <div class="three-debug-actions">
          <div class="three-view-switcher" :aria-label="t('debug.threeView')">
            <button v-for="view in (['front', 'rail', 'shoulder'] as const)" :key="view" type="button" :class="{ active: inspectionView === view }" @click="inspectionView = view">{{ t(`debug.threeView_${view}`) }}</button>
          </div>
          <label class="switch-control compact">
            <input v-model="threeFollowMotion" type="checkbox" />
            <span class="switch-track" aria-hidden="true"></span>
            <span>{{ t("debug.threeFollowMotion") }}</span>
          </label>
          <label v-if="!hasFusionOrientation" class="motion-sensitivity compact">
            <span>{{ t("debug.threeSensitivity") }}</span>
            <input v-model.number="motionSensitivity" type="range" min="1" max="30" step="1" />
            <output>{{ motionSensitivity }}×</output>
          </label>
          <button class="secondary" type="button" @click="resetThreePose">{{ t("debug.threeReset") }}</button>
        </div>
      </div>
    </div>
    <div class="three-joycon-stage">
      <ThreeJoyCon side="left" :imu="leftImu" :orientation="leftOrientation" :stick="leftStick" :active-controls="activeControls" :follow-motion="threeFollowMotion" :sensitivity="motionSensitivity" :reset-key="threeResetKey" :inspection-view="inspectionView" />
      <div class="axis-readout three-axis-readout">
        <span class="readout-label">{{ t("debug.primaryStick") }}</span><output class="axis-output">{{ renderStick(leftStick) }}</output>
        <span class="readout-label">{{ t("debug.secondaryAxes") }}</span><output class="axis-output">{{ renderStick(rightStick) }}</output>
        <span class="readout-label">{{ t("debug.leftImu") }}</span><output class="axis-output imu-output">{{ renderImu(leftImu) }}</output>
        <span class="readout-label">{{ t("debug.rightImu") }}</span><output class="axis-output imu-output">{{ renderImu(rightImu) }}</output>
      </div>
      <ThreeJoyCon side="right" :imu="rightImu" :orientation="rightOrientation" :stick="rightStick" :active-controls="activeControls" :follow-motion="threeFollowMotion" :sensitivity="motionSensitivity" :reset-key="threeResetKey" :inspection-view="inspectionView" />
    </div>
  </section>

  <section class="panel transcript-panel">
    <div class="section-heading log-heading">
      <div>
        <h2 class="section-title">{{ t("debug.reports") }}</h2>
        <p class="hint">{{ t("debug.reportsHint") }}</p>
      </div>
      <div class="log-actions">
        <span class="log-count" title="visible / retained">{{ visibleLogEntries }}/{{ totalLogEntries }}</span>
        <label>{{ t("debug.logPolicy") }}
          <select :value="sampleRate" @change="emit('updateSampleRate', ($event.target as HTMLSelectElement).value)">
            <option value="key">{{ t("debug.keyOperations") }}</option><option value="75">{{ t("debug.snapshots") }}</option><option value="60">60 Hz</option><option value="30">30 Hz</option><option value="10">10 Hz</option><option value="all">{{ t("debug.allReports") }}</option>
          </select>
        </label>
        <button class="secondary" @click="emit('clear')">{{ t("debug.clear") }}</button>
      </div>
    </div>
    <div class="transcript">
      <template v-if="visibleLogGroups.length">
        <div v-for="group in visibleLogGroups" :key="`${group.timestamp.getTime()}-${group.entries.map(({ device_id }) => device_id).join()}`" class="log-group">
          <time>{{ group.timestamp.toLocaleTimeString() }}</time>
          <div class="log-lines">
            <button v-for="entry in group.entries" :key="`${entry.device_id}-${fingerprint(entry.report)}`" class="log-row" @click="emit('annotate', entry)">
              <code><b>{{ controllers.find(({ id }) => id === entry.device_id)?.product_id === 0x2007 ? "R" : "L" }}</b> {{ formatReport(entry.report) }}</code>
              <span v-if="builtInLabels(entry).length" class="annotation-tags"><span v-for="label in builtInLabels(entry)" :key="`${label.target}-${label.phase}`" class="annotation-tag built-in">{{ labelText(label) }}</span></span>
              <span v-else-if="savedAnnotation(entry)" class="annotation-tag" :class="{ legacy: !savedAnnotation(entry)?.previous_report }">{{ labelText(savedAnnotation(entry)!.label, !savedAnnotation(entry)?.previous_report) }}</span>
              <span v-else class="label-prompt">{{ t("debug.label") }}</span>
            </button>
          </div>
        </div>
      </template>
      <span v-else>{{ t("debug.chooseController") }}</span>
    </div>
  </section>
</template>
