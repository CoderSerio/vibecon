<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { ImuSample } from "../types";
import { MotionPoseTracker, type MotionPose } from "../motion/pose";
import {
  PointerProjectionTracker,
  type PointerProjectionSample,
} from "../motion/pointer-projection";

const props = defineProps<{
  side: "left" | "right";
  imu: ImuSample | null;
  sensitivity: number;
}>();
const { t } = useI18n();
const tracker = new MotionPoseTracker(props.side);
const pointerTracker = new PointerProjectionTracker();
const pose = ref<MotionPose>(tracker.update(null));
const pointerProjection = ref<PointerProjectionSample>({
  offset: { x: 0, y: 0 },
  delta: { x: 0, y: 0 },
});
function projectionConfig() {
  return {
    horizontalPixelsPerDegree: props.sensitivity,
    verticalPixelsPerDegree: props.sensitivity,
    deadzoneDegrees: 0.4,
  };
}
function projectionAngles(nextPose: MotionPose) {
  return {
    horizontalDegrees: nextPose.rotateY,
    verticalDegrees: nextPose.rotateX,
  };
}
watch(
  () => props.imu,
  (sample) => {
    pose.value = tracker.update(sample);
    pointerProjection.value = pointerTracker.update(
      projectionAngles(pose.value),
      projectionConfig(),
    );
  },
  { immediate: true },
);
watch(
  () => props.sensitivity,
  () => {
    pointerTracker.reset();
    pointerProjection.value = pointerTracker.update(
      projectionAngles(pose.value),
      projectionConfig(),
    );
  },
);
const transform = computed(() => `rotateX(${pose.value.rotateX}deg) rotateY(${pose.value.rotateY}deg) rotateZ(${pose.value.rotateZ}deg)`);
function calibrate() {
  pose.value = tracker.calibrate();
  pointerTracker.reset();
  pointerProjection.value = pointerTracker.update(
    projectionAngles(pose.value),
    projectionConfig(),
  );
}
function resetUpright() {
  pose.value = tracker.resetUpright();
  pointerTracker.reset();
  pointerProjection.value = pointerTracker.update(
    projectionAngles(pose.value),
    projectionConfig(),
  );
}
</script>

<template>
  <article class="motion-device" :class="props.side">
    <header>
      <span class="motion-side">{{ props.side === "left" ? "JOY-CON (L)" : "JOY-CON (R)" }}</span>
      <div class="motion-device-actions">
        <span class="motion-state" :class="{ live: pose.hasSample }">{{ pose.hasSample ? "0x30 IMU LIVE" : "WAITING FOR 0x30" }}</span>
        <button class="motion-calibrate" type="button" :disabled="!pose.hasSample" @click="calibrate">{{ t("motion.calibrate") }}</button>
        <button class="motion-calibrate" type="button" :disabled="!pose.hasSample" @click="resetUpright">{{ t("motion.reset") }}</button>
      </div>
    </header>
    <div class="motion-stage">
      <div class="motion-cube" :style="{ transform }">
        <div class="motion-face front"><span></span><i></i></div>
        <div class="motion-face side"></div>
        <div class="motion-face top"></div>
      </div>
    </div>
    <dl class="motion-readout">
      <div><dt>gyro</dt><dd>{{ props.imu ? props.imu.gyroscope.join(" · ") : "—" }}</dd></div>
      <div><dt>accel</dt><dd>{{ props.imu ? props.imu.acceleration.join(" · ") : "—" }}</dd></div>
      <div><dt>angle</dt><dd>H {{ pose.rotateY.toFixed(1) }}° · V {{ pose.rotateX.toFixed(1) }}°</dd></div>
      <div><dt>pointer</dt><dd>x {{ pointerProjection.offset.x.toFixed(0) }} px · y {{ pointerProjection.offset.y.toFixed(0) }} px</dd></div>
      <div><dt>delta</dt><dd>dx {{ pointerProjection.delta.x.toFixed(1) }} · dy {{ pointerProjection.delta.y.toFixed(1) }}</dd></div>
    </dl>
  </article>
</template>
