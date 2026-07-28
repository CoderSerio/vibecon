use hidapi::HidApi;
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::Emitter;

const NINTENDO_VENDOR_ID: u16 = 0x057e;
const JOYCON_LEFT_PRODUCT_ID: u16 = 0x2006;
const JOYCON_RIGHT_PRODUCT_ID: u16 = 0x2007;

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
    generation: Arc<AtomicU64>,
}

impl Default for StreamState {
    fn default() -> Self {
        Self {
            generation: Arc::new(AtomicU64::new(0)),
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

fn report_from_bytes(bytes: Vec<u8>) -> InputReport {
    let (left_stick, right_stick, buttons) = decode_joycon_report(&bytes);
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
    id: String,
) -> Result<(), String> {
    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let stream_state = state.inner().clone();
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
            while stream_state.generation.load(Ordering::SeqCst) == generation {
                let count = device
                    .read_timeout(&mut buffer, 8)
                    .map_err(|error| format!("Could not read Joy-Con input: {error}"))?;
                if count > 0 {
                    let event = StreamEvent {
                        device_id: id.clone(),
                        report: report_from_bytes(buffer[..count].to_vec()),
                    };
                    let _ = app.emit("joycon-input", event);
                }
            }
            Ok(())
        })();
        if let Err(message) = result {
            let _ = app.emit("joycon-stream-error", message);
        }
    });
    Ok(())
}

#[tauri::command]
fn stop_joycon_stream(state: tauri::State<StreamState>) {
    state.generation.fetch_add(1, Ordering::SeqCst);
}

fn annotation_path() -> Result<PathBuf, String> {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or("Could not determine the home directory for VibeCon annotations")?;
    let directory = PathBuf::from(home).join(".vibecon");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create ~/.vibecon: {error}"))?;
    Ok(directory.join("annotations.jsonl"))
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
fn decode_joycon_report(bytes: &[u8]) -> (Option<Stick>, Option<Stick>, Option<[u8; 3]>) {
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
        return (
            Some(stick_from_hat(bytes[3])),
            Some(decode_macos_axis(4)),
            Some([bytes[1], bytes[2], bytes[3]]),
        );
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
    (
        Some(decode_stick(6)),
        Some(decode_stick(9)),
        Some([bytes[3], bytes[4], bytes[5]]),
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(StreamState::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_joycons,
            start_joycon_stream,
            stop_joycon_stream,
            load_annotations,
            save_annotation
        ])
        .run(tauri::generate_context!())
        .expect("error while running VibeCon");
}
