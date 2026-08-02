<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { MappingPreset } from "../types";

const props = defineProps<{ preset: MappingPreset; selected: boolean }>();
defineEmits<{ select: [] }>();
const { t } = useI18n();

const descriptions: Record<string, string> = {
  "codex-cowork": "presetCowork",
  "inspect-only": "presetInspect",
};
const glyphs: Record<string, string> = {
  "codex-cowork": "✦",
  "inspect-only": "◌",
};
</script>

<template>
  <button
    class="preset-card"
    :class="{ selected }"
    role="tab"
    :aria-selected="selected"
    @click="$emit('select')"
  >
    <span class="preset-glyph" aria-hidden="true">{{ glyphs[props.preset.id] ?? "·" }}</span>
    <span class="preset-content">
      <strong>{{ props.preset.name }}</strong>
      <small>{{ t(`mapping.${descriptions[props.preset.id] ?? "presetInspect"}`) }}</small>
    </span>
    <span class="preset-count">{{ props.preset.bindings.length ? t("mapping.bindings", { count: props.preset.bindings.length }) : t("mapping.noAutomation") }}</span>
  </button>
</template>
