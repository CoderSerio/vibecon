import assert from "node:assert/strict";
import test from "node:test";
import {
  PointerProjectionTracker,
  projectRotation,
} from "./pointer-projection.ts";

const config = {
  horizontalPixelsPerDegree: 2,
  verticalPixelsPerDegree: 3,
  deadzoneDegrees: 0,
};

test("projects calibrated rotation into an absolute pointer offset", () => {
  assert.deepEqual(
    projectRotation(
      { horizontalDegrees: 30, verticalDegrees: 10 },
      config,
    ),
    { x: 60, y: -30 },
  );
});

test("deadzone removes jitter without introducing a jump at its edge", () => {
  const deadzoneConfig = { ...config, deadzoneDegrees: 0.4 };
  assert.deepEqual(
    projectRotation(
      { horizontalDegrees: 0.3, verticalDegrees: -0.4 },
      deadzoneConfig,
    ),
    { x: 0, y: 0 },
  );
  const justOutside = projectRotation(
    { horizontalDegrees: 0.5, verticalDegrees: 0 },
    deadzoneConfig,
  ).x;
  assert.ok(Math.abs(justOutside - 0.2) < Number.EPSILON);
});

test("tracker emits only the change between consecutive projected offsets", () => {
  const tracker = new PointerProjectionTracker();
  assert.deepEqual(
    tracker.update(
      { horizontalDegrees: 10, verticalDegrees: 5 },
      config,
    ),
    { offset: { x: 20, y: -15 }, delta: { x: 0, y: 0 } },
  );
  assert.deepEqual(
    tracker.update(
      { horizontalDegrees: 12, verticalDegrees: 4 },
      config,
    ),
    { offset: { x: 24, y: -12 }, delta: { x: 4, y: 3 } },
  );
});

test("reset establishes a new baseline without a pointer jump", () => {
  const tracker = new PointerProjectionTracker();
  tracker.update({ horizontalDegrees: 10, verticalDegrees: 5 }, config);
  tracker.update({ horizontalDegrees: 12, verticalDegrees: 4 }, config);
  tracker.reset();
  assert.deepEqual(
    tracker.update(
      { horizontalDegrees: -20, verticalDegrees: 8 },
      config,
    ).delta,
    { x: 0, y: 0 },
  );
});
