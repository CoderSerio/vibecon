<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    side?: "left" | "right";
    activeControls?: string[];
    recentControls?: string[];
    nubTransform?: string;
    previewTarget?: string;
    selectedTarget?: string;
  }>(),
  {
    side: "left",
    activeControls: () => [],
    recentControls: () => [],
    nubTransform: "translate(0, 0)",
  },
);

function controlClass(target: string) {
  return {
    active: props.activeControls.includes(target),
    recent: props.recentControls.includes(target),
    "preview-active": props.previewTarget === target,
    "picker-selected": props.selectedTarget === target,
  };
}

function target(control: string) {
  return `joycon_${props.side}.${control}`;
}
</script>

<template>
  <div
    class="joycon"
    :class="side"
    :aria-label="`Joy-Con (${side === 'left' ? 'L' : 'R'}) visualizer`"
  >
    <div class="rail"></div>
    <template v-if="side === 'left'">
      <span class="control l" :class="controlClass(target('l'))">L</span>
      <button class="control shoulder" :class="controlClass(target('zl'))">
        ZL
      </button>
      <button class="control small sl" :class="controlClass(target('sl'))">
        SL
      </button>
      <button class="control small sr" :class="controlClass(target('sr'))">
        SR
      </button>
      <button class="control minus" :class="controlClass(target('minus'))">
        −
      </button>
      <div
        class="stick"
        :class="controlClass(target('stick_press'))"
        aria-label="Left analogue stick"
      >
        <div class="stick-nub" :style="{ transform: nubTransform }"></div>
      </div>
      <div class="dpad" aria-label="Direction buttons">
        <button
          class="control dpad-button up"
          :class="controlClass(target('dpad_up'))"
        >
          ▲
        </button>
        <button
          class="control dpad-button right"
          :class="controlClass(target('dpad_right'))"
        >
          ▶
        </button>
        <button
          class="control dpad-button down"
          :class="controlClass(target('dpad_down'))"
        >
          ▼
        </button>
        <button
          class="control dpad-button left"
          :class="controlClass(target('dpad_left'))"
        >
          ◀
        </button>
      </div>
      <button class="control capture" :class="controlClass(target('capture'))">
        ●
      </button>
    </template>
    <template v-else>
      <span class="control r" :class="controlClass(target('r'))">R</span>
      <button class="control r-shoulder" :class="controlClass(target('zr'))">
        ZR
      </button>
      <button class="control small r-sl" :class="controlClass(target('sl'))">
        SL
      </button>
      <button class="control small r-sr" :class="controlClass(target('sr'))">
        SR
      </button>
      <button class="control plus" :class="controlClass(target('plus'))">
        +
      </button>
      <div class="abxy" aria-label="ABXY buttons">
        <button
          class="control abxy-button y"
          :class="controlClass(target('y'))"
        >
          Y
        </button>
        <button
          class="control abxy-button a"
          :class="controlClass(target('a'))"
        >
          A
        </button>
        <button
          class="control abxy-button b"
          :class="controlClass(target('b'))"
        >
          B
        </button>
        <button
          class="control abxy-button x"
          :class="controlClass(target('x'))"
        >
          X
        </button>
      </div>
      <div
        class="stick"
        :class="controlClass(target('stick_press'))"
        aria-label="Right analogue stick"
      >
        <div class="stick-nub" :style="{ transform: nubTransform }"></div>
      </div>
      <button class="control home" :class="controlClass(target('home'))">
        ⌂
      </button>
    </template>
  </div>
</template>
