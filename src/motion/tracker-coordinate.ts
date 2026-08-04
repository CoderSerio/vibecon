export type Vector3Tuple = [number, number, number];
export type QuaternionTuple = [number, number, number, number];

function multiplyQuaternion(
  [ax, ay, az, aw]: QuaternionTuple,
  [bx, by, bz, bw]: QuaternionTuple,
): QuaternionTuple {
  return [
    aw * bx + ax * bw + ay * bz - az * by,
    aw * by - ax * bz + ay * bw + az * bx,
    aw * bz + ax * by - ay * bx + az * bw,
    aw * bw - ax * bx - ay * by - az * bz,
  ];
}

/**
 * Express an absolute body-to-world tracker pose relative to a recentered
 * starting pose. The initial world pose is removed on the right so physical
 * world axes do not rotate with a tilted calibration pose.
 */
export function relativeTrackerQuaternion(
  current: QuaternionTuple,
  zero: QuaternionTuple,
): QuaternionTuple {
  const [zx, zy, zz, zw] = zero;
  const relative = multiplyQuaternion(current, [-zx, -zy, -zz, zw]);
  const length = Math.hypot(...relative);
  return length > 0
    ? relative.map((component) => component / length) as QuaternionTuple
    : [0, 0, 0, 1];
}

/**
 * everything-imu publishes tracker quaternions in a body frame whose model
 * convention is width=X, thickness/front=Y, long/upright=Z. The imported GLB
 * uses front=X, long/upright=Y, width=Z.
 *
 * Keep the tracker quaternion untouched and mount the GLB into that frame:
 * GLB (x, y, z) -> tracker (z, x, y).
 */
export function joyConGlbToTrackerVector(
  [x, y, z]: Vector3Tuple,
): Vector3Tuple {
  return [z, x, y];
}

export const JOYCON_GLB_AXES_IN_TRACKER = {
  x: joyConGlbToTrackerVector([1, 0, 0]),
  y: joyConGlbToTrackerVector([0, 1, 0]),
  z: joyConGlbToTrackerVector([0, 0, 1]),
} as const;
