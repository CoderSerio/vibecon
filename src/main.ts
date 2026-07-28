import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

type Controller = { id: string; name: string; product_id: number; transport: string };
type Stick = { x: number; y: number; normalized_x: number; normalized_y: number };
type InputReport = { report_id: number; bytes: number[]; left_stick: Stick | null; right_stick: Stick | null; buttons: [number, number, number] | null };
type StreamEvent = { device_id: string; report: InputReport };
type Annotation = { version: number; created_at_ms: number; controller: { vendor_id: number; product_id: number; orientation: string }; report: { report_id: number; bytes: number[] }; label: { kind: "stick" | "button"; target: string } };
type LogEntry = { timestamp: Date; report: InputReport };

const controllersEl = document.querySelector<HTMLDivElement>("#controllers")!;
const statusEl = document.querySelector<HTMLSpanElement>("#connection-status")!;
const transcriptEl = document.querySelector<HTMLDivElement>("#transcript")!;
const leftStickEl = document.querySelector<HTMLOutputElement>("#left-stick")!;
const rightStickEl = document.querySelector<HTMLOutputElement>("#right-stick")!;
const buttonsEl = document.querySelector<HTMLOutputElement>("#buttons")!;
const stickNub = document.querySelector<HTMLDivElement>("#stick-nub")!;
const sampleRateEl = document.querySelector<HTMLSelectElement>("#sample-rate")!;
const modal = document.querySelector<HTMLDialogElement>("#annotation-modal")!;
const pickerEl = document.querySelector<HTMLDivElement>("#annotation-picker")!;
const selectedReportEl = document.querySelector<HTMLParagraphElement>("#selected-report")!;
const annotationChoiceEl = document.querySelector<HTMLParagraphElement>("#annotation-choice")!;
const saveAnnotationEl = document.querySelector<HTMLButtonElement>("#save-annotation")!;

let selectedController: Controller | undefined;
let logs: LogEntry[] = [];
let annotations: Annotation[] = [];
let unlistenInput: UnlistenFn | undefined;
let unlistenError: UnlistenFn | undefined;
let lastPresentedAt = 0;
let selectedLog: LogEntry | undefined;
let annotationKind: "stick" | "button" = "stick";
let annotationTarget: string | undefined;
const isTauriDesktop = "__TAURI_INTERNALS__" in window;

function hex(byte: number) { return byte.toString(16).padStart(2, "0").toUpperCase(); }
function fingerprint(report: { report_id: number; bytes: number[] }) { return `${report.report_id}:${report.bytes.map(hex).join("")}`; }
function labelText(label: Annotation["label"]) { return label.kind === "stick" ? `Stick · ${label.target}` : `Button · ${label.target.replace("joycon_left.", "")}`; }
function renderStick(stick: Stick | null) { return stick ? `x ${stick.normalized_x.toFixed(3)}\ny ${stick.normalized_y.toFixed(3)}\nraw ${stick.x}, ${stick.y}` : "No decoded value"; }

function renderReport(report: InputReport) {
  leftStickEl.value = renderStick(report.left_stick);
  rightStickEl.value = renderStick(report.right_stick);
  updateStickNub(report.left_stick);
  updateButtons(report);
}
function updateStickNub(stick: Stick | null) {
  if (!stick) return;
  stickNub.style.transform = `translate(${(Math.max(-1, Math.min(1, stick.normalized_x)) * 27).toFixed(1)}px, ${(Math.max(-1, Math.min(1, stick.normalized_y)) * 27).toFixed(1)}px)`;
}
function updateButtons(report: InputReport) {
  document.querySelectorAll<HTMLElement>("[data-control]").forEach((control) => control.classList.remove("active"));
  if (!report.buttons) { buttonsEl.value = "No decoded button data"; return; }
  const [buttonMask, extraButtons, hat] = report.buttons;
  const directions: Record<string, number> = { left: 0x01, down: 0x02, up: 0x04, right: 0x08 };
  for (const [direction, mask] of Object.entries(directions)) if ((buttonMask & mask) !== 0) document.querySelector(`[data-control="${direction}"]`)?.classList.add("active");
  buttonsEl.value = `D-pad 0x${hex(buttonMask)} · stick HAT ${hat === 8 ? "neutral" : hat} · extra 0x${hex(extraButtons)}`;
}
function present(report: InputReport) {
  renderReport(report);
  logs.unshift({ timestamp: new Date(), report });
  logs = logs.slice(0, 160);
  renderLogs();
}
function shouldPresent() {
  const rate = sampleRateEl.value;
  if (rate === "all") return true;
  const now = performance.now();
  const interval = 1000 / Number(rate);
  if (now - lastPresentedAt < interval) return false;
  lastPresentedAt = now;
  return true;
}
function renderLogs() {
  transcriptEl.replaceChildren();
  if (logs.length === 0) { transcriptEl.textContent = "No reports in this view. Move a stick or press a button."; return; }
  for (const entry of logs) {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "log-row";
    const key = fingerprint(entry.report);
    const matches = annotations.filter((annotation) => fingerprint(annotation.report) === key);
    row.innerHTML = `<time>${entry.timestamp.toLocaleTimeString()}</time><code>report 0x${hex(entry.report.report_id)}  ${entry.report.bytes.map(hex).join(" ")}</code>${matches.length ? `<span class="annotation-tag">${labelText(matches[matches.length - 1].label)}</span>` : "<span class=\"label-prompt\">Label</span>"}`;
    row.addEventListener("click", () => openAnnotation(entry));
    transcriptEl.append(row);
  }
}

function selectController(controller: Controller) {
  selectedController = controller;
  document.querySelectorAll<HTMLButtonElement>(".controller").forEach((button) => button.classList.toggle("selected", button.dataset.id === controller.id));
  statusEl.textContent = `Streaming ${controller.name}. Move a stick or press a button.`;
  statusEl.className = "status connected";
  void invoke("start_joycon_stream", { id: controller.id }).catch(showError);
}
function renderControllers(controllers: Controller[]) {
  controllersEl.replaceChildren();
  if (controllers.length === 0) { controllersEl.textContent = "No Nintendo controller found. Confirm Bluetooth pairing, then click Refresh."; return; }
  for (const controller of controllers) {
    const button = document.createElement("button"); button.className = "controller"; button.dataset.id = controller.id;
    button.innerHTML = `<strong>${controller.name}</strong><span>product 0x${controller.product_id.toString(16)} · ${controller.transport}</span>`;
    button.addEventListener("click", () => selectController(controller)); controllersEl.append(button);
  }
  selectController(controllers[0]);
}
function showError(error: unknown) { statusEl.textContent = String(error); statusEl.className = "status error"; }
async function refreshControllers() {
  if (!isTauriDesktop) { showError("Browser preview detected. Run `pnpm tauri dev` for HID access."); controllersEl.textContent = "This dashboard is waiting for the Tauri Rust backend."; return; }
  statusEl.textContent = "Checking HID devices…";
  try { renderControllers(await invoke<Controller[]>("list_joycons")); } catch (error) { showError(error); }
}

const stickTargets = ["center", ...["n", "ne", "e", "se", "s", "sw", "w", "nw"].flatMap((direction) => [`inner-${direction}`, `outer-${direction}`])];
const buttonTargets = ["joycon_left.stick_press", "joycon_left.dpad_up", "joycon_left.dpad_right", "joycon_left.dpad_down", "joycon_left.dpad_left", "joycon_left.minus", "joycon_left.capture", "joycon_left.sl", "joycon_left.sr", "joycon_left.l", "joycon_left.zl"];
function openAnnotation(entry: LogEntry) {
  selectedLog = entry; annotationKind = "stick"; annotationTarget = undefined; saveAnnotationEl.disabled = true;
  selectedReportEl.textContent = `report 0x${hex(entry.report.report_id)} · ${entry.report.bytes.map(hex).join(" ")}`;
  document.querySelectorAll(".kind").forEach((kind) => kind.classList.toggle("active", (kind as HTMLElement).dataset.kind === annotationKind));
  renderPicker(); modal.showModal();
}
function renderPicker() {
  pickerEl.replaceChildren(); annotationChoiceEl.textContent = "Choose a fixed target.";
  if (annotationKind === "stick") {
    const radar = document.createElement("div"); radar.className = "stick-radar";
    stickTargets.forEach((target) => {
      const targetButton = document.createElement("button"); targetButton.type = "button"; targetButton.className = `radar-point ${target}`; targetButton.title = target; targetButton.textContent = target === "center" ? "•" : "";
      targetButton.addEventListener("click", () => chooseTarget(target)); radar.append(targetButton);
    }); pickerEl.append(radar);
  } else {
    const picker = document.createElement("div"); picker.className = "button-picker";
    buttonTargets.forEach((target) => { const targetButton = document.createElement("button"); targetButton.type = "button"; targetButton.textContent = target.replace("joycon_left.", "").replace(/_/g, " "); targetButton.addEventListener("click", () => chooseTarget(target)); picker.append(targetButton); }); pickerEl.append(picker);
  }
}
function chooseTarget(target: string) {
  annotationTarget = target; saveAnnotationEl.disabled = false;
  annotationChoiceEl.textContent = `Selected: ${annotationKind === "stick" ? target : target.replace("joycon_left.", "")}`;
  pickerEl.querySelectorAll("button").forEach((button) => button.classList.toggle("selected", button.getAttribute("title") === target || button.textContent === target.replace("joycon_left.", "").replace(/_/g, " ")));
}
async function saveAnnotation() {
  if (!selectedLog || !selectedController || !annotationTarget) return;
  try {
    const annotation = await invoke<Annotation>("save_annotation", { draft: { controller: { vendor_id: 0x057e, product_id: selectedController.product_id, orientation: "portrait" }, report: { report_id: selectedLog.report.report_id, bytes: selectedLog.report.bytes }, label: { kind: annotationKind, target: annotationTarget } } });
    annotations.push(annotation); renderLogs(); modal.close();
  } catch (error) { annotationChoiceEl.textContent = `Save failed: ${String(error)}`; }
}

window.addEventListener("DOMContentLoaded", async () => {
  document.querySelector<HTMLButtonElement>("#refresh")!.addEventListener("click", refreshControllers);
  document.querySelector<HTMLButtonElement>("#clear-log")!.addEventListener("click", () => { logs = []; renderLogs(); });
  sampleRateEl.addEventListener("change", () => { lastPresentedAt = 0; });
  document.querySelectorAll<HTMLButtonElement>(".kind").forEach((button) => button.addEventListener("click", () => { annotationKind = button.dataset.kind as "stick" | "button"; annotationTarget = undefined; saveAnnotationEl.disabled = true; document.querySelectorAll(".kind").forEach((kind) => kind.classList.toggle("active", kind === button)); renderPicker(); }));
  saveAnnotationEl.addEventListener("click", () => { void saveAnnotation(); });
  if (!isTauriDesktop) { await refreshControllers(); return; }
  annotations = await invoke<Annotation[]>("load_annotations").catch((error) => { showError(error); return []; });
  unlistenInput = await listen<StreamEvent>("joycon-input", (event) => { if (event.payload.device_id === selectedController?.id && shouldPresent()) present(event.payload.report); });
  unlistenError = await listen<string>("joycon-stream-error", (event) => showError(event.payload));
  await refreshControllers();
});
window.addEventListener("beforeunload", () => { unlistenInput?.(); unlistenError?.(); if (isTauriDesktop) void invoke("stop_joycon_stream"); });
