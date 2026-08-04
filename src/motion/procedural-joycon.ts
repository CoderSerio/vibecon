import * as THREE from "three";
import { RoundedBoxGeometry } from "three/examples/jsm/geometries/RoundedBoxGeometry.js";

export type JoyConSide = "left" | "right";

/**
 * License-safe fallback used by clean checkouts and release builds.
 *
 * The detailed local GLBs are derived from a third-party reference model and
 * deliberately remain outside Git. This model is assembled entirely from
 * Three.js primitives while preserving the interactive node names expected by
 * ThreeJoyCon.vue.
 */
export function createProceduralJoyCon(side: JoyConSide): THREE.Group {
  const prefix = `SM_JoyCon_${side}_`;
  const root = new THREE.Group();
  root.name = `${prefix}ProceduralRoot`;

  const shell = new THREE.MeshStandardMaterial({
    name: `${prefix}ProceduralShell`,
    color: side === "left" ? "#0AB9E6" : "#FF3C28",
    roughness: 0.34,
    metalness: 0.08,
  });
  const dark = new THREE.MeshStandardMaterial({
    name: `${prefix}ProceduralDark`,
    color: "#111719",
    roughness: 0.48,
    metalness: 0.12,
  });
  const rail = new THREE.MeshStandardMaterial({
    name: `${prefix}ProceduralRail`,
    color: "#20292b",
    roughness: 0.42,
    metalness: 0.34,
  });

  const body = new THREE.Mesh(
    new RoundedBoxGeometry(0.18, 1.24, 0.44, 5, 0.085),
    shell,
  );
  body.name = `${prefix}Body`;
  root.add(body);

  const railSide = side === "left" ? -1 : 1;
  const railBar = new THREE.Mesh(
    new RoundedBoxGeometry(0.13, 1.08, 0.055, 3, 0.018),
    rail,
  );
  railBar.name = `${prefix}Rail`;
  railBar.position.z = railSide * 0.235;
  root.add(railBar);

  const createRoundButton = (
    nodeName: string,
    y: number,
    z: number,
    radius = 0.055,
    depth = 0.035,
  ) => {
    const group = new THREE.Group();
    group.name = `${prefix}${nodeName}`;
    group.position.set(0.105, y, z);
    const face = new THREE.Mesh(
      new THREE.CylinderGeometry(radius, radius, depth, 24),
      dark,
    );
    face.rotation.z = -Math.PI / 2;
    group.add(face);
    root.add(group);
    return group;
  };

  const createBarButton = (nodeName: string, y: number, z: number, vertical = false) => {
    const group = new THREE.Group();
    group.name = `${prefix}${nodeName}`;
    group.position.set(0.108, y, z);
    const horizontal = new THREE.Mesh(
      new RoundedBoxGeometry(0.035, vertical ? 0.09 : 0.026, vertical ? 0.026 : 0.09, 2, 0.009),
      dark,
    );
    group.add(horizontal);
    if (vertical) {
      const cross = new THREE.Mesh(
        new RoundedBoxGeometry(0.035, 0.026, 0.09, 2, 0.009),
        dark,
      );
      group.add(cross);
    }
    root.add(group);
    return group;
  };

  const createStick = (y: number) => {
    const group = new THREE.Group();
    group.name = `${prefix}StickAssembly`;
    group.position.set(0.11, y, 0);
    const stem = new THREE.Mesh(
      new THREE.CylinderGeometry(0.035, 0.045, 0.075, 20),
      dark,
    );
    stem.rotation.z = -Math.PI / 2;
    const cap = new THREE.Mesh(
      new THREE.CylinderGeometry(0.115, 0.105, 0.055, 32),
      dark,
    );
    cap.name = `${prefix}StickCap`;
    cap.rotation.z = -Math.PI / 2;
    cap.position.x = 0.055;
    group.add(stem, cap);
    root.add(group);
  };

  const createRailButton = (nodeName: string, y: number) => {
    const group = new THREE.Group();
    group.name = `${prefix}${nodeName}`;
    group.position.set(0.005, y, railSide * 0.268);
    const face = new THREE.Mesh(
      new RoundedBoxGeometry(0.075, 0.13, 0.035, 2, 0.012),
      shell,
    );
    group.add(face);
    root.add(group);
  };

  const createShoulder = (nodeName: string, y: number, z: number, rear = false) => {
    const group = new THREE.Group();
    group.name = `${prefix}${nodeName}`;
    group.position.set(rear ? -0.055 : 0.02, y, z);
    const face = new THREE.Mesh(
      new RoundedBoxGeometry(0.13, 0.16, 0.21, 3, 0.04),
      dark,
    );
    group.add(face);
    root.add(group);
  };

  if (side === "left") {
    createStick(0.36);
    createRoundButton("DPad_Up", -0.02, 0);
    createRoundButton("DPad_Right", -0.13, -0.11);
    createRoundButton("DPad_Down", -0.24, 0);
    createRoundButton("DPad_Left", -0.13, 0.11);
    createBarButton("Button_Minus", 0.5, -0.13);
    const capture = new THREE.Group();
    capture.name = `${prefix}Button_Capture`;
    capture.position.set(0.108, -0.45, -0.09);
    capture.add(new THREE.Mesh(new RoundedBoxGeometry(0.035, 0.1, 0.1, 2, 0.018), dark));
    root.add(capture);
    createShoulder("Button_L", 0.59, 0.04);
    createShoulder("Button_ZL", 0.61, -0.09, true);
  } else {
    createRoundButton("Button_X", 0.43, 0);
    createRoundButton("Button_A", 0.32, -0.11);
    createRoundButton("Button_B", 0.21, 0);
    createRoundButton("Button_Y", 0.32, 0.11);
    createBarButton("Button_Plus", 0.5, 0.13, true);
    createStick(-0.15);
    createRoundButton("Button_Home", -0.47, 0.1, 0.07);
    createShoulder("Button_R", 0.59, -0.04);
    createShoulder("Button_ZR", 0.61, 0.09, true);
  }

  createRailButton("Button_SL", 0.22);
  createRailButton("Button_SR", -0.23);
  return root;
}
