<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import BindingRoute from "./BindingRoute.vue";
import PresetCard from "./PresetCard.vue";
import type {
  MappingConfig,
  PointerConfig,
  PointerMode,
  PointerRuntimeStatus,
} from "../types";

const props = defineProps<{
  config: MappingConfig;
  inputStatus: string;
  feedback: string;
  accessibilityGranted: boolean;
  pointerRuntimeStatus: PointerRuntimeStatus | null;
}>();
const emit = defineEmits<{
  selectPreset: [id: string];
  updateConfig: [config: MappingConfig];
  copyPrompt: [];
  testWindow: [];
  testVibration: [];
  testPointer: [];
  openAccessibility: [];
  reset: [];
}>();

const motionSweepPresets = [120, 90, 60, 45, 30] as const;
const { t } = useI18n();

const activePreset = computed(() =>
  props.config.presets.find(({ id }) => id === props.config.activePresetId),
);
const enabledBindingCount = computed(
  () =>
    activePreset.value?.bindings.filter(({ enabled }) => enabled).length ?? 0,
);

function updatePresetEnabled(enabled: boolean) {
  if (!activePreset.value) return;
  emit("updateConfig", {
    ...props.config,
    presets: props.config.presets.map((preset) =>
      preset.id === activePreset.value?.id ? { ...preset, enabled } : preset,
    ),
  });
}

function updateBinding(id: string, enabled: boolean) {
  if (!activePreset.value) return;
  emit("updateConfig", {
    ...props.config,
    presets: props.config.presets.map((preset) =>
      preset.id === activePreset.value?.id
        ? {
            ...preset,
            bindings: preset.bindings.map((binding) =>
              binding.id === id ? { ...binding, enabled } : binding,
            ),
          }
        : preset,
    ),
  });
}

function updatePointer(patch: Partial<PointerConfig>) {
  emit("updateConfig", {
    ...props.config,
    pointer: { ...props.config.pointer, ...patch },
  });
}

function updatePointerMode(mode: PointerMode) {
  updatePointer({ mode });
}

function updateStick(key: keyof PointerConfig["stick"], value: number) {
  updatePointer({
    stick: { ...props.config.pointer.stick, [key]: value },
  });
}

function updateMotion(key: keyof PointerConfig["motion"], value: number) {
  updatePointer({
    motion: { ...props.config.pointer.motion, [key]: value },
  });
}
</script>

<template>
  <section class="panel mapping-panel">
    <div class="section-heading">
      <div>
        <p class="eyebrow">{{ t("mapping.eyebrow") }}</p>
        <h2 class="section-title">{{ t("mapping.title") }}</h2>
        <p class="hint">{{ t("mapping.subtitle") }}</p>
      </div>
      <button class="secondary" @click="emit('copyPrompt')">
        {{ t("mapping.copyPrompt") }}
      </button>
    </div>
    <div class="mapping-status-strip" aria-label="Mapping safety notes">
      <span><i aria-hidden="true">✓</i>{{ t("mapping.safeConfig") }}</span>
      <span><i aria-hidden="true">◉</i>{{ t("mapping.runtimeOnly") }}</span>
    </div>
    <section
      class="pointer-config"
      :class="{ enabled: props.config.pointer.enabled }"
    >
      <header class="pointer-config-heading">
        <div>
          <p class="mapping-preset-title">{{ t("mapping.pointerTitle") }}</p>
          <p class="mapping-input-status">{{ t("mapping.pointerSubtitle") }}</p>
        </div>
        <label class="switch-control">
          <input
            :checked="props.config.pointer.enabled"
            type="checkbox"
            @change="
              updatePointer({
                enabled: ($event.target as HTMLInputElement).checked,
              })
            "
          />
          <span class="switch-track" aria-hidden="true"></span>
          <span>{{
            props.config.pointer.enabled
              ? t("mapping.enabled")
              : t("mapping.disabled")
          }}</span>
        </label>
      </header>
      <div
        class="pointer-permission"
        :class="props.accessibilityGranted ? 'ready' : 'blocked'"
      >
        <span class="pointer-permission-indicator" aria-hidden="true">{{
          props.accessibilityGranted ? "✓" : "!"
        }}</span>
        <div>
          <strong>{{
            props.accessibilityGranted
              ? t("mapping.pointerPermissionReady")
              : t("mapping.pointerPermissionBlocked")
          }}</strong>
          <small v-if="props.pointerRuntimeStatus">
            {{ props.pointerRuntimeStatus.backend }} ·
            {{ props.pointerRuntimeStatus.executablePath }}
          </small>
        </div>
        <button
          v-if="!props.accessibilityGranted"
          class="secondary"
          type="button"
          @click="emit('openAccessibility')"
        >
          {{ t("mapping.grantAccessibility") }}
        </button>
        <button
          v-else
          class="secondary"
          type="button"
          @click="emit('testPointer')"
        >
          {{ t("mapping.testPointer") }}
        </button>
      </div>
      <div
        class="pointer-mode-picker"
        role="radiogroup"
        :aria-label="t('mapping.pointerMode')"
      >
        <button
          v-for="mode in ['stick', 'motion'] as const"
          :key="mode"
          class="pointer-mode-card"
          :class="{ selected: props.config.pointer.mode === mode }"
          type="button"
          role="radio"
          :aria-checked="props.config.pointer.mode === mode"
          @click="updatePointerMode(mode)"
        >
          <span class="pointer-mode-icon" aria-hidden="true">{{
            mode === "stick" ? "◉" : "⌁"
          }}</span>
          <span>
            <strong>{{ t(`mapping.pointerMode_${mode}`) }}</strong>
            <small>{{ t(`mapping.pointerMode_${mode}Hint`) }}</small>
          </span>
        </button>
      </div>
      <div class="pointer-bindings-table">
        <div class="pointer-table-head">
          <span>{{ t("mapping.pointerAction") }}</span>
          <span>{{ t("mapping.pointerStickMode") }}</span>
          <span>{{ t("mapping.pointerMotionMode") }}</span>
        </div>
        <div>
          <strong>{{ t("mapping.pointerMove") }}</strong
          ><span>{{ t("mapping.pointerMoveStick") }}</span
          ><span>{{ t("mapping.pointerMoveMotion") }}</span>
        </div>
        <div>
          <strong>{{ t("mapping.pointerLeftClick") }}</strong
          ><span>L / R</span><span>L / R</span>
        </div>
        <div>
          <strong>{{ t("mapping.pointerRightClick") }}</strong
          ><span>ZL / ZR</span><span>ZL + L / ZR + R</span>
        </div>
        <div>
          <strong>{{ t("mapping.pointerLift") }}</strong
          ><span>—</span><span>{{ t("mapping.pointerLiftMotion") }}</span>
        </div>
        <div>
          <strong>{{ t("mapping.pointerAdjustSensitivity") }}</strong
          ><span>—</span
          ><span>{{ t("mapping.pointerAdjustSensitivityMotion") }}</span>
        </div>
        <div>
          <strong>{{ t("mapping.pointerHardRecenter") }}</strong
          ><span>—</span><span>{{ t("mapping.pointerTapRecenter") }}</span>
        </div>
        <div>
          <strong>{{ t("mapping.pointerSwitch") }}</strong
          ><span>{{ t("mapping.pointerHoldSwitch") }}</span
          ><span>{{ t("mapping.pointerHoldSwitch") }}</span>
        </div>
      </div>
      <div class="pointer-tuning">
        <template v-if="props.config.pointer.mode === 'stick'">
          <label
            >{{ t("mapping.pointerDeadzone")
            }}<input
              :value="props.config.pointer.stick.deadzone"
              min="0"
              max="0.5"
              step="0.01"
              type="range"
              @input="
                updateStick(
                  'deadzone',
                  Number(($event.target as HTMLInputElement).value),
                )
              "
            /><output>{{
              props.config.pointer.stick.deadzone.toFixed(2)
            }}</output></label
          >
          <label
            >{{ t("mapping.pointerMaxSpeed")
            }}<input
              :value="props.config.pointer.stick.maxSpeed"
              min="400"
              max="2400"
              step="50"
              type="range"
              @input="
                updateStick(
                  'maxSpeed',
                  Number(($event.target as HTMLInputElement).value),
                )
              "
            /><output
              >{{ props.config.pointer.stick.maxSpeed.toFixed(0) }} px/s</output
            ></label
          >
          <label
            >{{ t("mapping.pointerAcceleration")
            }}<input
              :value="props.config.pointer.stick.acceleration"
              min="0.7"
              max="3"
              step="0.1"
              type="range"
              @input="
                updateStick(
                  'acceleration',
                  Number(($event.target as HTMLInputElement).value),
                )
              "
            /><output>{{
              props.config.pointer.stick.acceleration.toFixed(1)
            }}</output></label
          >
        </template>
        <template v-else>
          <label
            >{{ t("mapping.pointerSweep")
            }}<select
              :value="props.config.pointer.motion.sweepDegrees"
              @input="
                updateMotion(
                  'sweepDegrees',
                  Number(($event.target as HTMLSelectElement).value),
                )
              "
            >
              <option
                v-for="degrees in motionSweepPresets"
                :key="degrees"
                :value="degrees"
              >
                {{ degrees }}°
              </option></select
            ><output>{{
              t("mapping.pointerSweepValue", {
                degrees: props.config.pointer.motion.sweepDegrees.toFixed(0),
              })
            }}</output></label
          >
          <label
            >{{ t("mapping.pointerVerticalRatio")
            }}<input
              :value="props.config.pointer.motion.verticalRatio"
              min="0.4"
              max="1.4"
              step="0.05"
              type="range"
              @input="
                updateMotion(
                  'verticalRatio',
                  Number(($event.target as HTMLInputElement).value),
                )
              "
            /><output>{{
              props.config.pointer.motion.verticalRatio.toFixed(2)
            }}</output></label
          >
          <label
            >{{ t("mapping.pointerNoise")
            }}<input
              :value="props.config.pointer.motion.noiseThreshold"
              min="0"
              max="0.15"
              step="0.005"
              type="range"
              @input="
                updateMotion(
                  'noiseThreshold',
                  Number(($event.target as HTMLInputElement).value),
                )
              "
            /><output
              >{{
                props.config.pointer.motion.noiseThreshold.toFixed(3)
              }}°</output
            ></label
          >
        </template>
      </div>
    </section>
    <div class="preset-picker" role="tablist" aria-label="Mapping presets">
      <PresetCard
        v-for="preset in props.config.presets"
        :key="preset.id"
        :preset="preset"
        :selected="preset.id === props.config.activePresetId"
        @select="emit('selectPreset', preset.id)"
      />
    </div>
    <template v-if="activePreset">
      <div class="mapping-toolbar">
        <div>
          <p class="mapping-preset-title">{{ activePreset.name }}</p>
          <p class="mapping-input-status">
            {{ props.inputStatus }} ·
            {{
              t("mapping.enabledBindings", {
                enabled: enabledBindingCount,
                total: activePreset.bindings.length,
              })
            }}
          </p>
        </div>
        <label class="switch-control">
          <input
            :checked="activePreset.enabled"
            type="checkbox"
            @change="
              updatePresetEnabled(($event.target as HTMLInputElement).checked)
            "
          />
          <span class="switch-track" aria-hidden="true"></span>
          <span>{{
            activePreset.enabled ? t("mapping.enabled") : t("mapping.disabled")
          }}</span>
        </label>
      </div>
      <div v-if="activePreset.bindings.length" class="mapping-layout">
        <div class="binding-list">
          <BindingRoute
            v-for="binding in activePreset.bindings"
            :key="binding.id"
            :binding="binding"
            @toggle="updateBinding(binding.id, $event)"
          />
        </div>
      </div>
      <p v-else class="empty-preset">{{ t("mapping.inspectOnly") }}</p>
    </template>
    <div class="mapping-actions">
      <button class="app-button mapping-test" @click="emit('testWindow')">
        {{ t("mapping.test") }}
      </button>
      <button class="app-button mapping-test" @click="emit('testVibration')">
        {{ t("mapping.testVibration") }}
      </button>
      <button class="app-button mapping-test" @click="emit('testPointer')">
        {{ t("mapping.testPointer") }}
      </button>
      <button class="secondary" @click="emit('reset')">
        {{ t("mapping.reset") }}
      </button>
    </div>
    <p class="mapping-feedback">{{ props.feedback }}</p>
  </section>
</template>
