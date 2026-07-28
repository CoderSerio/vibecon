use hidapi::HidApi;
use serde::Serialize;

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

#[derive(Serialize)]
struct Stick {
    x: u16,
    y: u16,
    normalized_x: f32,
    normalized_y: f32,
}

#[derive(Serialize)]
struct InputReport {
    report_id: u8,
    bytes: Vec<u8>,
    left_stick: Option<Stick>,
    right_stick: Option<Stick>,
    buttons: Option<[u8; 3]>,
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

#[tauri::command]
fn poll_joycon_input(id: String) -> Result<Option<InputReport>, String> {
    let api = open_api()?;
    let info = api
        .device_list()
        .find(|device| device.path().to_string_lossy() == id)
        .ok_or("The selected Joy-Con is no longer connected")?;
    let device = info
        .open_device(&api)
        .map_err(|error| format!("Could not open Joy-Con input: {error}"))?;

    let mut buffer = [0_u8; 64];
    let count = device
        .read_timeout(&mut buffer, 45)
        .map_err(|error| format!("Could not read Joy-Con input: {error}"))?;
    if count == 0 {
        return Ok(None);
    }

    let bytes = buffer[..count].to_vec();
    let (left_stick, right_stick, buttons) = decode_standard_report(&bytes);
    Ok(Some(InputReport {
        report_id: bytes[0],
        bytes,
        left_stick,
        right_stick,
        buttons,
    }))
}

/// Joy-Con's standard 0x30 report carries two packed 12-bit stick values.
/// Keeping the complete report in the response makes this useful even when
/// macOS exposes a different report mode or a non-standard controller.
fn decode_standard_report(bytes: &[u8]) -> (Option<Stick>, Option<Stick>, Option<[u8; 3]>) {
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
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![list_joycons, poll_joycon_input])
        .run(tauri::generate_context!())
        .expect("error while running VibeCon");
}
