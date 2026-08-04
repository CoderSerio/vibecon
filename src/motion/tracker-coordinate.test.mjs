import assert from "node:assert/strict";
import test from "node:test";
import {
  JOYCON_GLB_AXES_IN_TRACKER,
  joyConGlbToTrackerVector,
  relativeTrackerQuaternion,
} from "./tracker-coordinate.ts";

test("mounts the GLB long axis onto tracker upright Z", () => {
  assert.deepEqual(joyConGlbToTrackerVector([0, 1, 0]), [0, 0, 1]);
});

test("mounts the GLB front normal onto tracker thickness Y", () => {
  assert.deepEqual(joyConGlbToTrackerVector([1, 0, 0]), [0, 1, 0]);
});

test("GLB mount basis stays right handed", () => {
  const [xx, xy, xz] = JOYCON_GLB_AXES_IN_TRACKER.x;
  const [yx, yy, yz] = JOYCON_GLB_AXES_IN_TRACKER.y;
  const cross = [xy * yz - xz * yy, xz * yx - xx * yz, xx * yy - xy * yx];
  assert.deepEqual(cross, JOYCON_GLB_AXES_IN_TRACKER.z);
});

test("recentring a portrait pose preserves a world Y rotation", () => {
  const halfQuarterTurn = Math.PI / 4;
  const zero = [Math.sin(halfQuarterTurn), 0, 0, Math.cos(halfQuarterTurn)];
  const halfMotion = Math.PI / 18;
  const worldYMotion = [0, Math.sin(halfMotion), 0, Math.cos(halfMotion)];

  // The absolute current body-to-world pose is motion * initial pose.
  const [mx, my, mz, mw] = worldYMotion;
  const [zx, zy, zz, zw] = zero;
  const current = [
    mw * zx + mx * zw + my * zz - mz * zy,
    mw * zy - mx * zz + my * zw + mz * zx,
    mw * zz + mx * zy - my * zx + mz * zw,
    mw * zw - mx * zx - my * zy - mz * zz,
  ];

  const relative = relativeTrackerQuaternion(current, zero);
  assert.ok(Math.abs(relative[0]) < 1e-9);
  assert.ok(Math.abs(relative[2]) < 1e-9);
  assert.ok(Math.abs(relative[1] - worldYMotion[1]) < 1e-9);
  assert.ok(Math.abs(relative[3] - worldYMotion[3]) < 1e-9);
});
