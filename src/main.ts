import { invoke } from "@tauri-apps/api/core";

type Controller = { id: string; name: string; product_id: number; transport: string };
type Stick = { x: number; y: number; normalized_x: number; normalized_y: number };
type InputReport = {
  report_id: number;
  bytes: number[];
  left_stick: Stick | null;
  right_stick: Stick | null;
  buttons: [number, number, number] | null;
};

const controllersEl = document.querySelector<HTMLDivElement>("#controllers")!;
const statusEl = document.querySelector<HTMLSpanElement>("#connection-status")!;
const transcriptEl = document.querySelector<HTMLPreElement>("#transcript")!;
const leftStickEl = document.querySelector<HTMLOutputElement>("#left-stick")!;
const rightStickEl = document.querySelector<HTMLOutputElement>("#right-stick")!;
const buttonsEl = document.querySelector<HTMLOutputElement>("#buttons")!;

let selectedController: Controller | undefined;
let reports: string[] = [];
let polling = false;
const isTauriDesktop = "__TAURI_INTERNALS__" in window;

function hex(byte: number) {
  return byte.toString(16).padStart(2, "0").toUpperCase();
}

function renderStick(stick: Stick | null) {
  if (!stick) return "No decoded value";
  return `x ${stick.normalized_x.toFixed(3)}\ny ${stick.normalized_y.toFixed(3)}\nraw ${stick.x}, ${stick.y}`;
}

function renderReport(report: InputReport) {
  leftStickEl.value = renderStick(report.left_stick);
  rightStickEl.value = renderStick(report.right_stick);
  buttonsEl.value = report.buttons ? report.buttons.map(hex).join("  ") : "No standard button data";
  reports.unshift(`${new Date().toLocaleTimeString()}  report 0x${hex(report.report_id)}  ${report.bytes.map(hex).join(" ")}`);
  reports = reports.slice(0, 80);
  transcriptEl.textContent = reports.join("\n");
}

function selectController(controller: Controller) {
  selectedController = controller;
  document.querySelectorAll<HTMLButtonElement>(".controller").forEach((button) => {
    button.classList.toggle("selected", button.dataset.id === controller.id);
  });
  statusEl.textContent = `Reading ${controller.name}. Move a stick or press a button.`;
  statusEl.className = "status connected";
}

function renderControllers(controllers: Controller[]) {
  controllersEl.replaceChildren();
  if (controllers.length === 0) {
    controllersEl.textContent = "No Nintendo controller found. Confirm Bluetooth pairing, then click Refresh.";
    return;
  }

  for (const controller of controllers) {
    const button = document.createElement("button");
    button.className = "controller";
    button.dataset.id = controller.id;
    button.innerHTML = `<strong>${controller.name}</strong><span>product 0x${controller.product_id.toString(16)} · ${controller.transport}</span>`;
    button.addEventListener("click", () => selectController(controller));
    controllersEl.append(button);
  }
  selectController(controllers[0]);
}

async function refreshControllers() {
  if (!isTauriDesktop) {
    statusEl.textContent = "Browser preview detected. Run `pnpm tauri dev` to start the native desktop shell; HID access is unavailable in Vite alone.";
    statusEl.className = "status error";
    controllersEl.textContent = "This dashboard is waiting for the Tauri Rust backend.";
    return;
  }
  statusEl.textContent = "Checking HID devices…";
  try {
    const controllers = await invoke<Controller[]>("list_joycons");
    renderControllers(controllers);
    if (controllers.length === 0) {
      statusEl.textContent = "No Joy-Con exposed to HID yet.";
      statusEl.className = "status";
    }
  } catch (error) {
    statusEl.textContent = String(error);
    statusEl.className = "status error";
  }
}

async function pollInput() {
  if (polling || !selectedController) return;
  polling = true;
  try {
    const report = await invoke<InputReport | null>("poll_joycon_input", { id: selectedController.id });
    if (report) renderReport(report);
  } catch (error) {
    statusEl.textContent = String(error);
    statusEl.className = "status error";
  } finally {
    polling = false;
  }
}

document.querySelector<HTMLButtonElement>("#refresh")!.addEventListener("click", refreshControllers);
window.addEventListener("DOMContentLoaded", async () => {
  await refreshControllers();
  if (isTauriDesktop) window.setInterval(pollInput, 75);
});
