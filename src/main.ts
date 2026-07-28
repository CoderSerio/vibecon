import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

type Controller = { id: string; name: string; product_id: number; transport: string };
type Stick = { x: number; y: number; normalized_x: number; normalized_y: number };
type InputReport = { report_id: number; bytes: number[]; left_stick: Stick | null; right_stick: Stick | null; buttons: [number, number, number] | null };
type StreamEvent = { device_id: string; report: InputReport };
type Annotation = { version: number; created_at_ms: number; controller: { vendor_id: number; product_id: number; orientation: string }; previous_report?: { report_id: number; bytes: number[] }; report: { report_id: number; bytes: number[] }; label: { kind: "stick" | "button"; target: string; phase?: "pressed" | "released" | "moved" | "reset" } };
type LogEntry = { timestamp: Date; previous_report?: InputReport; report: InputReport };

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
let lastKeyOperation: string | undefined;
let lastIncomingReport: InputReport | undefined;
let selectedLog: LogEntry | undefined;
let annotationKind: "stick" | "button" = "stick";
let annotationTarget: string | undefined;
let buttonPhase: "pressed" | "released" = "pressed";
let stickPhase: "moved" | "reset" = "moved";
const isTauriDesktop = "__TAURI_INTERNALS__" in window;

function hex(byte: number) { return byte.toString(16).padStart(2, "0").toUpperCase(); }
function fingerprint(report: { report_id: number; bytes: number[] }) { return `${report.report_id}:${report.bytes.map(hex).join("")}`; }
function labelText(label: Annotation["label"], legacy = false) { const text = label.kind === "stick" ? `Stick · ${label.target} · ${label.phase ?? "moved"}` : `Button · ${label.target.replace("joycon_left.", "")} · ${label.phase ?? "pressed"}`; return legacy ? `${text} · legacy raw` : text; }
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
function appendLog(report: InputReport, previous_report?: InputReport) {
  logs.unshift({ timestamp: new Date(), previous_report, report });
  logs = logs.slice(0, 160);
  renderLogs();
}
function stickBucket(stick: Stick | null) {
  if (!stick) return "unknown";
  const { normalized_x: x, normalized_y: y } = stick;
  const radius = Math.hypot(x, y);
  if (radius < 0.2) return "center";
  const ring = radius < 0.7 ? "inner" : "outer";
  // Eight fixed sectors intentionally mirror the annotation radar, keeping a
  // high-resolution raw stream from producing a label-worthy log line per pixel.
  const sectors = ["e", "se", "s", "sw", "w", "nw", "n", "ne"];
  const index = Math.round(Math.atan2(y, x) / (Math.PI / 4));
  return `${ring}-${sectors[(index + 8) % 8]}`;
}
function keyOperation(report: InputReport) {
  const buttons = report.buttons?.join(":") ?? "none";
  return `${buttons}|${stickBucket(report.left_stick)}`;
}
function shouldLog(report: InputReport) {
  const rate = sampleRateEl.value;
  if (rate === "all") return true;
  if (rate === "key") {
    const current = keyOperation(report);
    if (current === lastKeyOperation) return false;
    lastKeyOperation = current;
    return true;
  }
  const now = performance.now();
  const interval = rate === "75" ? 75 : 1000 / Number(rate);
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
    const matches = annotations.filter((annotation) => annotation.previous_report ? Boolean(entry.previous_report) && fingerprint(annotation.previous_report) === fingerprint(entry.previous_report!) && fingerprint(annotation.report) === key : fingerprint(annotation.report) === key);
    const match = matches[matches.length - 1];
    row.innerHTML = `<time>${entry.timestamp.toLocaleTimeString()}</time><code>report 0x${hex(entry.report.report_id)}  ${entry.report.bytes.map(hex).join(" ")}</code>${match ? `<span class="annotation-tag${match.previous_report ? "" : " legacy"}">${labelText(match.label, !match.previous_report)}</span>` : "<span class=\"label-prompt\">Label</span>"}`;
    row.addEventListener("click", () => openAnnotation(entry));
    transcriptEl.append(row);
  }
}

function selectController(controller: Controller) {
  selectedController = controller;
  lastKeyOperation = undefined;
  lastIncomingReport = undefined;
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
const buttonNames: Record<string, string> = {
  "joycon_left.stick_press": "Stick press", "joycon_left.dpad_up": "D-pad up", "joycon_left.dpad_right": "D-pad right", "joycon_left.dpad_down": "D-pad down", "joycon_left.dpad_left": "D-pad left",
  "joycon_left.minus": "Minus", "joycon_left.capture": "Capture", "joycon_left.sl": "SL", "joycon_left.sr": "SR", "joycon_left.l": "L", "joycon_left.zl": "ZL",
};
function openAnnotation(entry: LogEntry) {
  selectedLog = entry; annotationKind = "stick"; annotationTarget = undefined; buttonPhase = "pressed"; stickPhase = "moved"; saveAnnotationEl.disabled = true;
  selectedReportEl.textContent = `report 0x${hex(entry.report.report_id)} · ${entry.report.bytes.map(hex).join(" ")}`;
  document.querySelectorAll(".kind").forEach((kind) => kind.classList.toggle("active", (kind as HTMLElement).dataset.kind === annotationKind));
  renderPicker(); modal.showModal();
}
function renderPicker() {
  pickerEl.replaceChildren();
  annotationChoiceEl.textContent = annotationTarget ? `Selected: ${annotationKind === "stick" ? `${annotationTarget} · ${stickPhase}` : `${buttonNames[annotationTarget]} · ${buttonPhase}`}` : "Choose a fixed target.";
  if (annotationKind === "stick") {
    const phasePicker = document.createElement("div"); phasePicker.className = "phase-picker";
    (["moved", "reset"] as const).forEach((phase) => { const phaseButton = document.createElement("button"); phaseButton.type = "button"; phaseButton.className = phase === stickPhase ? "selected" : ""; phaseButton.textContent = phase === "moved" ? "Moved" : "Reset to center"; phaseButton.addEventListener("click", () => { stickPhase = phase; annotationTarget = phase === "reset" ? "center" : undefined; saveAnnotationEl.disabled = phase !== "reset"; renderPicker(); }); phasePicker.append(phaseButton); });
    pickerEl.append(phasePicker);
    const radar = document.createElement("div"); radar.className = "stick-radar";
    stickTargets.forEach((target) => {
      const targetButton = document.createElement("button"); targetButton.type = "button"; targetButton.className = `radar-point ${target}`; targetButton.classList.toggle("selected", annotationTarget === target); targetButton.title = target; targetButton.textContent = target === "center" ? "•" : ""; targetButton.disabled = stickPhase === "reset" && target !== "center";
      targetButton.addEventListener("click", () => chooseTarget(target)); radar.append(targetButton);
    }); pickerEl.append(radar);
  } else {
    const layout = document.createElement("div"); layout.className = "button-picker-layout";
    layout.innerHTML = `<div class="annotation-joycon-wrap"><div class="joycon left annotation-joycon" aria-label="Joy-Con control reference"><div class="rail"></div><span class="label shoulder-label">L</span><span class="control shoulder" data-picker-control="joycon_left.zl">ZL</span><span class="control small sl" data-picker-control="joycon_left.sl">SL</span><span class="control small sr" data-picker-control="joycon_left.sr">SR</span><span class="control minus" data-picker-control="joycon_left.minus">−</span><span class="stick" data-picker-control="joycon_left.stick_press"><span class="stick-nub"></span></span><div class="dpad"><span class="control dpad-button up" data-picker-control="joycon_left.dpad_up">▲</span><span class="control dpad-button right" data-picker-control="joycon_left.dpad_right">▶</span><span class="control dpad-button down" data-picker-control="joycon_left.dpad_down">▼</span><span class="control dpad-button left" data-picker-control="joycon_left.dpad_left">◀</span></div><span class="control capture" data-picker-control="joycon_left.capture">●</span><span class="label capture-label">Capture</span></div></div>`;
    const rightColumn = document.createElement("div"); rightColumn.className = "button-picker-column";
    const phasePicker = document.createElement("div"); phasePicker.className = "phase-picker";
    (["pressed", "released"] as const).forEach((phase) => { const phaseButton = document.createElement("button"); phaseButton.type = "button"; phaseButton.className = phase === buttonPhase ? "selected" : ""; phaseButton.textContent = phase === "pressed" ? "Pressed" : "Released"; phaseButton.addEventListener("click", () => { buttonPhase = phase; renderPicker(); }); phasePicker.append(phaseButton); });
    rightColumn.append(phasePicker);
    const picker = document.createElement("div"); picker.className = "button-picker";
    buttonTargets.forEach((target) => {
      const targetButton = document.createElement("button"); targetButton.type = "button"; targetButton.dataset.target = target; targetButton.classList.toggle("selected", annotationTarget === target); targetButton.textContent = buttonNames[target];
      targetButton.addEventListener("mouseenter", () => previewControl(target, true));
      targetButton.addEventListener("mouseleave", () => previewControl(target, false));
      targetButton.addEventListener("click", () => chooseTarget(target)); picker.append(targetButton);
    });
    rightColumn.append(picker); layout.append(rightColumn); pickerEl.append(layout);
  }
}
function previewControl(target: string, active: boolean) {
  const control = pickerEl.querySelector<HTMLElement>(`[data-picker-control="${target}"]`);
  control?.classList.toggle("preview-active", active);
}
function chooseTarget(target: string) {
  annotationTarget = target; saveAnnotationEl.disabled = false;
  annotationChoiceEl.textContent = `Selected: ${annotationKind === "stick" ? `${target} · ${stickPhase}` : `${buttonNames[target]} · ${buttonPhase}`}`;
  pickerEl.querySelectorAll("button").forEach((button) => button.classList.toggle("selected", button.getAttribute("title") === target || button.dataset.target === target));
  pickerEl.querySelectorAll<HTMLElement>("[data-picker-control]").forEach((control) => control.classList.toggle("picker-selected", control.dataset.pickerControl === target));
}
async function saveAnnotation() {
  if (!selectedLog || !selectedController || !annotationTarget) return;
  try {
    const annotation = await invoke<Annotation>("save_annotation", { draft: { controller: { vendor_id: 0x057e, product_id: selectedController.product_id, orientation: "portrait" }, previous_report: selectedLog.previous_report ? { report_id: selectedLog.previous_report.report_id, bytes: selectedLog.previous_report.bytes } : undefined, report: { report_id: selectedLog.report.report_id, bytes: selectedLog.report.bytes }, label: { kind: annotationKind, target: annotationTarget, phase: annotationKind === "button" ? buttonPhase : stickPhase } } });
    annotations.push(annotation); renderLogs(); modal.close();
  } catch (error) { annotationChoiceEl.textContent = `Save failed: ${String(error)}`; }
}

window.addEventListener("DOMContentLoaded", async () => {
  document.querySelector<HTMLButtonElement>("#refresh")!.addEventListener("click", refreshControllers);
  document.querySelector<HTMLButtonElement>("#clear-log")!.addEventListener("click", () => { logs = []; renderLogs(); });
  sampleRateEl.addEventListener("change", () => { lastPresentedAt = 0; lastKeyOperation = undefined; });
  document.querySelectorAll<HTMLButtonElement>(".kind").forEach((button) => button.addEventListener("click", () => { annotationKind = button.dataset.kind as "stick" | "button"; annotationTarget = undefined; saveAnnotationEl.disabled = true; document.querySelectorAll(".kind").forEach((kind) => kind.classList.toggle("active", kind === button)); renderPicker(); }));
  saveAnnotationEl.addEventListener("click", () => { void saveAnnotation(); });
  if (!isTauriDesktop) { await refreshControllers(); return; }
  annotations = await invoke<Annotation[]>("load_annotations").catch((error) => { showError(error); return []; });
  unlistenInput = await listen<StreamEvent>("joycon-input", (event) => {
    if (event.payload.device_id !== selectedController?.id) return;
    // The visualizer is deliberately independent of logging policy.
    renderReport(event.payload.report);
    const previous = lastIncomingReport;
    lastIncomingReport = event.payload.report;
    if (shouldLog(event.payload.report)) appendLog(event.payload.report, previous);
  });
  unlistenError = await listen<string>("joycon-stream-error", (event) => showError(event.payload));
  await refreshControllers();
});
window.addEventListener("beforeunload", () => { unlistenInput?.(); unlistenError?.(); if (isTauriDesktop) void invoke("stop_joycon_stream"); });
