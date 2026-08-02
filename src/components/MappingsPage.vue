<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import BindingRoute from "./BindingRoute.vue";
import JoyCon from "./JoyCon.vue";
import PresetCard from "./PresetCard.vue";
import type { MappingConfig } from "../types";

const props = defineProps<{
  config: MappingConfig;
  inputStatus: string;
  feedback: string;
  accessibilityGranted: boolean;
}>();
const emit = defineEmits<{
  selectPreset: [id: string];
  updateConfig: [config: MappingConfig];
  copyPrompt: [];
  testWindow: [];
  testVibration: [];
  openAccessibility: [];
  reset: [];
}>();
const { t } = useI18n();

const activePreset = computed(() =>
  props.config.presets.find(({ id }) => id === props.config.activePresetId),
);
const enabledBindingCount = computed(
  () => activePreset.value?.bindings.filter(({ enabled }) => enabled).length ?? 0,
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

function updatePreview(target: string | undefined) {
  previewTarget.value = target;
}

const previewTarget = computed({
  get: () => previewTargetValue.value,
  set: (value: string | undefined) => { previewTargetValue.value = value; },
});
const previewTargetValue = ref<string>();
</script>

<template>
  <section class="panel mapping-panel">
    <div class="section-heading">
      <div>
        <p class="eyebrow">{{ t("mapping.eyebrow") }}</p>
        <h2 class="section-title">{{ t("mapping.title") }}</h2>
        <p class="hint">{{ t("mapping.subtitle") }}</p>
      </div>
      <button class="secondary" @click="emit('copyPrompt')">{{ t("mapping.copyPrompt") }}</button>
    </div>
    <div class="mapping-status-strip" aria-label="Mapping safety notes">
      <span><i aria-hidden="true">✓</i>{{ t("mapping.safeConfig") }}</span>
      <span><i aria-hidden="true">◉</i>{{ t("mapping.runtimeOnly") }}</span>
    </div>
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
          <p class="mapping-input-status">{{ props.inputStatus }} · {{ t("mapping.enabledBindings", { enabled: enabledBindingCount, total: activePreset.bindings.length }) }}</p>
        </div>
        <label class="switch-control">
          <input :checked="activePreset.enabled" type="checkbox" @change="updatePresetEnabled(($event.target as HTMLInputElement).checked)">
          <span class="switch-track" aria-hidden="true"></span>
          <span>{{ activePreset.enabled ? t("mapping.enabled") : t("mapping.disabled") }}</span>
        </label>
      </div>
      <div v-if="activePreset.bindings.length" class="mapping-layout">
        <JoyCon side="left" :preview-target="previewTarget" />
        <div class="binding-list">
          <BindingRoute
            v-for="binding in activePreset.bindings"
            :key="binding.id"
            :binding="binding"
            @preview="updatePreview"
            @toggle="updateBinding(binding.id, $event)"
          />
        </div>
        <JoyCon side="right" :preview-target="previewTarget" />
      </div>
      <p v-else class="empty-preset">{{ t("mapping.inspectOnly") }}</p>
    </template>
    <div class="mapping-actions">
      <button class="app-button mapping-test" @click="emit('testWindow')">{{ t("mapping.test") }}</button>
      <button class="app-button mapping-test" @click="emit('testVibration')">{{ t("mapping.testVibration") }}</button>
      <button v-if="!props.accessibilityGranted" class="app-button mapping-test" @click="emit('openAccessibility')">{{ t("mapping.openAccessibility") }}</button>
      <button class="secondary" @click="emit('reset')">{{ t("mapping.reset") }}</button>
    </div>
    <p class="mapping-feedback">{{ props.feedback }}</p>
  </section>
</template>
