<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import type { ImuSample, Stick } from "../types";
import { MotionPoseTracker } from "../motion/pose";

const props = defineProps<{
  side: "left" | "right";
  imu: ImuSample | null;
  stick: Stick | null;
  activeControls: string[];
  followMotion: boolean;
  resetKey: number;
  inspectionView: "front" | "rail" | "shoulder";
}>();
const { t } = useI18n();

const host = ref<HTMLDivElement | null>(null);
const status = ref(t("debug.threeLoading"));
const diagnostics = ref("render —");
const modelUrl = props.side === "left"
  ? new URL("../assets/models/joycon-left.interactive.glb", import.meta.url).href
  : new URL("../assets/models/joycon-right.interactive.glb", import.meta.url).href;
const tracker = new MotionPoseTracker(props.side);

let renderer: THREE.WebGLRenderer | undefined;
let scene: THREE.Scene | undefined;
let camera: THREE.PerspectiveCamera | undefined;
let joycon: THREE.Group | undefined;
let modelDiameter = 1;
let disposed = false;
// After glTF's Y-up conversion the full-switch export already has its long
// axis on Y and its front normal toward +X, matching the shared camera.
const baseModelRotation = new THREE.Euler(0, 0, 0);
let resizeObserver: ResizeObserver | undefined;
let frameId = 0;
const nodes = new Map<string, THREE.Object3D>();
const materialsByName = new Map<string, THREE.MeshStandardMaterial[]>();
const basePositions = new Map<string, THREE.Vector3>();
const baseRotations = new Map<string, THREE.Euler>();
const baseMaterialEmission = new Map<
  THREE.MeshStandardMaterial,
  { baseColor: THREE.Color; emissive: THREE.Color; intensity: number }
>();
const highlightLocalTargets = new Map<string, THREE.Vector3>();
const surfaceHighlightShaders = new Map<THREE.Material, THREE.Shader>();
let highlightScale = 1;
let highlightFrontX = 0;
// One side has at most eleven distinct controls. Keeping enough uniforms for
// all of them means combined presses are never silently truncated.
const maxSurfaceHighlights = 11;
const stickTiltRadians = THREE.MathUtils.degToRad(21);
const highlightDefinitions = highlightSpecs();
const activeSurfaceTargets = Array.from(
  { length: maxSurfaceHighlights },
  () => new THREE.Vector4(),
);
const highlightWorldScratch = new THREE.Vector3();
let diagnosticRenderPeak = 0;
let activeSurfaceHighlightCount = 0;

function control(name: string) {
  return `joycon_${props.side}.${name}`;
}

function isPressed(name: string) {
  return props.activeControls.includes(control(name));
}

function findNode(name: string) {
  return nodes.get(`SM_JoyCon_${props.side}_${name}`);
}

function findStickAssembly() {
  return findNode("StickAssembly") ?? findNode("StickCap");
}

function findInteractiveControlNode(name: string) {
  if (name === "stick_press") return findStickAssembly();
  if (name.startsWith("dpad_")) {
    const direction = name.slice("dpad_".length);
    return findNode(`DPad_${direction[0].toUpperCase()}${direction.slice(1)}`);
  }
  return findNode(`Button_${name.toUpperCase()}`);
}

function dedicatedControlMaterials(name: string) {
  const materialName = props.side === "left"
    ? {
        sr: "MAT_FullJoyCon_L_Button_SR",
      }[name]
    : undefined;
  return materialName ? materialsByName.get(materialName) ?? [] : [];
}

function applyMaterialHighlight(materials: THREE.MeshStandardMaterial[], pressed: boolean) {
  for (const standard of materials) {
    const original = baseMaterialEmission.get(standard);
    if (pressed) {
      standard.color.set("#52ffc2");
      standard.emissive.set("#20d99a");
      standard.emissiveIntensity = 0.85;
    } else if (original) {
      standard.color.copy(original.baseColor);
      standard.emissive.copy(original.emissive);
      standard.emissiveIntensity = original.intensity;
    }
  }
}

function restoreNode(node: THREE.Object3D) {
  const base = basePositions.get(node.name);
  const rotation = baseRotations.get(node.name);
  if (base) node.position.copy(base);
  if (rotation) node.rotation.copy(rotation);
}

function applyButton(name: string, pressed: boolean) {
  const controlName = name.replace(/^Button_/, "").toLowerCase();
  const dedicatedMaterials = dedicatedControlMaterials(controlName);
  if (dedicatedMaterials.length) {
    applyMaterialHighlight(dedicatedMaterials, pressed);
    return;
  }
  const node = findNode(name);
  if (!node) return;
  const isShoulderButton = ["Button_L", "Button_ZL", "Button_R", "Button_ZR"].includes(name);
  restoreNode(node);
  if (pressed) {
    if (["Button_SL", "Button_SR", "Button_L", "Button_ZL", "Button_R", "Button_ZR"].includes(name)) {
      // The rail parts use a different exported local basis from the front
      // buttons. Keep them seated while highlighted until their inward press
      // axis is calibrated per model; moving them on the front-button axis
      // visibly pulls the mesh away from its opening.
    } else {
      // The exported model's front normal is local +X.
      node.position.x -= 0.006;
    }
  }
  node.traverse((child) => {
    if (!(child instanceof THREE.Mesh)) return;
    const materials = Array.isArray(child.material) ? child.material : [child.material];
    for (const material of materials) {
      const standard = material as THREE.MeshStandardMaterial;
      if (!standard.emissive) continue;
      const original = baseMaterialEmission.get(standard);
      if (pressed) {
        standard.color.set("#52ffc2");
        standard.emissive.set("#20d99a");
        standard.emissiveIntensity = 0.85;
      } else if (original) {
        if (isShoulderButton) {
          // These faces were split from a blue body material even though the
          // source texture depicts black shoulder controls. Give the isolated
          // nodes their intended resting tint without changing the shell.
          standard.color.set("#161a1c");
          standard.emissive.set("#000000");
          standard.emissiveIntensity = 0;
        } else {
          standard.color.copy(original.baseColor);
          standard.emissive.copy(original.emissive);
          standard.emissiveIntensity = original.intensity;
        }
      }
    }
  });
}

type HighlightSpec = {
  name: string;
  x?: number;
  y: number;
  z: number;
  radius: number;
};

function highlightSpecs(): HighlightSpec[] {
  if (props.side === "left") {
    return [
      { name: "stick_press", y: 0.47, z: 0, radius: 0.12 },
      { name: "dpad_up", y: 0.06, z: 0, radius: 0.055 },
      { name: "dpad_right", y: -0.04, z: -0.11, radius: 0.055 },
      { name: "dpad_down", y: -0.14, z: 0, radius: 0.055 },
      { name: "dpad_left", y: -0.04, z: 0.11, radius: 0.055 },
      { name: "minus", y: 0.52, z: -0.14, radius: 0.045 },
      { name: "capture", y: -0.28, z: -0.08, radius: 0.050 },
      { name: "l", x: 0, y: 0.59, z: 0, radius: 0.035 },
      { name: "zl", x: -0.08, y: 0.62, z: -0.10, radius: 0.045 },
      { name: "sl", x: 0, y: 0.31, z: -0.21, radius: 0.026 },
      { name: "sr", x: 0, y: -0.24, z: -0.21, radius: 0.026 },
    ];
  }
  return [
    { name: "plus", y: 0.51, z: 0.14, radius: 0.045 },
    { name: "x", y: 0.43, z: 0, radius: 0.050 },
    { name: "a", y: 0.34, z: -0.11, radius: 0.055 },
    { name: "b", y: 0.21, z: 0, radius: 0.055 },
    { name: "y", y: 0.34, z: 0.11, radius: 0.055 },
    { name: "stick_press", y: -0.14, z: 0, radius: 0.12 },
    { name: "home", y: -0.44, z: 0.14, radius: 0.050 },
    { name: "r", x: 0, y: 0.59, z: 0, radius: 0.035 },
    { name: "zr", x: -0.08, y: 0.62, z: 0.10, radius: 0.045 },
    { name: "sr", x: 0, y: 0.31, z: 0.21, radius: 0.026 },
    { name: "sl", x: 0, y: -0.24, z: 0.21, radius: 0.026 },
  ];
}

function enableSurfaceHighlight(material: THREE.Material) {
  if (!(material instanceof THREE.MeshStandardMaterial)) return;
  material.onBeforeCompile = (shader) => {
    shader.uniforms.uVibeHighlightCount = { value: 0 };
    shader.uniforms.uVibeHighlightTargets = {
      value: Array.from({ length: maxSurfaceHighlights }, () => new THREE.Vector4()),
    };
    shader.uniforms.uVibeHighlightColor = { value: new THREE.Color("#7de6c4") };
    shader.uniforms.uVibeHighlightIntensity = { value: 1.15 };
    shader.vertexShader = shader.vertexShader
      .replace(
        "#include <common>",
        "#include <common>\nvarying vec3 vVibeWorldPosition;",
      )
      .replace(
        "#include <worldpos_vertex>",
        "#include <worldpos_vertex>\nvVibeWorldPosition = (modelMatrix * vec4(transformed, 1.0)).xyz;",
      );
    shader.fragmentShader = shader.fragmentShader
      .replace(
        "#include <common>",
        `#include <common>
varying vec3 vVibeWorldPosition;
uniform int uVibeHighlightCount;
uniform vec4 uVibeHighlightTargets[${maxSurfaceHighlights}];
uniform vec3 uVibeHighlightColor;
uniform float uVibeHighlightIntensity;`,
      )
      .replace(
        "#include <emissivemap_fragment>",
        `#include <emissivemap_fragment>
float vibeHighlight = 0.0;
for (int i = 0; i < ${maxSurfaceHighlights}; i++) {
  if (i >= uVibeHighlightCount) break;
  vec4 target = uVibeHighlightTargets[i];
  float distanceToTarget = distance(vVibeWorldPosition, target.xyz);
  vibeHighlight = max(
    vibeHighlight,
    1.0 - smoothstep(target.w * 0.62, target.w, distanceToTarget)
  );
}
totalEmissiveRadiance += uVibeHighlightColor * vibeHighlight * uVibeHighlightIntensity;`,
      );
    // Recompilation replaces the previous program for this material instead
    // of retaining every historical shader object.
    surfaceHighlightShaders.set(material, shader);
  };
  material.customProgramCacheKey = () => "vibecon-surface-highlight-v1";
  material.needsUpdate = true;
}

function createSurfaceHighlightTargets(modelSize: THREE.Vector3) {
  if (!joycon) return;
  // The specs are expressed in the source FBX's Joy-Con units. The imported
  // model is uniformly scaled for the preview, so convert them to centered
  // world coordinates first, then back into the GLTF root's local space. This
  // also accounts for the source node's translation and mirrored scale.
  highlightScale = modelSize.y / 1.279;
  highlightFrontX = modelSize.x / 2 + 0.018;
  joycon.updateMatrixWorld(true);
  const toRootLocal = (y: number, z: number, x?: number) =>
    joycon!.worldToLocal(new THREE.Vector3(
      x === undefined ? highlightFrontX : x * highlightScale,
      y * highlightScale,
      z * highlightScale,
    ));
  for (const spec of highlightDefinitions) {
    highlightLocalTargets.set(control(spec.name), toRootLocal(spec.y, spec.z, spec.x));
  }
}

function updateSurfaceHighlights() {
  if (!joycon) return;
  let activeCount = 0;

  joycon.updateMatrixWorld(true);
  for (const spec of highlightDefinitions) {
    const name = control(spec.name);
    const pressed = isPressed(spec.name);
    if (!pressed) continue;
    // Once a control has a real Blender mesh, let that mesh own its highlight;
    // the surface mask is only the fallback for still-welded controls.
    const interactiveNode = findInteractiveControlNode(spec.name);
    if (interactiveNode || dedicatedControlMaterials(spec.name).length) continue;
    const local = highlightLocalTargets.get(name);
    if (!local) continue;
    if (activeCount >= maxSurfaceHighlights) break;
    joycon.localToWorld(highlightWorldScratch.copy(local));
    activeSurfaceTargets[activeCount].set(
      highlightWorldScratch.x,
      highlightWorldScratch.y,
      highlightWorldScratch.z,
      // Specs describe the visible planar button radius. The sphere also has
      // to reach through the welded shell to its slightly raised surface.
      spec.radius * highlightScale * 2,
    );
    activeCount += 1;
  }

  for (const shader of surfaceHighlightShaders.values()) {
    const targets = shader.uniforms.uVibeHighlightTargets.value as THREE.Vector4[];
    shader.uniforms.uVibeHighlightCount.value = activeCount;
    for (let index = 0; index < maxSurfaceHighlights; index += 1) {
      if (index < activeCount) {
        targets[index].copy(activeSurfaceTargets[index]);
      } else {
        targets[index].set(0, 0, 0, 0);
      }
    }
  }
  activeSurfaceHighlightCount = activeCount;
}

function updateInputAnimation() {
  if (!joycon) return;
  updateSurfaceHighlights();
  const stick = props.stick;
  const stickAssembly = findStickAssembly();
  if (stickAssembly) {
    restoreNode(stickAssembly);
    const x = Math.max(-1, Math.min(1, stick?.normalized_x ?? 0));
    const y = Math.max(-1, Math.min(1, stick?.normalized_y ?? 0));
    const pressed = isPressed("stick_press");
    // The cap's local X axis is its front normal. Rotate around Z for the
    // screen-horizontal axis and around Y for screen-vertical tilt.
    stickAssembly.rotation.z += x * (props.side === "left" ? -stickTiltRadians : stickTiltRadians);
    stickAssembly.rotation.y += y * stickTiltRadians;
    // Stick click moves inward along the now-tilted stem axis.
    if (pressed) stickAssembly.translateX(-0.007);
    stickAssembly.traverse((child) => {
      if (!(child instanceof THREE.Mesh)) return;
      const materials = Array.isArray(child.material) ? child.material : [child.material];
      for (const material of materials) {
        const standard = material as THREE.MeshStandardMaterial;
        if (standard.emissive) {
          const original = baseMaterialEmission.get(standard);
          if (pressed) {
            standard.emissive.set("#7de6c4");
            standard.emissiveIntensity = 1.15;
          } else if (original) {
            standard.color.copy(original.baseColor);
            standard.emissive.copy(original.emissive);
            standard.emissiveIntensity = original.intensity;
          }
        }
      }
    });
  }

  const bindings = props.side === "right"
    ? [
        ["Button_A", "a"], ["Button_B", "b"], ["Button_X", "x"], ["Button_Y", "y"],
        ["Button_Plus", "plus"], ["Button_Home", "home"], ["Button_R", "r"],
        ["Button_ZR", "zr"], ["Button_SL", "sl"], ["Button_SR", "sr"],
      ]
    : [
        ["DPad_Up", "dpad_up"], ["DPad_Down", "dpad_down"],
        ["DPad_Left", "dpad_left"], ["DPad_Right", "dpad_right"],
        ["Button_Minus", "minus"], ["Button_Capture", "capture"], ["Button_L", "l"],
        ["Button_ZL", "zl"], ["Button_SL", "sl"], ["Button_SR", "sr"],
      ];
  for (const [nodeName, controlName] of bindings) {
    applyButton(nodeName, isPressed(controlName));
  }
}

function renderOnce() {
  if (!renderer || !scene || !camera) return;
  const startedAt = performance.now();
  updateInputAnimation();
  renderer.render(scene, camera);
  const elapsed = performance.now() - startedAt;
  diagnosticRenderPeak = Math.max(diagnosticRenderPeak, elapsed);
  const memory = renderer.info.memory;
  diagnostics.value = `${elapsed.toFixed(1)}ms last · ${diagnosticRenderPeak.toFixed(1)}ms peak · T${memory.textures} G${memory.geometries} P${renderer.info.programs?.length ?? 0} S${surfaceHighlightShaders.size} H${activeSurfaceHighlightCount}`;
}

function requestRender() {
  if (frameId) return;
  frameId = requestAnimationFrame(() => {
    frameId = 0;
    renderOnce();
  });
}

function resize() {
  if (!renderer || !camera || !host.value) return;
  const { width, height } = host.value.getBoundingClientRect();
  if (!width || !height) return;
  renderer.setSize(width, height, false);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  camera.aspect = width / height;
  camera.updateProjectionMatrix();
  requestRender();
}

function applyInspectionView() {
  if (!camera) return;
  const distance = modelDiameter * 2.35;
  if (props.inspectionView === "rail") {
    camera.position.set(0, 0, props.side === "left" ? -distance : distance);
  } else if (props.inspectionView === "shoulder") {
    camera.position.set(distance * 0.72, distance * 0.72, 0);
  } else {
    camera.position.set(distance, 0, 0);
  }
  camera.up.set(0, 1, 0);
  camera.lookAt(0, 0, 0);
  requestRender();
}

function disposeObjectTree(root: THREE.Object3D) {
  const geometries = new Set<THREE.BufferGeometry>();
  const materials = new Set<THREE.Material>();
  const textures = new Set<THREE.Texture>();

  root.traverse((node) => {
    if (!(node instanceof THREE.Mesh)) return;
    geometries.add(node.geometry);
    const nodeMaterials = Array.isArray(node.material) ? node.material : [node.material];
    for (const material of nodeMaterials) materials.add(material);
  });

  for (const material of materials) {
    for (const value of Object.values(material as unknown as Record<string, unknown>)) {
      if (value instanceof THREE.Texture) textures.add(value);
    }
  }
  for (const texture of textures) texture.dispose();
  for (const material of materials) material.dispose();
  for (const geometry of geometries) geometry.dispose();
  root.removeFromParent();
}

watch(
  () => [props.stick?.normalized_x, props.stick?.normalized_y, props.activeControls.join("|")],
  requestRender,
);

watch(
  () => props.imu,
  (sample) => {
    const pose = tracker.update(sample);
    if (!joycon) return;
    if (!props.followMotion) {
      tracker.resetUpright();
      joycon.rotation.copy(baseModelRotation);
      return;
    }
    joycon.rotation.set(
      baseModelRotation.x + THREE.MathUtils.degToRad(pose.rotateX),
      baseModelRotation.y + THREE.MathUtils.degToRad(pose.rotateY),
      baseModelRotation.z + THREE.MathUtils.degToRad(pose.rotateZ),
    );
    requestRender();
  },
  { immediate: true },
);

watch(
  () => props.followMotion,
  (enabled) => {
    if (enabled) return;
    tracker.resetUpright();
    joycon?.rotation.copy(baseModelRotation);
    requestRender();
  },
);

watch(
  () => props.resetKey,
  () => {
    tracker.resetUpright();
    joycon?.rotation.copy(baseModelRotation);
    requestRender();
  },
);

watch(() => props.inspectionView, applyInspectionView);

onMounted(() => {
  if (!host.value) return;
  disposed = false;
  scene = new THREE.Scene();
  scene.background = new THREE.Color("#101d1b");
  camera = new THREE.PerspectiveCamera(35, 1, 0.01, 100);
  camera.position.set(2.15, 0, 0);
  camera.lookAt(0, 0, 0);

  renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.2;
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFShadowMap;
  host.value.appendChild(renderer.domElement);

  scene.add(new THREE.AmbientLight("#ffffff", 0.75));
  scene.add(new THREE.HemisphereLight("#e8fff9", "#17332d", 1.9));
  const key = new THREE.DirectionalLight("#ffffff", 3.8);
  key.position.set(4, 2, 2);
  key.castShadow = true;
  scene.add(key);
  const rim = new THREE.DirectionalLight(props.side === "left" ? "#51d9ff" : "#ff8a7e", 1.8);
  rim.position.set(-3, 1, -2);
  scene.add(rim);

  new GLTFLoader().load(
    modelUrl,
    (gltf) => {
      if (disposed) {
        disposeObjectTree(gltf.scene);
        return;
      }
      joycon = gltf.scene;
      joycon.scale.setScalar(2.9);
      joycon.rotation.copy(baseModelRotation);
      joycon.traverse((node) => {
        nodes.set(node.name, node);
        basePositions.set(node.name, node.position.clone());
        baseRotations.set(node.name, node.rotation.clone());
        if (node instanceof THREE.Mesh) {
          // Keep the material's original shape. Assigning `[material]` to a
          // single-material mesh without geometry groups makes Three skip the
          // draw call entirely, which looks like a successfully loaded but
          // empty canvas.
          node.material = Array.isArray(node.material)
            ? node.material.map((material) => material.clone())
            : node.material.clone();
          const materials = Array.isArray(node.material) ? node.material : [node.material];
          for (const material of materials) {
            if (material instanceof THREE.MeshStandardMaterial) {
              baseMaterialEmission.set(material, {
                baseColor: material.color.clone(),
                emissive: material.emissive.clone(),
                intensity: material.emissiveIntensity,
              });
              const registered = materialsByName.get(material.name) ?? [];
              registered.push(material);
              materialsByName.set(material.name, registered);
            }
            enableSurfaceHighlight(material);
          }
          node.castShadow = true;
          node.receiveShadow = true;
        }
      });
      // Normalize both mirrored GLBs into the same camera framing. The left
      // asset contains a negative mirror scale, so relying on a fixed camera
      // distance makes that side appear clipped even when its bounds match.
      const bounds = new THREE.Box3().setFromObject(joycon);
      const center = bounds.getCenter(new THREE.Vector3());
      const size = bounds.getSize(new THREE.Vector3());
      joycon.position.sub(center);
      modelDiameter = Math.max(size.x, size.y, size.z);
      applyInspectionView();
      createSurfaceHighlightTargets(size);
      scene?.add(joycon);
      status.value = t("debug.threeLive");
      // The first render compiles the injected shader; the second applies its
      // initial uniform values without starting a permanent animation loop.
      renderOnce();
      requestRender();
    },
    undefined,
    () => {
      status.value = t("debug.threeUnavailable");
    },
  );

  resizeObserver = new ResizeObserver(resize);
  resizeObserver.observe(host.value);
  resize();
});

onBeforeUnmount(() => {
  disposed = true;
  cancelAnimationFrame(frameId);
  resizeObserver?.disconnect();
  if (joycon) disposeObjectTree(joycon);
  renderer?.renderLists.dispose();
  renderer?.dispose();
  renderer?.forceContextLoss();
  renderer?.domElement.remove();
  nodes.clear();
  basePositions.clear();
  baseRotations.clear();
  baseMaterialEmission.clear();
  highlightLocalTargets.clear();
  surfaceHighlightShaders.clear();
  scene?.clear();
  joycon = undefined;
  scene = undefined;
  camera = undefined;
  renderer = undefined;
});
</script>

<template>
  <article class="three-joycon" :class="side">
    <header>
      <strong>JOY-CON ({{ side === "left" ? "L" : "R" }}) · 3D</strong>
      <span :title="diagnostics">{{ status }} · {{ diagnostics }}</span>
    </header>
    <div ref="host" class="three-joycon-canvas" />
  </article>
</template>
