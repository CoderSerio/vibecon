<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    activeControls?: string[];
    nubTransform?: string;
    previewTarget?: string;
    selectedTarget?: string;
  }>(),
  { activeControls: () => [], nubTransform: "translate(0, 0)" },
);

function controlClass(target: string) {
  return {
    active: props.activeControls.includes(target),
    "preview-active": props.previewTarget === target,
    "picker-selected": props.selectedTarget === target,
  };
}
</script>

<template>
  <div class="joycon left" aria-label="Joy-Con (L) visualizer">
    <div class="rail"></div>
    <span class="control l" :class="controlClass('joycon_left.l')">L</span>
    <button class="control shoulder" :class="controlClass('joycon_left.zl')">
      ZL
    </button>
    <button class="control small sl" :class="controlClass('joycon_left.sl')">
      SL
    </button>
    <button class="control small sr" :class="controlClass('joycon_left.sr')">
      SR
    </button>
    <button class="control minus" :class="controlClass('joycon_left.minus')">
      −
    </button>
    <div
      class="stick"
      :class="controlClass('joycon_left.stick_press')"
      aria-label="Left analogue stick"
    >
      <div class="stick-nub" :style="{ transform: nubTransform }"></div>
    </div>
    <div class="dpad" aria-label="Direction buttons">
      <button
        class="control dpad-button up"
        :class="controlClass('joycon_left.dpad_up')"
      >
        ▲
      </button>
      <button
        class="control dpad-button right"
        :class="controlClass('joycon_left.dpad_right')"
      >
        ▶
      </button>
      <button
        class="control dpad-button down"
        :class="controlClass('joycon_left.dpad_down')"
      >
        ▼
      </button>
      <button
        class="control dpad-button left"
        :class="controlClass('joycon_left.dpad_left')"
      >
        ◀
      </button>
    </div>
    <button
      class="control capture"
      :class="controlClass('joycon_left.capture')"
    >
      ●
    </button>
  </div>
</template>
