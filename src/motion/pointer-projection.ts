export type RotationProjectionConfig = {
  horizontalPixelsPerDegree: number;
  verticalPixelsPerDegree: number;
  deadzoneDegrees: number;
};

export type RotationAngles = {
  horizontalDegrees: number;
  verticalDegrees: number;
};

export type ProjectedPointerOffset = {
  x: number;
  y: number;
};

export type PointerProjectionSample = {
  offset: ProjectedPointerOffset;
  delta: ProjectedPointerOffset;
};

function removeDeadzone(value: number, deadzone: number) {
  const magnitude = Math.abs(value);
  if (magnitude <= deadzone) return 0;
  return Math.sign(value) * (magnitude - deadzone);
}

/**
 * Projects orientation relative to the calibrated zero into a pointer offset.
 * This function has no renderer or OS side effects, so the same mapping can be
 * previewed before a platform-specific mouse driver consumes it.
 */
export function projectRotation(
  angles: RotationAngles,
  config: RotationProjectionConfig,
): ProjectedPointerOffset {
  const horizontal = removeDeadzone(
    angles.horizontalDegrees,
    Math.max(0, config.deadzoneDegrees),
  );
  const vertical = removeDeadzone(
    angles.verticalDegrees,
    Math.max(0, config.deadzoneDegrees),
  );

  const x = horizontal * config.horizontalPixelsPerDegree;
  // Screen coordinates grow downward, while a positive tilt means up.
  const y = -vertical * config.verticalPixelsPerDegree;
  return {
    x: Object.is(x, -0) ? 0 : x,
    y: Object.is(y, -0) ? 0 : y,
  };
}

/**
 * Converts absolute projected offsets into per-report pointer deltas.
 * Platform mouse APIs consume the delta; the UI can still show the offset.
 */
export class PointerProjectionTracker {
  private previous: ProjectedPointerOffset | null = null;

  update(
    angles: RotationAngles,
    config: RotationProjectionConfig,
  ): PointerProjectionSample {
    const offset = projectRotation(angles, config);
    const delta = this.previous
      ? { x: offset.x - this.previous.x, y: offset.y - this.previous.y }
      : { x: 0, y: 0 };
    this.previous = offset;
    return { offset, delta };
  }

  reset() {
    this.previous = null;
  }
}
