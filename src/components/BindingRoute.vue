<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { MappingBinding } from "../types";

const props = defineProps<{ binding: MappingBinding }>();
const emit = defineEmits<{ preview: [target: string | undefined]; toggle: [enabled: boolean] }>();
const { t } = useI18n();

function controlName(control: string) {
  const names: Record<string, string> = {
    "joycon_left.stick_left": "L stick ←",
    "joycon_left.stick_right": "L stick →",
    "joycon_left.dpad_up": "L D-pad ↑",
    "joycon_left.dpad_down": "L D-pad ↓",
    "joycon_right.x": "R X",
    "joycon_right.a": "R A",
  };
  return names[control] ?? control.replace("joycon_", "").replace(".", " · ");
}

function actionName(action: MappingBinding["action"]) {
  const key = {
    window_previous: "previousWindow",
    window_next: "nextWindow",
    focus_codex: "focusCodex",
  }[action];
  return t(`mapping.${key}`);
}

function previewTarget() {
  return props.binding.control.replace(/\.stick_(left|right)$/, ".stick_press");
}
</script>

<template>
  <article
    class="binding-card"
    @mouseenter="emit('preview', previewTarget())"
    @mouseleave="emit('preview', undefined)"
    @focusin="emit('preview', previewTarget())"
    @focusout="emit('preview', undefined)"
  >
    <div class="binding-route">
      <div class="binding-endpoint">
        <span>{{ t("mapping.control") }}</span>
        <p class="binding-control">{{ controlName(props.binding.control) }}</p>
      </div>
      <span class="binding-arrow" aria-hidden="true">→</span>
      <div class="binding-endpoint action-endpoint">
        <span>{{ t("mapping.outcome") }}</span>
        <p class="binding-action">{{ actionName(props.binding.action) }}</p>
      </div>
    </div>
    <label class="switch-control compact">
      <input
        :checked="props.binding.enabled"
        type="checkbox"
        @change="emit('toggle', ($event.target as HTMLInputElement).checked)"
      >
      <span class="switch-track" aria-hidden="true"></span>
    </label>
  </article>
</template>
