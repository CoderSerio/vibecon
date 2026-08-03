import assert from "node:assert/strict";
import test from "node:test";
import { MotionPoseTracker } from "./pose.ts";

const sample = (gyroscope) => ({
  acceleration: [0, 0, 4096],
  gyroscope,
});

test("maps sensor X rotation to model X without leaking into Y", () => {
  const tracker = new MotionPoseTracker("right");
  tracker.update(sample([0, 0, 0]), 0);
  tracker.update(sample([0, 0, 0]), 50);
  const pose = tracker.update(sample([164, 0, 0]), 100);

  assert.ok(pose.rotateX > 0.4);
  assert.ok(Math.abs(pose.rotateY) < Number.EPSILON);
});

test("ignores low-amplitude gyro noise inside the raised deadband", () => {
  const tracker = new MotionPoseTracker("right");
  tracker.update(sample([0, 0, 0]), 0);
  tracker.update(sample([0, 0, 0]), 50);
  const pose = tracker.update(sample([30, -30, 30]), 100);

  assert.ok(Math.abs(pose.rotateX) < Number.EPSILON);
  assert.ok(Math.abs(pose.rotateY) < Number.EPSILON);
  assert.ok(Math.abs(pose.rotateZ) < Number.EPSILON);
});
