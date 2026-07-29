use hidapi::HidApi;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "macos")]
use core_graphics::{
    event::{CGEvent, CGEventFlags, CGEventTapLocation},
    event_source::{CGEventSource, CGEventSourceStateID},
};
#[cfg(target_os = "macos")]
use std::process::Command;

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}
use tauri::Emitter;

const NINTENDO_VENDOR_ID: u16 = 0x057e;
const JOYCON_LEFT_PRODUCT_ID: u16 = 0x2006;
const JOYCON_RIGHT_PRODUCT_ID: u16 = 0x2007;
const STICK_TRIGGER_THRESHOLD: f32 = 0.40;
const MAPPING_COOLDOWN: Duration = Duration::from_millis(240);

#[derive(Serialize)]
struct ControllerDevice {
    id: String,
    name: String,
    product_id: u16,
    transport: String,
}

#[derive(Clone, Serialize)]
struct Stick {
    x: u16,
    y: u16,
    normalized_x: f32,
    normalized_y: f32,
}

#[derive(Clone, Serialize)]
struct InputReport {
    report_id: u8,
    bytes: Vec<u8>,
    left_stick: Option<Stick>,
    right_stick: Option<Stick>,
    buttons: Option<[u8; 3]>,
}

#[derive(Clone, Serialize)]
struct StreamEvent {
    device_id: String,
    report: InputReport,
}

#[derive(Clone)]
struct StreamState {
    active_ids: Arc<Mutex<HashSet<String>>>,
    devices: Arc<Mutex<HashMap<String, Arc<Mutex<hidapi::HidDevice>>>>>,
    output_packet_counter: Arc<AtomicU8>,
}

impl Default for StreamState {
    fn default() -> Self {
        Self {
            active_ids: Arc::new(Mutex::new(HashSet::new())),
            devices: Arc::new(Mutex::new(HashMap::new())),
            output_packet_counter: Arc::new(AtomicU8::new(0)),
        }
    }
}

#[derive(Clone)]
struct MappingRuntimeState {
    active: Arc<AtomicBool>,
    config: Arc<Mutex<MappingConfig>>,
}

impl Default for MappingRuntimeState {
    fn default() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
            config: Arc::new(Mutex::new(MappingConfig::default())),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct AnnotationController {
    vendor_id: u16,
    product_id: u16,
    orientation: String,
}

#[derive(Clone, Deserialize, Serialize)]
struct AnnotationReport {
    report_id: u8,
    bytes: Vec<u8>,
}

#[derive(Clone, Deserialize, Serialize)]
struct AnnotationLabel {
    kind: String,
    target: String,
    #[serde(default)]
    phase: Option<String>,
}

#[derive(Deserialize)]
struct AnnotationDraft {
    controller: AnnotationController,
    #[serde(default)]
    previous_report: Option<AnnotationReport>,
    report: AnnotationReport,
    label: AnnotationLabel,
}

#[derive(Clone, Serialize, Deserialize)]
struct Annotation {
    version: u8,
    created_at_ms: u128,
    controller: AnnotationController,
    #[serde(default)]
    previous_report: Option<AnnotationReport>,
    report: AnnotationReport,
    label: AnnotationLabel,
}

#[derive(Clone, Deserialize, Serialize)]
struct MappingSettings {
    window_switch_enabled: bool,
    #[serde(default)]
    focus_codex_enabled: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MappingBinding {
    id: String,
    control: String,
    action: String,
    enabled: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MappingPreset {
    id: String,
    name: String,
    enabled: bool,
    bindings: Vec<MappingBinding>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MappingConfig {
    version: u8,
    active_preset_id: String,
    presets: Vec<MappingPreset>,
}

impl MappingConfig {
    fn active_preset(&self) -> Option<&MappingPreset> {
        self.presets
            .iter()
            .find(|preset| preset.id == self.active_preset_id)
    }
}

impl Default for MappingConfig {
    fn default() -> Self {
        Self {
            version: 1,
            active_preset_id: "codex-cowork".to_owned(),
            presets: vec![
                MappingPreset {
                    id: "code".to_owned(),
                    name: "Code".to_owned(),
                    enabled: true,
                    bindings: vec![
                        mapping_binding("window-previous", "joycon_left.stick_left", "window_previous"),
                        mapping_binding("window-next", "joycon_left.stick_right", "window_next"),
                    ],
                },
                MappingPreset {
                    id: "codex-cowork".to_owned(),
                    name: "Codex Cowork".to_owned(),
                    enabled: true,
                    bindings: vec![
                        mapping_binding("window-previous", "joycon_left.stick_left", "window_previous"),
                        mapping_binding("window-next", "joycon_left.stick_right", "window_next"),
                        mapping_binding("focus-codex-left", "joycon_left.dpad_up", "focus_codex"),
                        mapping_binding("focus-codex-right", "joycon_right.x", "focus_codex"),
                    ],
                },
                MappingPreset {
                    id: "inspect-only".to_owned(),
                    name: "Inspect Only".to_owned(),
                    enabled: false,
                    bindings: vec![],
                },
                MappingPreset {
                    id: "keyboard-focus".to_owned(),
                    name: "Keyboard Focus".to_owned(),
                    enabled: false,
                    bindings: vec![
                        mapping_binding("focus-previous", "joycon_left.dpad_up", "focus_previous"),
                        mapping_binding("focus-next", "joycon_left.dpad_down", "focus_next"),
                        mapping_binding("activate-focused", "joycon_right.a", "activate_focused"),
                    ],
                },
            ],
        }
    }
}

fn mapping_binding(id: &str, control: &str, action: &str) -> MappingBinding {
    MappingBinding {
        id: id.to_owned(),
        control: control.to_owned(),
        action: action.to_owned(),
        enabled: true,
    }
}

struct BindingTriggerState {
    armed: bool,
    last_triggered_at: Instant,
}

impl Default for BindingTriggerState {
    fn default() -> Self {
        Self {
            armed: true,
            last_triggered_at: Instant::now() - Duration::from_secs(1),
        }
    }
}

impl Default for MappingSettings {
    fn default() -> Self {
        Self {
            window_switch_enabled: false,
            focus_codex_enabled: false,
        }
    }
}

fn display_name(product_id: u16) -> &'static str {
    match product_id {
        JOYCON_LEFT_PRODUCT_ID => "Joy-Con (L)",
        JOYCON_RIGHT_PRODUCT_ID => "Joy-Con (R)",
        _ => "Nintendo controller",
    }
}

fn open_api() -> Result<HidApi, String> {
    HidApi::new().map_err(|error| format!("Cannot access HID devices: {error}"))
}

fn report_from_bytes(bytes: Vec<u8>, product_id: u16) -> InputReport {
    let (left_stick, right_stick, buttons) = decode_joycon_report(&bytes, product_id);
    InputReport {
        report_id: bytes[0],
        bytes,
        left_stick,
        right_stick,
        buttons,
    }
}

#[tauri::command]
fn list_joycons() -> Result<Vec<ControllerDevice>, String> {
    let api = open_api()?;
    Ok(api
        .device_list()
        .filter(|device| device.vendor_id() == NINTENDO_VENDOR_ID)
        .map(|device| ControllerDevice {
            id: device.path().to_string_lossy().into_owned(),
            name: device
                .product_string()
                .map(str::to_owned)
                .unwrap_or_else(|| display_name(device.product_id()).to_owned()),
            product_id: device.product_id(),
            transport: "Bluetooth / HID".to_owned(),
        })
        .collect())
}

/// Keep the HID handle open in a reader thread. The frontend chooses how often
/// to present these events; it never throttles physical input collection.
#[tauri::command]
fn start_joycon_stream(
    app: tauri::AppHandle,
    state: tauri::State<StreamState>,
    mapping_state: tauri::State<MappingRuntimeState>,
    id: String,
) -> Result<(), String> {
    if !state.active_ids.lock().map_err(|_| "Input stream state is unavailable")?.insert(id.clone()) {
        return Ok(());
    }
    let stream_state = state.inner().clone();
    let mapping_state = mapping_state.inner().clone();
    thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let api = open_api()?;
            let info = api
                .device_list()
                .find(|device| device.path().to_string_lossy() == id)
                .ok_or("The selected Joy-Con is no longer connected")?;
            let device = Arc::new(Mutex::new(info
                .open_device(&api)
                .map_err(|error| format!("Could not open Joy-Con input: {error}"))?));
            stream_state
                .devices
                .lock()
                .map_err(|_| "Joy-Con output state is unavailable")?
                .insert(id.clone(), device.clone());
            let mut buffer = [0_u8; 64];
            // Keep the frontend responsive even when Bluetooth HID reports at
            // a much higher rate than the WebView can render.
            let mut last_emitted_at = Instant::now() - Duration::from_secs(1);
            let mut binding_states = HashMap::<String, BindingTriggerState>::new();
            while stream_state.active_ids.lock().map(|ids| ids.contains(&id)).unwrap_or(false) {
                let count = device
                    .lock()
                    .map_err(|_| "Joy-Con device handle is unavailable")?
                    .read_timeout(&mut buffer, 8)
                    .map_err(|error| format!("Could not read Joy-Con input: {error}"))?;
                if count > 0 && last_emitted_at.elapsed() >= Duration::from_millis(16) {
                    let report = report_from_bytes(buffer[..count].to_vec(), info.product_id());
                    process_mapping_report(
                        &report,
                        info.product_id(),
                        &mapping_state,
                        &mut binding_states,
                    );
                    let event = StreamEvent {
                        device_id: id.clone(),
                        report,
                    };
                    let _ = app.emit("joycon-input", event);
                    last_emitted_at = Instant::now();
                }
            }
            Ok(())
        })();
        if let Err(message) = result {
            let _ = app.emit("joycon-stream-error", message);
        }
        if let Ok(mut ids) = stream_state.active_ids.lock() {
            ids.remove(&id);
        }
        if let Ok(mut devices) = stream_state.devices.lock() {
            devices.remove(&id);
        }
    });
    Ok(())
}

fn process_mapping_report(
    report: &InputReport,
    product_id: u16,
    mapping_state: &MappingRuntimeState,
    binding_states: &mut HashMap<String, BindingTriggerState>,
) {
    if !mapping_state.active.load(Ordering::Relaxed) {
        binding_states.clear();
        return;
    }
    let config = match mapping_state.config.lock() {
        Ok(config) => config.clone(),
        Err(_) => return,
    };
    let Some(preset) = config.active_preset() else { return };
    if !preset.enabled {
        return;
    }
    for binding in preset.bindings.iter().filter(|binding| binding.enabled) {
        let pressed = control_is_pressed(&binding.control, report, product_id);
        let state = binding_states.entry(binding.id.clone()).or_default();
        if !pressed {
            state.armed = true;
            continue;
        }
        if !state.armed || state.last_triggered_at.elapsed() < MAPPING_COOLDOWN {
            continue;
        }
        state.armed = false;
        state.last_triggered_at = Instant::now();
        let _ = dispatch_mapping_action(&binding.action);
    }
}

fn control_is_pressed(control: &str, report: &InputReport, product_id: u16) -> bool {
    match control {
        "joycon_left.stick_left" if product_id == JOYCON_LEFT_PRODUCT_ID => report
            .left_stick
            .as_ref()
            .is_some_and(|stick| stick.normalized_x <= -STICK_TRIGGER_THRESHOLD),
        "joycon_left.stick_right" if product_id == JOYCON_LEFT_PRODUCT_ID => report
            .left_stick
            .as_ref()
            .is_some_and(|stick| stick.normalized_x >= STICK_TRIGGER_THRESHOLD),
        _ => pressed_button_controls(report, product_id).iter().any(|target| *target == control),
    }
}

fn pressed_button_controls(report: &InputReport, product_id: u16) -> Vec<&'static str> {
    let Some([buttons, extra, left]) = report.buttons else { return vec![] };
    let mut controls = Vec::new();
    let mut add_bits = |bits: u8, mappings: &[(&'static str, u8)]| {
        controls.extend(
            mappings
                .iter()
                .filter_map(|(name, mask)| (bits & mask != 0).then_some(*name)),
        );
    };
    match (report.report_id, product_id) {
        (0x30, JOYCON_LEFT_PRODUCT_ID) => {
            add_bits(left, &[
                ("joycon_left.dpad_down", 0x01), ("joycon_left.dpad_up", 0x02),
                ("joycon_left.dpad_right", 0x04), ("joycon_left.dpad_left", 0x08),
                ("joycon_left.sr", 0x10), ("joycon_left.sl", 0x20),
                ("joycon_left.l", 0x40), ("joycon_left.zl", 0x80),
            ]);
            add_bits(extra, &[
                ("joycon_left.minus", 0x01), ("joycon_left.stick_press", 0x04),
                ("joycon_left.capture", 0x20),
            ]);
        }
        (0x30, JOYCON_RIGHT_PRODUCT_ID) => {
            add_bits(buttons, &[
                ("joycon_right.y", 0x01), ("joycon_right.x", 0x02),
                ("joycon_right.b", 0x04), ("joycon_right.a", 0x08),
                ("joycon_right.sr", 0x10), ("joycon_right.sl", 0x20),
                ("joycon_right.r", 0x40), ("joycon_right.zr", 0x80),
            ]);
            add_bits(extra, &[
                ("joycon_right.plus", 0x02), ("joycon_right.stick_press", 0x08),
                ("joycon_right.home", 0x10),
            ]);
        }
        (0x3f, JOYCON_LEFT_PRODUCT_ID) => {
            add_bits(buttons, &[
                ("joycon_left.dpad_left", 0x01), ("joycon_left.dpad_down", 0x02),
                ("joycon_left.dpad_up", 0x04), ("joycon_left.dpad_right", 0x08),
                ("joycon_left.sl", 0x10), ("joycon_left.sr", 0x20),
            ]);
            add_bits(extra, &[
                ("joycon_left.minus", 0x01), ("joycon_left.stick_press", 0x04),
                ("joycon_left.capture", 0x20), ("joycon_left.l", 0x40),
                ("joycon_left.zl", 0x80),
            ]);
        }
        (0x3f, JOYCON_RIGHT_PRODUCT_ID) => {
            add_bits(buttons, &[
                ("joycon_right.y", 0x01), ("joycon_right.x", 0x02),
                ("joycon_right.b", 0x04), ("joycon_right.a", 0x08),
                ("joycon_right.sr", 0x10), ("joycon_right.sl", 0x20),
                ("joycon_right.r", 0x40), ("joycon_right.zr", 0x80),
            ]);
        }
        _ => {}
    }
    controls
}

fn dispatch_mapping_action(action: &str) -> Result<(), String> {
    match action {
        "window_next" => switch_window("next".to_owned()),
        "window_previous" => switch_window("previous".to_owned()),
        "focus_codex" => focus_codex(),
        "focus_next" => move_keyboard_focus("next"),
        "focus_previous" => move_keyboard_focus("previous"),
        "activate_focused" => activate_keyboard_focus(),
        _ => Err(format!("Unsupported mapping action: {action}")),
    }
}

#[tauri::command]
fn stop_joycon_stream(state: tauri::State<StreamState>, id: Option<String>) {
    if let Ok(mut ids) = state.active_ids.lock() {
        if let Some(id) = id {
            ids.remove(&id);
            if let Ok(mut devices) = state.devices.lock() {
                devices.remove(&id);
            }
        } else {
            ids.clear();
            if let Ok(mut devices) = state.devices.lock() {
                devices.clear();
            }
        }
    }
}

#[tauri::command]
fn set_mapping_runtime(
    state: tauri::State<MappingRuntimeState>,
    config: MappingConfig,
    active: bool,
) -> Result<(), String> {
    validate_mapping_config(&config)?;
    *state
        .config
        .lock()
        .map_err(|_| "Mapping runtime is unavailable")? = config;
    state.active.store(active, Ordering::Relaxed);
    Ok(())
}

fn focus_codex() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-a", "Codex"])
            .spawn()
            .map_err(|error| format!("Could not focus Codex: {error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Focusing Codex is not implemented for this platform yet".to_owned())
    }
}

#[tauri::command]
fn switch_window(direction: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if !macos_accessibility_trusted() {
            return Err("Accessibility is not granted to this running VibeCon process. Quit the app, enable VibeCon.app in System Settings → Privacy & Security → Accessibility, then reopen VibeCon.app.".to_owned());
        }
        // Post the shortcut directly through Quartz instead of spawning
        // `osascript`. This uses the Accessibility permission granted to
        // VibeCon itself and keeps the HID reader responsive.
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|_| "Could not create a macOS keyboard event source")?;
        let modifiers = if direction == "previous" {
            CGEventFlags::CGEventFlagCommand | CGEventFlags::CGEventFlagShift
        } else {
            CGEventFlags::CGEventFlagCommand
        };
        // System shortcuts such as Cmd+Tab need real modifier key transitions.
        // Setting flags on the Tab event alone is enough for text input but
        // is not reliably accepted by macOS's application switcher.
        post_key(&source, 55, true, CGEventFlags::CGEventFlagCommand)?; // Command down
        if direction == "previous" {
            post_key(&source, 56, true, modifiers)?; // Shift down
        }
        post_key(&source, 48, true, modifiers)?; // Tab down
        post_key(&source, 48, false, modifiers)?; // Tab up
        if direction == "previous" {
            post_key(&source, 56, false, CGEventFlags::CGEventFlagCommand)?; // Shift up
        }
        post_key(&source, 55, false, CGEventFlags::CGEventFlagNull)?; // Command up
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = direction;
        Err("Window switching is not implemented for this platform yet".to_owned())
    }
}

const NEUTRAL_RUMBLE_FRAME: [u8; 4] = [0x00, 0x01, 0x40, 0x40];
const GENTLE_RUMBLE_FRAME: [u8; 4] = [0x00, 0x58, 0xC0, 0x59];

fn next_output_packet_counter(state: &StreamState) -> u8 {
    state.output_packet_counter.fetch_add(1, Ordering::Relaxed) & 0x0f
}

fn rumble_subcommand(counter: u8, subcommand: u8, data: u8) -> [u8; 12] {
    [
        0x01,
        counter & 0x0f,
        NEUTRAL_RUMBLE_FRAME[0],
        NEUTRAL_RUMBLE_FRAME[1],
        NEUTRAL_RUMBLE_FRAME[2],
        NEUTRAL_RUMBLE_FRAME[3],
        NEUTRAL_RUMBLE_FRAME[0],
        NEUTRAL_RUMBLE_FRAME[1],
        NEUTRAL_RUMBLE_FRAME[2],
        NEUTRAL_RUMBLE_FRAME[3],
        subcommand,
        data,
    ]
}

fn rumble_report(counter: u8, frame: [u8; 4]) -> [u8; 10] {
    [
        0x10, counter & 0x0f, frame[0], frame[1], frame[2], frame[3], frame[0], frame[1],
        frame[2], frame[3],
    ]
}

fn write_joycon_output(device: &Arc<Mutex<hidapi::HidDevice>>, bytes: &[u8]) -> Result<(), String> {
    device
        .lock()
        .map_err(|_| "Joy-Con device handle is unavailable")?
        .write(bytes)
        .map(|_| ())
        .map_err(|error| format!("Could not write Joy-Con output report: {error}"))
}

/// Send one deliberately short and gentle vibration pulse. This is only a
/// manual experiment: callers must have already selected and opened the exact
/// Joy-Con. Every path attempts the neutral frame after the pulse.
#[tauri::command]
fn test_joycon_vibration(state: tauri::State<StreamState>, id: String) -> Result<(), String> {
    let device = state
        .devices
        .lock()
        .map_err(|_| "Joy-Con output state is unavailable")?
        .get(&id)
        .cloned()
        .ok_or("Select this Joy-Con in Debug before testing vibration")?;
    let enable = rumble_subcommand(next_output_packet_counter(state.inner()), 0x48, 0x01);
    let pulse = rumble_report(next_output_packet_counter(state.inner()), GENTLE_RUMBLE_FRAME);
    let neutral = rumble_report(next_output_packet_counter(state.inner()), NEUTRAL_RUMBLE_FRAME);
    write_joycon_output(&device, &enable)
        .map_err(|error| format!("Could not enable Joy-Con rumble: {error}"))?;
    thread::sleep(Duration::from_millis(120));
    let pulse_result = write_joycon_output(&device, &pulse)
        .map_err(|error| format!("Could not send Joy-Con rumble pulse: {error}"));
    thread::sleep(Duration::from_millis(70));
    let neutral_result = write_joycon_output(&device, &neutral)
        .map_err(|error| format!("Could not stop Joy-Con rumble: {error}"));
    pulse_result?;
    neutral_result?;
    Ok(())
}

/// A deliberately small accessibility-navigation primitive. It does not read
/// or manipulate an accessibility tree; it sends the standard keyboard focus
/// keys to the foreground app, where normal Tab navigation is supported.
fn move_keyboard_focus(direction: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        ensure_macos_accessibility()?;
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|_| "Could not create a macOS keyboard event source")?;
        if direction == "previous" {
            post_key(&source, 56, true, CGEventFlags::CGEventFlagShift)?;
            post_key(&source, 48, true, CGEventFlags::CGEventFlagShift)?;
            post_key(&source, 48, false, CGEventFlags::CGEventFlagShift)?;
            post_key(&source, 56, false, CGEventFlags::CGEventFlagNull)?;
        } else {
            post_key(&source, 48, true, CGEventFlags::CGEventFlagNull)?;
            post_key(&source, 48, false, CGEventFlags::CGEventFlagNull)?;
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = direction;
        Err("Keyboard focus navigation is not implemented for this platform yet".to_owned())
    }
}

fn activate_keyboard_focus() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        ensure_macos_accessibility()?;
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|_| "Could not create a macOS keyboard event source")?;
        // Space activates the currently focused standard macOS control.
        post_key(&source, 49, true, CGEventFlags::CGEventFlagNull)?;
        post_key(&source, 49, false, CGEventFlags::CGEventFlagNull)?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Keyboard focus activation is not implemented for this platform yet".to_owned())
    }
}

#[cfg(target_os = "macos")]
fn ensure_macos_accessibility() -> Result<(), String> {
    if macos_accessibility_trusted() {
        Ok(())
    } else {
        Err("Accessibility is not granted to this running VibeCon process. Quit the app, enable VibeCon.app in System Settings → Privacy & Security → Accessibility, then reopen VibeCon.app.".to_owned())
    }
}

#[cfg(target_os = "macos")]
fn post_key(
    source: &CGEventSource,
    key_code: u16,
    key_down: bool,
    flags: CGEventFlags,
) -> Result<(), String> {
    let event = CGEvent::new_keyboard_event(source.clone(), key_code, key_down)
        .map_err(|_| "Could not create a macOS keyboard event")?;
    event.set_flags(flags);
    event.post(CGEventTapLocation::HID);
    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_accessibility_trusted() -> bool {
    // SAFETY: AXIsProcessTrusted has no arguments and only reports TCC state.
    unsafe { AXIsProcessTrusted() }
}

#[tauri::command]
fn mapping_accessibility_status() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(if macos_accessibility_trusted() {
            "Accessibility: granted to this running VibeCon process.".to_owned()
        } else {
            format!(
                "Accessibility: not granted to this running process. Remove the old VibeCon entry, then add this exact app and reopen it: {}",
                running_app_path(),
            )
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Window switching is not implemented for this platform yet".to_owned())
    }
}

#[tauri::command]
fn open_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map_err(|error| format!("Could not open macOS Accessibility settings: {error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Accessibility settings are not implemented for this platform yet".to_owned())
    }
}

#[cfg(target_os = "macos")]
fn running_app_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.ancestors().nth(3).map(|app| app.display().to_string()))
        .unwrap_or_else(|| "could not determine the current VibeCon.app path".to_owned())
}

fn annotation_path() -> Result<PathBuf, String> {
    vibecon_data_directory().map(|directory| directory.join("annotations.jsonl"))
}

fn vibecon_data_directory() -> Result<PathBuf, String> {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or("Could not determine the home directory for VibeCon annotations")?;
    let directory = PathBuf::from(home).join(".vibecon");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create ~/.vibecon: {error}"))?;
    Ok(directory)
}

fn mapping_settings_path() -> Result<PathBuf, String> {
    vibecon_data_directory().map(|directory| directory.join("mapping-settings.json"))
}

fn mapping_config_path() -> Result<PathBuf, String> {
    vibecon_data_directory().map(|directory| directory.join("mappings.json"))
}

fn migrated_mapping_config(settings: MappingSettings) -> MappingConfig {
    let mut config = MappingConfig::default();
    let preset = config
        .presets
        .iter_mut()
        .find(|preset| preset.id == "codex-cowork")
        .expect("built-in Codex preset exists");
    for binding in &mut preset.bindings {
        binding.enabled = match binding.action.as_str() {
            "focus_codex" => settings.focus_codex_enabled,
            "window_next" | "window_previous" => settings.window_switch_enabled,
            _ => false,
        };
    }
    preset.enabled = settings.window_switch_enabled || settings.focus_codex_enabled;
    config
}

fn write_mapping_config(config: &MappingConfig) -> Result<(), String> {
    validate_mapping_config(config)?;
    let path = mapping_config_path()?;
    let serialized = serde_json::to_string_pretty(config)
        .map_err(|error| format!("Could not encode mapping configuration: {error}"))?;
    fs::write(&path, format!("{serialized}\n"))
        .map_err(|error| format!("Could not write mapping configuration: {error}"))
}

fn validate_mapping_config(config: &MappingConfig) -> Result<(), String> {
    if config.version != 1 {
        return Err("Unsupported mapping configuration version".to_owned());
    }
    if config.presets.is_empty() {
        return Err("At least one mapping preset is required".to_owned());
    }
    if !config.presets.iter().any(|preset| preset.id == config.active_preset_id) {
        return Err("The active mapping preset does not exist".to_owned());
    }
    let mut preset_ids = HashSet::new();
    let mut binding_ids = HashSet::new();
    for preset in &config.presets {
        if preset.id.trim().is_empty() || !preset_ids.insert(&preset.id) {
            return Err("Each mapping preset needs a unique non-empty id".to_owned());
        }
        for binding in &preset.bindings {
            if binding.id.trim().is_empty() || !binding_ids.insert(format!("{}:{}", preset.id, binding.id)) {
                return Err(format!("Preset {} has duplicate or empty binding ids", preset.id));
            }
            if !known_mapping_controls().contains(&binding.control.as_str()) {
                return Err(format!("Unsupported mapping control: {}", binding.control));
            }
            if !known_mapping_actions().contains(&binding.action.as_str()) {
                return Err(format!("Unsupported mapping action: {}", binding.action));
            }
        }
    }
    Ok(())
}

fn known_mapping_controls() -> &'static [&'static str] {
    &[
        "joycon_left.stick_left", "joycon_left.stick_right",
        "joycon_left.dpad_up", "joycon_left.dpad_down", "joycon_left.dpad_left", "joycon_left.dpad_right",
        "joycon_left.stick_press", "joycon_left.minus", "joycon_left.capture", "joycon_left.sl", "joycon_left.sr", "joycon_left.l", "joycon_left.zl",
        "joycon_right.x", "joycon_right.y", "joycon_right.a", "joycon_right.b",
        "joycon_right.stick_press", "joycon_right.plus", "joycon_right.home", "joycon_right.sl", "joycon_right.sr", "joycon_right.r", "joycon_right.zr",
    ]
}

fn known_mapping_actions() -> &'static [&'static str] {
    &[
        "window_previous",
        "window_next",
        "focus_codex",
        "focus_next",
        "focus_previous",
        "activate_focused",
    ]
}

#[tauri::command]
fn load_mapping_config() -> Result<MappingConfig, String> {
    let path = mapping_config_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("Could not read mapping configuration: {error}"))?;
        let config: MappingConfig = serde_json::from_str(&content)
            .map_err(|error| format!("Invalid mapping configuration in {}: {error}", path.display()))?;
        validate_mapping_config(&config)?;
        return Ok(config);
    }
    let legacy_path = mapping_settings_path()?;
    let config = if legacy_path.exists() {
        let content = fs::read_to_string(&legacy_path)
            .map_err(|error| format!("Could not read legacy mapping settings: {error}"))?;
        migrated_mapping_config(serde_json::from_str(&content).map_err(|error| {
            format!("Invalid legacy mapping settings in {}: {error}", legacy_path.display())
        })?)
    } else {
        MappingConfig::default()
    };
    write_mapping_config(&config)?;
    Ok(config)
}

#[tauri::command]
fn save_mapping_config(config: MappingConfig) -> Result<(), String> {
    write_mapping_config(&config)
}

#[tauri::command]
fn reset_mapping_config() -> Result<MappingConfig, String> {
    let config = MappingConfig::default();
    write_mapping_config(&config)?;
    Ok(config)
}

#[tauri::command]
fn load_annotations() -> Result<Vec<Annotation>, String> {
    let path = annotation_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read annotations: {error}"))?;
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .map_err(|error| format!("Invalid annotation in {}: {error}", path.display()))
        })
        .collect()
}

#[tauri::command]
fn save_annotation(draft: AnnotationDraft) -> Result<Annotation, String> {
    let annotation = Annotation {
        version: 1,
        created_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis(),
        controller: draft.controller,
        previous_report: draft.previous_report,
        report: draft.report,
        label: draft.label,
    };
    let path = annotation_path()?;
    let serialized = serde_json::to_string(&annotation)
        .map_err(|error| format!("Could not encode annotation: {error}"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("Could not open {}: {error}", path.display()))?;
    writeln!(file, "{serialized}")
        .map_err(|error| format!("Could not write annotation: {error}"))?;
    Ok(annotation)
}

/// Native Joy-Con 0x30 reports carry packed 12-bit axes. macOS's generic HID
/// driver currently exposes paired Joy-Con (L) through a compact 0x3f report.
fn decode_joycon_report(bytes: &[u8], product_id: u16) -> (Option<Stick>, Option<Stick>, Option<[u8; 3]>) {
    if bytes.len() >= 12 && bytes[0] == 0x3f {
        let decode_macos_axis = |offset: usize| {
            let x = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
            let y = u16::from_le_bytes([bytes[offset + 2], bytes[offset + 3]]);
            Stick {
                x,
                y,
                normalized_x: (f32::from(x) - 32768.0) / 32767.0,
                normalized_y: (f32::from(y) - 32768.0) / 32767.0,
            }
        };
        // Byte 1: Joy-Con (L) D-pad bitfield: left 01, down 02, up 04, right 08.
        // Byte 3: stick encoded as an eight-way HAT. The visual Joy-Con is
        // vertical, hence this is a 90-degree clockwise rotation of horizontal grip.
        let stick_from_hat = |hat: u8| {
            let (normalized_x, normalized_y) = match hat {
                0 => (1.0, 0.0),
                1 => (0.707, 0.707),
                2 => (0.0, 1.0),
                3 => (-0.707, 0.707),
                4 => (-1.0, 0.0),
                5 => (-0.707, -0.707),
                6 => (0.0, -1.0),
                7 => (0.707, -0.707),
                _ => (0.0, 0.0),
            };
            Stick {
                x: ((normalized_x + 1.0) * 32767.5) as u16,
                y: ((normalized_y + 1.0) * 32767.5) as u16,
                normalized_x,
                normalized_y,
            }
        };
        let hat_stick = stick_from_hat(bytes[3]);
        let axes_stick = decode_macos_axis(4);
        return if product_id == JOYCON_RIGHT_PRODUCT_ID {
            (Some(axes_stick), Some(hat_stick), Some([bytes[1], bytes[2], bytes[3]]))
        } else {
            (Some(hat_stick), Some(axes_stick), Some([bytes[1], bytes[2], bytes[3]]))
        };
    }
    if bytes.len() < 12 || bytes[0] != 0x30 {
        return (None, None, None);
    }
    let decode_stick = |offset: usize| {
        let x = u16::from(bytes[offset]) | (u16::from(bytes[offset + 1] & 0x0f) << 8);
        let y = (u16::from(bytes[offset + 1]) >> 4) | (u16::from(bytes[offset + 2]) << 4);
        Stick {
            x,
            y,
            normalized_x: (f32::from(x) - 2048.0) / 2048.0,
            normalized_y: (f32::from(y) - 2048.0) / 2048.0,
        }
    };
    // Native Joy-Con Y grows upward, while the left visualizer uses screen
    // coordinates where Y grows downward. X already matches the portrait UI.
    let mut left_stick = decode_stick(6);
    left_stick.normalized_y = -left_stick.normalized_y;
    (
        Some(left_stick),
        Some(decode_stick(9)),
        Some([bytes[3], bytes[4], bytes[5]]),
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(StreamState::default())
        .manage(MappingRuntimeState::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_joycons,
            start_joycon_stream,
            stop_joycon_stream,
            set_mapping_runtime,
            switch_window,
            test_joycon_vibration,
            mapping_accessibility_status,
            open_accessibility_settings,
            load_mapping_config,
            save_mapping_config,
            reset_mapping_config,
            load_annotations,
            save_annotation
        ])
        .run(tauri::generate_context!())
        .expect("error while running VibeCon");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_mapping_config_is_valid() {
        assert!(validate_mapping_config(&MappingConfig::default()).is_ok());
    }

    #[test]
    fn mapping_config_rejects_unknown_automation() {
        let mut config = MappingConfig::default();
        config.presets[0].bindings[0].action = "run_shell_command".to_owned();
        assert!(validate_mapping_config(&config)
            .unwrap_err()
            .contains("Unsupported mapping action"));
    }

    #[test]
    fn mapping_config_rejects_unknown_controls() {
        let mut config = MappingConfig::default();
        config.presets[0].bindings[0].control = "joycon_left.magic_button".to_owned();
        assert!(validate_mapping_config(&config)
            .unwrap_err()
            .contains("Unsupported mapping control"));
    }

    #[test]
    fn rumble_report_uses_a_masked_counter_and_both_frames() {
        let frame = [0x11, 0x22, 0x33, 0x44];
        assert_eq!(
            rumble_report(0xa3, frame),
            [0x10, 0x03, 0x11, 0x22, 0x33, 0x44, 0x11, 0x22, 0x33, 0x44]
        );
    }

    #[test]
    fn rumble_enable_subcommand_has_neutral_frames() {
        let report = rumble_subcommand(0x1f, 0x48, 0x01);
        assert_eq!(report[0], 0x01);
        assert_eq!(report[1], 0x0f);
        assert_eq!(&report[2..10], &[0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40]);
        assert_eq!(&report[10..], &[0x48, 0x01]);
    }
}
