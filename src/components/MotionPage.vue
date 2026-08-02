<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import MotionJoyCon from "./MotionJoyCon.vue";
import type { ImuSample } from "../types";

defineProps<{ leftImu: ImuSample | null; rightImu: ImuSample | null }>();
const { t } = useI18n();
const SENSITIVITY_STORAGE_KEY = "vibecon.motion.pointerSensitivity";
const storedSensitivity = Number(localStorage.getItem(SENSITIVITY_STORAGE_KEY));
const sensitivity = ref(
  Number.isFinite(storedSensitivity) && storedSensitivity >= 1 && storedSensitivity <= 30
    ? storedSensitivity
    : 8,
);
watch(sensitivity, (value) => {
  localStorage.setItem(SENSITIVITY_STORAGE_KEY, String(value));
});
</script>

<template>
  <section class="panel motion-panel">
    <div class="section-heading">
      <div>
        <p class="eyebrow">{{ t("motion.eyebrow") }}</p>
        <h2 class="section-title">{{ t("motion.title") }}</h2>
        <p class="hint">{{ t("motion.subtitle") }}</p>
      </div>
      <div class="motion-toolbar">
        <label class="motion-sensitivity">
          <span>{{ t("motion.sensitivity") }}</span>
          <input v-model.number="sensitivity" type="range" min="1" max="30" step="1" />
          <output>{{ sensitivity }} px/°</output>
        </label>
        <span class="motion-safe">{{ t("motion.safe") }}</span>
      </div>
    </div>
    <div class="motion-devices">
      <MotionJoyCon side="left" :imu="leftImu" :sensitivity="sensitivity" />
      <MotionJoyCon side="right" :imu="rightImu" :sensitivity="sensitivity" />
    </div>
    <p v-if="!leftImu && !rightImu" class="motion-empty">{{ t("motion.waiting") }}</p>
  </section>
</template>
