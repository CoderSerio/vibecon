use hidapi::HidApi;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
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
const WINDOW_SWITCH_TRIGGER_THRESHOLD: f32 = 0.60;

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
}

impl Default for StreamState {
    fn default() -> Self {
        Self {
            active_ids: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

#[derive(Clone, Default)]
struct MappingRuntimeState {
    window_switch_active: Arc<AtomicBool>,
    focus_codex_active: Arc<AtomicBool>,
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

#[derive(Deserialize, Serialize)]
struct MappingSettings {
    window_switch_enabled: bool,
    #[serde(default)]
    focus_codex_enabled: bool,
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
            let device = info
                .open_device(&api)
                .map_err(|error| format!("Could not open Joy-Con input: {error}"))?;
            let mut buffer = [0_u8; 64];
            // Keep the frontend responsive even when Bluetooth HID reports at
            // a much higher rate than the WebView can render.
            let mut last_emitted_at = Instant::now() - Duration::from_secs(1);
            let mut window_switch_armed = true;
            let mut last_window_switch_at = Instant::now() - Duration::from_secs(1);
            let mut focus_codex_armed = true;
            while stream_state.active_ids.lock().map(|ids| ids.contains(&id)).unwrap_or(false) {
                let count = device
                    .read_timeout(&mut buffer, 8)
                    .map_err(|error| format!("Could not read Joy-Con input: {error}"))?;
                if count > 0 && last_emitted_at.elapsed() >= Duration::from_millis(16) {
                    let report = report_from_bytes(buffer[..count].to_vec(), info.product_id());
                    if info.product_id() == JOYCON_LEFT_PRODUCT_ID
                        && mapping_state.window_switch_active.load(Ordering::Relaxed)
                    {
                        maybe_switch_window(
                            report.left_stick.as_ref(),
                            &mut window_switch_armed,
                            &mut last_window_switch_at,
                        );
                    }
                    if mapping_state.focus_codex_active.load(Ordering::Relaxed) {
                        maybe_focus_codex(
                            &report,
                            info.product_id(),
                            &mut focus_codex_armed,
                        );
                    }
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
    });
    Ok(())
}

fn maybe_focus_codex(report: &InputReport, product_id: u16, armed: &mut bool) {
    let Some(buttons) = report.buttons else { return };
    let pressed = match (report.report_id, product_id) {
        // Native layout: byte 5 is L controls (D-pad up 0x02); byte 3 is R
        // controls (X 0x02). We restrict the mapping to the matching device.
        (0x30, JOYCON_LEFT_PRODUCT_ID) => buttons[2] & 0x02 != 0,
        (0x30, JOYCON_RIGHT_PRODUCT_ID) => buttons[0] & 0x02 != 0,
        (0x3f, JOYCON_LEFT_PRODUCT_ID) => buttons[0] & 0x04 != 0,
        (0x3f, JOYCON_RIGHT_PRODUCT_ID) => buttons[0] & 0x02 != 0,
        _ => false,
    };
    if !pressed {
        *armed = true;
        return;
    }
    if *armed {
        *armed = false;
        let _ = focus_codex();
    }
}

fn maybe_switch_window(
    stick: Option<&Stick>,
    armed: &mut bool,
    last_switch_at: &mut Instant,
) {
    let Some(stick) = stick else { return };
    if stick.normalized_x.abs() < 0.35 {
        *armed = true;
        return;
    }
    if !*armed
        || stick.normalized_x.abs() < WINDOW_SWITCH_TRIGGER_THRESHOLD
        || last_switch_at.elapsed() < Duration::from_millis(300)
    {
        return;
    }
    *armed = false;
    *last_switch_at = Instant::now();
    let direction = if stick.normalized_x > 0.0 { "next" } else { "previous" };
    let _ = switch_window(direction.to_owned());
}

#[tauri::command]
fn stop_joycon_stream(state: tauri::State<StreamState>, id: Option<String>) {
    if let Ok(mut ids) = state.active_ids.lock() {
        if let Some(id) = id { ids.remove(&id); } else { ids.clear(); }
    }
}

#[tauri::command]
fn set_window_switch_active(state: tauri::State<MappingRuntimeState>, active: bool) {
    state.window_switch_active.store(active, Ordering::Relaxed);
}

#[tauri::command]
fn set_focus_codex_active(state: tauri::State<MappingRuntimeState>, active: bool) {
    state.focus_codex_active.store(active, Ordering::Relaxed);
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

#[tauri::command]
fn load_mapping_settings() -> Result<MappingSettings, String> {
    let path = mapping_settings_path()?;
    if !path.exists() {
        return Ok(MappingSettings::default());
    }
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read mapping settings: {error}"))?;
    serde_json::from_str(&content)
        .map_err(|error| format!("Invalid mapping settings in {}: {error}", path.display()))
}

#[tauri::command]
fn save_mapping_settings(settings: MappingSettings) -> Result<(), String> {
    let path = mapping_settings_path()?;
    let serialized = serde_json::to_string_pretty(&settings)
        .map_err(|error| format!("Could not encode mapping settings: {error}"))?;
    fs::write(&path, format!("{serialized}\n"))
        .map_err(|error| format!("Could not write mapping settings: {error}"))
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
            set_window_switch_active,
            set_focus_codex_active,
            switch_window,
            mapping_accessibility_status,
            open_accessibility_settings,
            load_mapping_settings,
            save_mapping_settings,
            load_annotations,
            save_annotation
        ])
        .run(tauri::generate_context!())
        .expect("error while running VibeCon");
}
