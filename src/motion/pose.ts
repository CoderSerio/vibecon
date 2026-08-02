import type { ImuSample } from "../types";

export type MotionPose = {
  rotateX: number;
  rotateY: number;
  rotateZ: number;
  hasSample: boolean;
};

const GYRO_COUNTS_PER_DEGREE_PER_SECOND = 16.4;
const MAX_STEP_SECONDS = 0.05;
const CALIBRATION_WINDOW_MS = 450;
const STATIONARY_GYRO_THRESHOLD = 110;
const GYRO_DEADBAND = 10;

const clamp = (value: number, min: number, max: number) =>
  Math.min(Math.max(value, min), max);

const emptyPose = (): MotionPose => ({
  rotateX: 0,
  rotateY: 0,
  rotateZ: 0,
  hasSample: false,
});

/**
 * Stateful MVP orientation estimator.
 *
 * Joy-Con gyro values describe angular velocity, so rendering a raw sample can
 * only show the instant in which the controller moves. This tracker integrates
 * that velocity over time and keeps the resulting pose until another sample
 * changes it. It intentionally keeps the renderer-independent state here so a
 * future Three.js view can consume the same estimator.
 *
 * Linear position is not integrated: consumer IMU acceleration has enough
 * bias that double integration drifts almost immediately. Yaw can also drift
 * slowly because a Joy-Con has no absolute heading reference.
 */
export class MotionPoseTracker {
  private orientation = { x: 0, y: 0, z: 0 };
  private zero = { x: 0, y: 0, z: 0 };
  private gyroBias: [number, number, number] = [0, 0, 0];
  private lastGyroscope: [number, number, number] = [0, 0, 0];
  private lastUpdatedAt: number | null = null;
  private calibrationUntil = 0;
  private calibrationSum: [number, number, number] = [0, 0, 0];
  private calibrationCount = 0;
  private gravityInitialized = false;
  private pose = emptyPose();

  constructor(private readonly side: "left" | "right") {}

  update(sample: ImuSample | null, now = performance.now()): MotionPose {
    if (!sample) {
      this.lastUpdatedAt = null;
      this.pose = emptyPose();
      return this.pose;
    }

    if (this.lastUpdatedAt === null) {
      this.lastUpdatedAt = now;
      this.pose = { ...this.pose, hasSample: true };
      return this.pose;
    }

    const elapsed = clamp((now - this.lastUpdatedAt) / 1000, 0, MAX_STEP_SECONDS);
    this.lastUpdatedAt = now;
    const [ax] = sample.acceleration;
    const [gx, gy, gz] = sample.gyroscope;
    this.lastGyroscope = [gx, gy, gz];
    if (this.calibrationUntil > 0) {
      this.calibrationSum[0] += gx;
      this.calibrationSum[1] += gy;
      this.calibrationSum[2] += gz;
      this.calibrationCount += 1;
      if (now >= this.calibrationUntil && this.calibrationCount > 0) {
        this.gyroBias = this.calibrationSum.map((value) => value / this.calibrationCount) as [number, number, number];
        this.calibrationUntil = 0;
        this.calibrationSum = [0, 0, 0];
        this.calibrationCount = 0;
        this.zero = { ...this.orientation };
      }
      this.lastUpdatedAt = now;
      this.pose = { ...this.pose, rotateX: 0, rotateY: 0, rotateZ: 0, hasSample: true };
      return this.pose;
    }
    const [biasX, biasY, biasZ] = this.gyroBias;
    const mirrored = this.side === "left" ? -1 : 1;

    // A resting Joy-Con still reports a small gyro offset. Learn it slowly
    // whenever all axes are below a conservative stationary threshold, then
    // apply a deadband so sensor noise is not integrated into visible motion.
    if (Math.hypot(gx - biasX, gy - biasY, gz - biasZ) < STATIONARY_GYRO_THRESHOLD) {
      const learningRate = 0.025;
      this.gyroBias = [
        biasX + (gx - biasX) * learningRate,
        biasY + (gy - biasY) * learningRate,
        biasZ + (gz - biasZ) * learningRate,
      ];
    }
    const corrected = [
      Math.abs(gx - this.gyroBias[0]) < GYRO_DEADBAND ? 0 : gx - this.gyroBias[0],
      Math.abs(gy - this.gyroBias[1]) < GYRO_DEADBAND ? 0 : gy - this.gyroBias[1],
      Math.abs(gz - this.gyroBias[2]) < GYRO_DEADBAND ? 0 : gz - this.gyroBias[2],
    ];

    // Preserve the axes already verified by the direct preview, but integrate
    // degrees/second instead of treating velocity as an absolute angle.
    this.orientation.x += (corrected[1] / GYRO_COUNTS_PER_DEGREE_PER_SECOND) * elapsed;
    this.orientation.y += (corrected[0] * mirrored / GYRO_COUNTS_PER_DEGREE_PER_SECOND) * elapsed;
    this.orientation.z += (corrected[2] * mirrored / GYRO_COUNTS_PER_DEGREE_PER_SECOND) * elapsed;

    // Accelerometer gravity gives an absolute reference for two axes. Blend
    // it in gently so movement remains gyro-smooth while long-term tilt drift
    // is corrected. It cannot correct yaw, which has no absolute reference.
    const gravityX = Math.atan2(ax, Math.hypot(sample.acceleration[1], sample.acceleration[2])) * 180 / Math.PI;
    const gravityY = Math.atan2(sample.acceleration[1], sample.acceleration[2]) * 180 / Math.PI * mirrored;
    if (!this.gravityInitialized) {
      this.orientation.x = gravityX;
      this.orientation.y = gravityY;
      this.gravityInitialized = true;
    } else {
      this.orientation.x += (gravityX - this.orientation.x) * 0.018;
      this.orientation.y += (gravityY - this.orientation.y) * 0.018;
    }

    this.pose = {
      rotateX: this.orientation.x - this.zero.x,
      rotateY: this.orientation.y - this.zero.y,
      rotateZ: this.orientation.z - this.zero.z,
      hasSample: true,
    };
    return this.pose;
  }

  calibrate(): MotionPose {
    // Collect a short stillness window instead of treating one noisy report as
    // the bias. The user should hold the Joy-Con still until this completes.
    this.calibrationUntil = performance.now() + CALIBRATION_WINDOW_MS;
    this.calibrationSum = [...this.lastGyroscope];
    this.calibrationCount = 1;
    this.zero = { ...this.orientation };
    this.pose = {
      ...this.pose,
      rotateX: 0,
      rotateY: 0,
      rotateZ: 0,
    };
    return this.pose;
  }

  /** Re-zero the rendered orientation without changing the sensor bias. */
  resetUpright(): MotionPose {
    this.zero = { ...this.orientation };
    this.pose = {
      ...this.pose,
      rotateX: 0,
      rotateY: 0,
      rotateZ: 0,
    };
    return this.pose;
  }
}
