use fusion_ahrs::{Ahrs, AhrsSettings, Convention, Offset, OffsetSettings};
use hidapi::HidApi;
use nalgebra::Vector3;
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

use tauri::Emitter;

mod platform_pointer;
use platform_pointer::PointerButton;

const NINTENDO_VENDOR_ID: u16 = 0x057e;
const JOYCON_LEFT_PRODUCT_ID: u16 = 0x2006;
const JOYCON_RIGHT_PRODUCT_ID: u16 = 0x2007;
const STICK_TRIGGER_THRESHOLD: f32 = 0.40;
const MAPPING_COOLDOWN: Duration = Duration::from_millis(240);
const POINTER_MODE_SWITCH_GUARD: Duration = Duration::from_millis(150);
const MOTION_SWEEP_PRESETS: [f32; 5] = [30.0, 45.0, 60.0, 90.0, 120.0];
const JOYCON_IMU_SAMPLE_RATE_HZ: f32 = 208.0;
const JOYCON_ACCEL_COUNTS_PER_G: f32 = 4096.0;
const JOYCON_GYRO_COUNTS_PER_DEGREE_PER_SECOND: f32 = 16.4;

#[derive(Serialize)]
struct ControllerDevice {
    id: String,
    name: String,
    product_id: u16,
    transport: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PointerRuntimeStatus {
    active: bool,
    enabled: bool,
    mode: &'static str,
    accessibility_granted: bool,
    backend: &'static str,
    executable_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PointerMoveTestResult {
    requested_x: f64,
    requested_y: f64,
    actual_x: f64,
    actual_y: f64,
}

#[derive(Clone, Serialize)]
struct Stick {
    x: u16,
    y: u16,
    normalized_x: f32,
    normalized_y: f32,
}

/// One raw 0x30 IMU sample. Values are intentionally left uncalibrated until
/// the controller-specific factory calibration path is verified.
#[derive(Clone, Copy, Serialize)]
struct ImuSample {
    acceleration: [i16; 3],
    gyroscope: [i16; 3],
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrientationQuaternion {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrientationFrame {
    quaternion: OrientationQuaternion,
    gyroscope: [f32; 3],
    initializing: bool,
    accelerometer_ignored: bool,
    acceleration_error: f32,
    sample_period_ms: f32,
    gyroscope_speed: f32,
    gyro_offset_active: bool,
    source: &'static str,
}

#[derive(Clone, Serialize)]
struct InputReport {
    report_id: u8,
    bytes: Vec<u8>,
    left_stick: Option<Stick>,
    right_stick: Option<Stick>,
    buttons: Option<[u8; 3]>,
    imu: Option<ImuSample>,
}

#[derive(Clone, Serialize)]
struct StreamEvent {
    device_id: String,
    report: InputReport,
    orientation: Option<OrientationFrame>,
}

struct FusionOrientationTracker {
    ahrs: Ahrs,
    offset: Offset,
    last_report_at: Option<Instant>,
}

impl FusionOrientationTracker {
    fn new() -> Self {
        let settings = AhrsSettings {
            convention: Convention::Nwu,
            gain: 0.5,
            gyroscope_range: 2000.0,
            acceleration_rejection: 10.0,
            magnetic_rejection: 0.0,
            recovery_trigger_period: (JOYCON_IMU_SAMPLE_RATE_HZ * 5.0) as u32,
        };
        Self {
            ahrs: Ahrs::with_settings(settings),
            offset: Offset::new(
                OffsetSettings {
                    cutoff_frequency: 0.01,
                    timeout: 5.0,
                    // Fusion's 3°/s embedded default mistakes deliberate slow
                    // wrist turns for stationary bias. Joy-Con interaction
                    // needs a much narrower stillness definition.
                    threshold: 0.35,
                },
                JOYCON_IMU_SAMPLE_RATE_HZ,
            ),
            last_report_at: None,
        }
    }

    fn update_report(&mut self, bytes: &[u8], product_id: u16) -> Option<OrientationFrame> {
        let samples = decode_joycon_imu_samples(bytes);
        let sample_count = samples.iter().flatten().count();
        if sample_count == 0 {
            return None;
        }

        let now = Instant::now();
        let nominal_period = 1.0 / JOYCON_IMU_SAMPLE_RATE_HZ;
        let sample_period = self
            .last_report_at
            .map(|previous| previous.elapsed().as_secs_f32() / sample_count as f32)
            .unwrap_or(nominal_period)
            // Ignore scheduler stalls and malformed timestamps without hiding
            // the normal Bluetooth report jitter from the estimator.
            .clamp(nominal_period * 0.45, nominal_period * 2.5);
        self.last_report_at = Some(now);

        let mut gyroscope = Vector3::zeros();
        for sample in samples.into_iter().flatten() {
            let (sample_gyroscope, acceleration) = calibrated_imu_vectors(&sample, product_id);
            let corrected_gyroscope = self.offset.update(sample_gyroscope);
            gyroscope = corrected_gyroscope;
            self.ahrs
                .update_no_magnetometer(corrected_gyroscope, acceleration, sample_period);
        }

        let quaternion = self.ahrs.quaternion();
        let quaternion = quaternion.quaternion();
        let states = self.ahrs.internal_states();
        let flags = self.ahrs.flags();
        Some(OrientationFrame {
            quaternion: OrientationQuaternion {
                x: quaternion.i,
                y: quaternion.j,
                z: quaternion.k,
                w: quaternion.w,
            },
            gyroscope: [gyroscope.x, gyroscope.y, gyroscope.z],
            initializing: flags.initialising,
            accelerometer_ignored: states.accelerometer_ignored,
            acceleration_error: states.acceleration_error,
            sample_period_ms: sample_period * 1000.0,
            gyroscope_speed: gyroscope.norm(),
            gyro_offset_active: self.offset.is_active(),
            source: "fusion-ahrs",
        })
    }
}

fn calibrated_imu_vectors(sample: &ImuSample, product_id: u16) -> (Vector3<f32>, Vector3<f32>) {
    let mut acceleration = Vector3::new(
        f32::from(sample.acceleration[0]) / JOYCON_ACCEL_COUNTS_PER_G,
        f32::from(sample.acceleration[1]) / JOYCON_ACCEL_COUNTS_PER_G,
        f32::from(sample.acceleration[2]) / JOYCON_ACCEL_COUNTS_PER_G,
    );
    let mut gyroscope = Vector3::new(
        f32::from(sample.gyroscope[0]) / JOYCON_GYRO_COUNTS_PER_DEGREE_PER_SECOND,
        f32::from(sample.gyroscope[1]) / JOYCON_GYRO_COUNTS_PER_DEGREE_PER_SECOND,
        f32::from(sample.gyroscope[2]) / JOYCON_GYRO_COUNTS_PER_DEGREE_PER_SECOND,
    );

    // The right Joy-Con's chip is mirrored around X relative to the left.
    // First normalize that hardware difference into Nintendo's shared JSL
    // frame, whose upright axis is +Y.
    if product_id == JOYCON_RIGHT_PRODUCT_ID {
        acceleration.y = -acceleration.y;
        acceleration.z = -acceleration.z;
        gyroscope.y = -gyroscope.y;
        gyroscope.z = -gyroscope.z;
    }

    // Fusion's NWU convention is Z-up. Rotate the Joy-Con's Y-up body frame
    // +90 degrees around X: out = (x, -z, y). The renderer later applies its
    // own fixed Fusion-body -> GLB-model basis transform.
    let to_fusion_body = |vector: Vector3<f32>| Vector3::new(vector.x, -vector.z, vector.y);
    (to_fusion_body(gyroscope), to_fusion_body(acceleration))
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
    pointer_mode: Arc<AtomicU8>,
    pointer_mode_changed_at: Arc<Mutex<Instant>>,
}

impl Default for MappingRuntimeState {
    fn default() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
            config: Arc::new(Mutex::new(MappingConfig::default())),
            pointer_mode: Arc::new(AtomicU8::new(PointerMode::Stick as u8)),
            pointer_mode_changed_at: Arc::new(Mutex::new(
                Instant::now() - POINTER_MODE_SWITCH_GUARD,
            )),
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

#[derive(Clone, Default, Deserialize, Serialize)]
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PointerMode {
    Stick = 0,
    Motion = 1,
}

impl PointerMode {
    fn from_u8(value: u8) -> Self {
        if value == Self::Motion as u8 {
            Self::Motion
        } else {
            Self::Stick
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Stick => "stick",
            Self::Motion => "motion",
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StickPointerConfig {
    deadzone: f32,
    max_speed: f32,
    acceleration: f32,
}

impl Default for StickPointerConfig {
    fn default() -> Self {
        Self {
            deadzone: 0.12,
            max_speed: 1400.0,
            acceleration: 1.6,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MotionPointerConfig {
    #[serde(default = "default_motion_sweep_degrees")]
    sweep_degrees: f32,
    vertical_ratio: f32,
    noise_threshold: f32,
}

fn default_motion_sweep_degrees() -> f32 {
    60.0
}

impl Default for MotionPointerConfig {
    fn default() -> Self {
        Self {
            sweep_degrees: default_motion_sweep_degrees(),
            vertical_ratio: 0.85,
            noise_threshold: 0.015,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PointerConfig {
    enabled: bool,
    mode: PointerMode,
    mode_switch_hold_ms: u64,
    stick: StickPointerConfig,
    motion: MotionPointerConfig,
}

impl Default for PointerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: PointerMode::Stick,
            mode_switch_hold_ms: 600,
            stick: StickPointerConfig::default(),
            motion: MotionPointerConfig::default(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MappingConfig {
    version: u8,
    active_preset_id: String,
    presets: Vec<MappingPreset>,
    #[serde(default)]
    pointer: PointerConfig,
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
            version: 3,
            active_preset_id: "codex-cowork".to_owned(),
            presets: vec![
                MappingPreset {
                    id: "codex-cowork".to_owned(),
                    name: "Codex Cowork".to_owned(),
                    enabled: true,
                    bindings: vec![
                        mapping_binding(
                            "window-previous",
                            "joycon_left.stick_left",
                            "window_previous",
                        ),
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
            ],
            pointer: PointerConfig::default(),
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

struct PointerDeviceState {
    last_mode: PointerMode,
    last_report_at: Option<Instant>,
    motion_origin_orientation: Option<[f32; 4]>,
    previous_motion_angles: Option<(f32, f32)>,
    mode_switch_started_at: Option<Instant>,
    mode_switch_fired: bool,
    sensitivity_decrease_pressed: bool,
    sensitivity_increase_pressed: bool,
    last_motion_at: Option<Instant>,
    display_size: Option<(f32, f32)>,
    display_size_checked_at: Option<Instant>,
    left_button_down: bool,
    right_button_down: bool,
    subpixel_x: f32,
    subpixel_y: f32,
    permission_error_reported: bool,
}

struct PointerFeedbackContext<'a> {
    device: &'a Arc<Mutex<hidapi::HidDevice>>,
    stream_state: &'a StreamState,
}

impl Default for PointerDeviceState {
    fn default() -> Self {
        Self {
            last_mode: PointerMode::Stick,
            last_report_at: None,
            motion_origin_orientation: None,
            previous_motion_angles: None,
            mode_switch_started_at: None,
            mode_switch_fired: false,
            sensitivity_decrease_pressed: false,
            sensitivity_increase_pressed: false,
            last_motion_at: None,
            display_size: None,
            display_size_checked_at: None,
            left_button_down: false,
            right_button_down: false,
            subpixel_x: 0.0,
            subpixel_y: 0.0,
            permission_error_reported: false,
        }
    }
}

impl PointerDeviceState {
    fn reset_motion(&mut self, orientation: Option<&OrientationFrame>) {
        self.motion_origin_orientation = orientation.map(orientation_components);
        self.previous_motion_angles = orientation.map(|_| (0.0, 0.0));
        self.last_motion_at = None;
        self.subpixel_x = 0.0;
        self.subpixel_y = 0.0;
    }

    fn reset_all(&mut self, orientation: Option<&OrientationFrame>) {
        self.last_report_at = None;
        self.reset_motion(orientation);
    }
}

fn orientation_components(frame: &OrientationFrame) -> [f32; 4] {
    [
        frame.quaternion.x,
        frame.quaternion.y,
        frame.quaternion.z,
        frame.quaternion.w,
    ]
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
    let (left_stick, right_stick, buttons, imu) = decode_joycon_report(&bytes, product_id);
    InputReport {
        report_id: bytes[0],
        bytes,
        left_stick,
        right_stick,
        buttons,
        imu,
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
    if !state
        .active_ids
        .lock()
        .map_err(|_| "Input stream state is unavailable")?
        .insert(id.clone())
    {
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
            let device =
                Arc::new(Mutex::new(info.open_device(&api).map_err(|error| {
                    format!("Could not open Joy-Con input: {error}")
                })?));
            stream_state
                .devices
                .lock()
                .map_err(|_| "Joy-Con output state is unavailable")?
                .insert(id.clone(), device.clone());
            if let Err(error) = configure_joycon_motion(&device, &stream_state) {
                // Motion is an optional capability. Keep the input stream alive
                // so button and stick debugging still works on devices or HID
                // transports that reject native Joy-Con subcommands.
                let _ = app.emit(
                    "joycon-stream-error",
                    format!("Joy-Con motion could not be enabled; ordinary input is still available: {error}"),
                );
            }
            let mut buffer = [0_u8; 64];
            // Keep the frontend responsive even when Bluetooth HID reports at
            // a much higher rate than the WebView can render.
            let mut last_emitted_at = Instant::now() - Duration::from_secs(1);
            let mut binding_states = HashMap::<String, BindingTriggerState>::new();
            let mut pointer_state = PointerDeviceState::default();
            let mut orientation_tracker = FusionOrientationTracker::new();
            while stream_state
                .active_ids
                .lock()
                .map(|ids| ids.contains(&id))
                .unwrap_or(false)
            {
                let count = device
                    .lock()
                    .map_err(|_| "Joy-Con device handle is unavailable")?
                    .read_timeout(&mut buffer, 8)
                    .map_err(|error| format!("Could not read Joy-Con input: {error}"))?;
                if count > 0 {
                    let report = report_from_bytes(buffer[..count].to_vec(), info.product_id());
                    // Consume every native IMU report and all three sub-samples
                    // even though WebView presentation remains capped below.
                    // Dropping samples before fusion causes visible lag and
                    // permanently loses part of fast rotations.
                    let orientation =
                        orientation_tracker.update_report(&report.bytes, info.product_id());
                    process_mapping_report(
                        &report,
                        info.product_id(),
                        &mapping_state,
                        &mut binding_states,
                    );
                    process_pointer_report(
                        &app,
                        &report,
                        orientation.as_ref(),
                        info.product_id(),
                        &mapping_state,
                        &mut pointer_state,
                        PointerFeedbackContext {
                            device: &device,
                            stream_state: &stream_state,
                        },
                    );
                    if last_emitted_at.elapsed() >= Duration::from_millis(16) {
                        let event = StreamEvent {
                            device_id: id.clone(),
                            report,
                            orientation,
                        };
                        let _ = app.emit("joycon-input", event);
                        last_emitted_at = Instant::now();
                    }
                }
            }
            release_pointer_buttons(&mut pointer_state);
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
    let Some(preset) = config.active_preset() else {
        return;
    };
    if !preset.enabled {
        return;
    }
    for binding in preset.bindings.iter().filter(|binding| binding.enabled) {
        if config.pointer.enabled
            && matches!(
                binding.control.as_str(),
                "joycon_left.stick_left" | "joycon_left.stick_right"
            )
        {
            continue;
        }
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

fn process_pointer_report(
    app: &tauri::AppHandle,
    report: &InputReport,
    orientation: Option<&OrientationFrame>,
    product_id: u16,
    mapping_state: &MappingRuntimeState,
    state: &mut PointerDeviceState,
    feedback: PointerFeedbackContext<'_>,
) {
    let active = mapping_state.active.load(Ordering::Relaxed);
    let mut pointer = match mapping_state.config.lock() {
        Ok(config) => config.pointer.clone(),
        Err(_) => return,
    };
    if !active || !pointer.enabled {
        release_pointer_buttons(state);
        state.reset_all(orientation);
        state.mode_switch_started_at = None;
        state.mode_switch_fired = false;
        state.sensitivity_decrease_pressed = false;
        state.sensitivity_increase_pressed = false;
        return;
    }

    let controls = pressed_button_controls(report, product_id);
    let mode_switch_control = if product_id == JOYCON_RIGHT_PRODUCT_ID {
        "joycon_right.plus"
    } else {
        "joycon_left.minus"
    };
    let mode_switch_pressed = controls.contains(&mode_switch_control);
    if mode_switch_pressed {
        let started_at = state
            .mode_switch_started_at
            .get_or_insert_with(Instant::now);
        if !state.mode_switch_fired
            && started_at.elapsed() >= Duration::from_millis(pointer.mode_switch_hold_ms)
        {
            state.mode_switch_fired = true;
            let can_toggle = mapping_state
                .pointer_mode_changed_at
                .lock()
                .map(|changed_at| changed_at.elapsed() >= POINTER_MODE_SWITCH_GUARD)
                .unwrap_or(false);
            if can_toggle {
                let current =
                    PointerMode::from_u8(mapping_state.pointer_mode.load(Ordering::Relaxed));
                let next = if current == PointerMode::Stick {
                    PointerMode::Motion
                } else {
                    PointerMode::Stick
                };
                mapping_state
                    .pointer_mode
                    .store(next as u8, Ordering::Relaxed);
                let persist_result = update_runtime_pointer_config(mapping_state, |pointer| {
                    pointer.mode = next;
                });
                if let Ok(mut changed_at) = mapping_state.pointer_mode_changed_at.lock() {
                    *changed_at = Instant::now();
                }
                release_pointer_buttons(state);
                state.last_mode = next;
                state.reset_all(orientation);
                let _ = app.emit("pointer-mode-changed", next.as_str());
                if let Err(error) = persist_result {
                    let _ = app.emit(
                        "joycon-stream-error",
                        format!("Could not persist the pointer mode: {error}"),
                    );
                }
            }
        }
    } else {
        let short_press = state
            .mode_switch_started_at
            .take()
            .is_some_and(|started_at| {
                !state.mode_switch_fired
                    && started_at.elapsed() < Duration::from_millis(pointer.mode_switch_hold_ms)
            });
        state.mode_switch_fired = false;
        if short_press
            && PointerMode::from_u8(mapping_state.pointer_mode.load(Ordering::Relaxed))
                == PointerMode::Motion
        {
            state.reset_motion(orientation);
            let _ = app.emit("pointer-recentered", product_id);
        }
    }

    let mode = PointerMode::from_u8(mapping_state.pointer_mode.load(Ordering::Relaxed));
    if mode != state.last_mode {
        release_pointer_buttons(state);
        state.last_mode = mode;
        state.reset_all(orientation);
    }

    let (decrease_control, increase_control) = if product_id == JOYCON_RIGHT_PRODUCT_ID {
        ("joycon_right.sl", "joycon_right.sr")
    } else {
        ("joycon_left.sl", "joycon_left.sr")
    };
    let decrease_pressed = controls.contains(&decrease_control);
    let increase_pressed = controls.contains(&increase_control);
    let decrease_edge = decrease_pressed && !state.sensitivity_decrease_pressed;
    let increase_edge = increase_pressed && !state.sensitivity_increase_pressed;
    state.sensitivity_decrease_pressed = decrease_pressed;
    state.sensitivity_increase_pressed = increase_pressed;
    if mode == PointerMode::Motion && decrease_edge != increase_edge {
        let next_sweep = adjusted_motion_sweep(pointer.motion.sweep_degrees, increase_edge);
        if (next_sweep - pointer.motion.sweep_degrees).abs() > f32::EPSILON {
            let persist_result = update_runtime_pointer_config(mapping_state, |pointer| {
                pointer.motion.sweep_degrees = next_sweep;
            });
            pointer.motion.sweep_degrees = next_sweep;
            let _ = app.emit("pointer-sweep-changed", next_sweep);
            send_pointer_feedback_rumble(feedback.device.clone(), feedback.stream_state.clone());
            if let Err(error) = persist_result {
                let _ = app.emit(
                    "joycon-stream-error",
                    format!("Could not persist motion sensitivity: {error}"),
                );
            }
        }
    }
    if mapping_state
        .pointer_mode_changed_at
        .lock()
        .map(|changed_at| changed_at.elapsed() < POINTER_MODE_SWITCH_GUARD)
        .unwrap_or(true)
    {
        return;
    }

    if !platform_pointer::accessibility_granted() {
        release_pointer_buttons(state);
        state.reset_all(orientation);
        if !state.permission_error_reported {
            state.permission_error_reported = true;
            let _ = app.emit(
                "pointer-runtime-error",
                "Mouse control is blocked by macOS Accessibility. Grant access to this running VibeCon build, then quit and reopen it.",
            );
        }
        return;
    }
    state.permission_error_reported = false;

    let (shoulder, trigger) = if product_id == JOYCON_RIGHT_PRODUCT_ID {
        (
            controls.contains(&"joycon_right.r"),
            controls.contains(&"joycon_right.zr"),
        )
    } else {
        (
            controls.contains(&"joycon_left.l"),
            controls.contains(&"joycon_left.zl"),
        )
    };
    let (left_down, right_down) = match mode {
        PointerMode::Stick => (shoulder, trigger),
        PointerMode::Motion => (shoulder && !trigger, shoulder && trigger),
    };
    let output_result = (|| {
        update_pointer_button(state, PointerButton::Left, left_down)?;
        update_pointer_button(state, PointerButton::Right, right_down)?;

        match mode {
            PointerMode::Stick => process_stick_pointer(report, product_id, &pointer.stick, state),
            PointerMode::Motion => {
                if trigger {
                    state.reset_motion(orientation);
                    Ok(())
                } else {
                    process_motion_pointer(orientation, product_id, &pointer.motion, state)
                }
            }
        }
    })();
    if let Err(error) = output_result {
        state.reset_all(orientation);
        if !state.permission_error_reported {
            state.permission_error_reported = true;
            let _ = app.emit(
                "pointer-runtime-error",
                format!("Mouse output failed: {error}"),
            );
        }
    } else {
        state.permission_error_reported = false;
    }
}

fn process_stick_pointer(
    report: &InputReport,
    product_id: u16,
    config: &StickPointerConfig,
    state: &mut PointerDeviceState,
) -> Result<(), String> {
    let stick = if product_id == JOYCON_RIGHT_PRODUCT_ID {
        report.right_stick.as_ref()
    } else {
        report.left_stick.as_ref()
    };
    let Some(stick) = stick else {
        state.last_report_at = None;
        return Ok(());
    };
    let now = Instant::now();
    let elapsed = state
        .last_report_at
        .map(|previous| now.duration_since(previous).as_secs_f32())
        .unwrap_or(0.0)
        .clamp(0.0, 0.05);
    state.last_report_at = Some(now);
    if elapsed == 0.0 {
        return Ok(());
    }
    let screen_y = if product_id == JOYCON_RIGHT_PRODUCT_ID {
        -stick.normalized_y
    } else {
        stick.normalized_y
    };
    let (x, y) = pointer_stick_velocity(
        stick.normalized_x,
        screen_y,
        config.deadzone,
        config.max_speed,
        config.acceleration,
    );
    post_pointer_delta(x * elapsed, y * elapsed, state)
}

fn pointer_stick_velocity(
    x: f32,
    y: f32,
    deadzone: f32,
    max_speed: f32,
    acceleration: f32,
) -> (f32, f32) {
    let magnitude = x.hypot(y).min(1.0);
    let deadzone = deadzone.clamp(0.0, 0.95);
    if magnitude <= deadzone || magnitude == 0.0 {
        return (0.0, 0.0);
    }
    let normalized = ((magnitude - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0);
    let speed = max_speed.max(0.0) * normalized.powf(acceleration.max(0.1));
    (x / magnitude * speed, y / magnitude * speed)
}

fn adjusted_motion_sweep(current: f32, increase_sensitivity: bool) -> f32 {
    if increase_sensitivity {
        MOTION_SWEEP_PRESETS
            .iter()
            .rev()
            .copied()
            .find(|preset| *preset < current - 0.1)
            .unwrap_or(MOTION_SWEEP_PRESETS[0])
    } else {
        MOTION_SWEEP_PRESETS
            .iter()
            .copied()
            .find(|preset| *preset > current + 0.1)
            .unwrap_or(*MOTION_SWEEP_PRESETS.last().unwrap_or(&120.0))
    }
}

fn update_runtime_pointer_config(
    mapping_state: &MappingRuntimeState,
    update: impl FnOnce(&mut PointerConfig),
) -> Result<(), String> {
    let mut config = mapping_state
        .config
        .lock()
        .map_err(|_| "Mapping runtime is unavailable".to_owned())?;
    update(&mut config.pointer);
    write_mapping_config(&config)
}

fn motion_projection(rotation_x: f32, rotation_z: f32, product_id: u16) -> (f32, f32) {
    let horizontal_sign = if product_id == JOYCON_RIGHT_PRODUCT_ID {
        -1.0
    } else {
        1.0
    };
    (rotation_z * horizontal_sign, -rotation_x)
}

fn motion_angles_from_origin(current: [f32; 4], origin: [f32; 4], product_id: u16) -> (f32, f32) {
    let [rotation_x, _, rotation_z] = quaternion_delta_degrees(current, origin);
    motion_projection(rotation_x, rotation_z, product_id)
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn motion_adaptive_gain(angular_speed: f32) -> f32 {
    if angular_speed <= 10.0 {
        0.45
    } else if angular_speed < 60.0 {
        0.45 + 0.55 * smoothstep((angular_speed - 10.0) / 50.0)
    } else if angular_speed < 180.0 {
        1.0 + 0.8 * smoothstep((angular_speed - 60.0) / 120.0)
    } else {
        1.8
    }
}

fn pointer_display_size(state: &mut PointerDeviceState) -> Result<(f32, f32), String> {
    let should_refresh = state
        .display_size_checked_at
        .map(|checked_at| checked_at.elapsed() >= Duration::from_secs(1))
        .unwrap_or(true);
    if should_refresh {
        state.display_size_checked_at = Some(Instant::now());
        match platform_pointer::display_size_at_cursor() {
            Ok(size) => state.display_size = Some(size),
            Err(error) if state.display_size.is_none() => return Err(error),
            Err(_) => {}
        }
    }
    state
        .display_size
        .ok_or_else(|| "Could not determine the pointer display size".to_owned())
}

fn process_motion_pointer(
    orientation: Option<&OrientationFrame>,
    product_id: u16,
    config: &MotionPointerConfig,
    state: &mut PointerDeviceState,
) -> Result<(), String> {
    let Some(orientation) = orientation else {
        return Ok(());
    };
    let current = orientation_components(orientation);
    let Some(origin) = state.motion_origin_orientation else {
        state.motion_origin_orientation = Some(current);
        state.previous_motion_angles = Some((0.0, 0.0));
        state.last_motion_at = Some(Instant::now());
        return Ok(());
    };
    let now = Instant::now();
    let elapsed = state
        .last_motion_at
        .replace(now)
        .map(|previous_at| now.duration_since(previous_at).as_secs_f32())
        .unwrap_or(0.0)
        .clamp(0.001, 0.05);
    let angles = motion_angles_from_origin(current, origin, product_id);
    let previous_angles = state
        .previous_motion_angles
        .replace(angles)
        .unwrap_or(angles);
    let horizontal = angles.0 - previous_angles.0;
    let vertical = angles.1 - previous_angles.1;
    let threshold = config.noise_threshold.max(0.0);
    let horizontal = if horizontal.abs() >= threshold {
        horizontal
    } else {
        0.0
    };
    let vertical = if vertical.abs() >= threshold {
        vertical
    } else {
        0.0
    };
    if horizontal == 0.0 && vertical == 0.0 {
        return Ok(());
    }
    let angular_speed = horizontal.hypot(vertical) / elapsed;
    let adaptive_gain = motion_adaptive_gain(angular_speed);
    let (display_width, display_height) = pointer_display_size(state)?;
    let sweep_degrees = config.sweep_degrees.clamp(30.0, 120.0);
    post_pointer_delta(
        horizontal / sweep_degrees * display_width * adaptive_gain,
        vertical / sweep_degrees * display_height * config.vertical_ratio * adaptive_gain,
        state,
    )
}

fn quaternion_delta_degrees(current: [f32; 4], previous: [f32; 4]) -> [f32; 3] {
    let inverse_previous = [-previous[0], -previous[1], -previous[2], previous[3]];
    let mut delta = multiply_quaternion(current, inverse_previous);
    if delta[3] < 0.0 {
        delta
            .iter_mut()
            .for_each(|component| *component = -*component);
    }
    let vector_length = delta[0].hypot(delta[1]).hypot(delta[2]);
    if vector_length <= f32::EPSILON {
        return [0.0; 3];
    }
    let angle = 2.0 * vector_length.atan2(delta[3]) * 180.0 / std::f32::consts::PI;
    [
        delta[0] / vector_length * angle,
        delta[1] / vector_length * angle,
        delta[2] / vector_length * angle,
    ]
}

fn multiply_quaternion(left: [f32; 4], right: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = left;
    let [bx, by, bz, bw] = right;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

fn update_pointer_button(
    state: &mut PointerDeviceState,
    button: PointerButton,
    down: bool,
) -> Result<(), String> {
    let slot = match button {
        PointerButton::Left => &mut state.left_button_down,
        PointerButton::Right => &mut state.right_button_down,
    };
    if *slot == down {
        return Ok(());
    }
    platform_pointer::post_button(button, down)?;
    *slot = down;
    Ok(())
}

fn release_pointer_buttons(state: &mut PointerDeviceState) {
    let _ = update_pointer_button(state, PointerButton::Left, false);
    let _ = update_pointer_button(state, PointerButton::Right, false);
}

fn post_pointer_delta(dx: f32, dy: f32, state: &mut PointerDeviceState) -> Result<(), String> {
    state.subpixel_x += dx;
    state.subpixel_y += dy;
    if state.subpixel_x.abs() < 0.35 && state.subpixel_y.abs() < 0.35 {
        return Ok(());
    }
    let whole_x = state.subpixel_x;
    let whole_y = state.subpixel_y;
    platform_pointer::post_move(whole_x, whole_y, state.left_button_down)?;
    state.subpixel_x = 0.0;
    state.subpixel_y = 0.0;
    Ok(())
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
        _ => pressed_button_controls(report, product_id).contains(&control),
    }
}

fn pressed_button_controls(report: &InputReport, product_id: u16) -> Vec<&'static str> {
    let Some([buttons, extra, left]) = report.buttons else {
        return vec![];
    };
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
            add_bits(
                left,
                &[
                    ("joycon_left.dpad_down", 0x01),
                    ("joycon_left.dpad_up", 0x02),
                    ("joycon_left.dpad_right", 0x04),
                    ("joycon_left.dpad_left", 0x08),
                    ("joycon_left.sr", 0x10),
                    ("joycon_left.sl", 0x20),
                    ("joycon_left.l", 0x40),
                    ("joycon_left.zl", 0x80),
                ],
            );
            add_bits(
                extra,
                &[
                    ("joycon_left.minus", 0x01),
                    ("joycon_left.stick_press", 0x04),
                    ("joycon_left.capture", 0x20),
                ],
            );
        }
        (0x30, JOYCON_RIGHT_PRODUCT_ID) => {
            add_bits(
                buttons,
                &[
                    ("joycon_right.y", 0x01),
                    ("joycon_right.x", 0x02),
                    ("joycon_right.b", 0x04),
                    ("joycon_right.a", 0x08),
                    ("joycon_right.sr", 0x10),
                    ("joycon_right.sl", 0x20),
                    ("joycon_right.r", 0x40),
                    ("joycon_right.zr", 0x80),
                ],
            );
            add_bits(
                extra,
                &[
                    ("joycon_right.plus", 0x02),
                    ("joycon_right.stick_press", 0x08),
                    ("joycon_right.home", 0x10),
                ],
            );
        }
        (0x3f, JOYCON_LEFT_PRODUCT_ID) => {
            add_bits(
                buttons,
                &[
                    ("joycon_left.dpad_left", 0x01),
                    ("joycon_left.dpad_down", 0x02),
                    ("joycon_left.dpad_up", 0x04),
                    ("joycon_left.dpad_right", 0x08),
                    ("joycon_left.sl", 0x10),
                    ("joycon_left.sr", 0x20),
                ],
            );
            add_bits(
                extra,
                &[
                    ("joycon_left.minus", 0x01),
                    ("joycon_left.stick_press", 0x04),
                    ("joycon_left.capture", 0x20),
                    ("joycon_left.l", 0x40),
                    ("joycon_left.zl", 0x80),
                ],
            );
        }
        (0x3f, JOYCON_RIGHT_PRODUCT_ID) => {
            add_bits(
                buttons,
                &[
                    ("joycon_right.y", 0x01),
                    ("joycon_right.x", 0x02),
                    ("joycon_right.b", 0x04),
                    ("joycon_right.a", 0x08),
                    ("joycon_right.sr", 0x10),
                    ("joycon_right.sl", 0x20),
                    ("joycon_right.r", 0x40),
                    ("joycon_right.zr", 0x80),
                ],
            );
            add_bits(
                extra,
                &[
                    ("joycon_right.plus", 0x01),
                    ("joycon_right.stick_press", 0x04),
                    ("joycon_right.home", 0x20),
                ],
            );
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
    if !active || !config.pointer.enabled {
        release_system_pointer_buttons();
    }
    state
        .pointer_mode
        .store(config.pointer.mode as u8, Ordering::Relaxed);
    *state
        .config
        .lock()
        .map_err(|_| "Mapping runtime is unavailable")? = config;
    state.active.store(active, Ordering::Relaxed);
    Ok(())
}

fn release_system_pointer_buttons() {
    let _ = platform_pointer::post_button(PointerButton::Left, false);
    let _ = platform_pointer::post_button(PointerButton::Right, false);
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
        if !platform_pointer::accessibility_granted() {
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

fn joycon_subcommand(counter: u8, subcommand: u8, data: u8) -> [u8; 12] {
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

fn configure_joycon_motion(
    device: &Arc<Mutex<hidapi::HidDevice>>,
    state: &StreamState,
) -> Result<(), String> {
    let enable_imu = joycon_subcommand(next_output_packet_counter(state), 0x40, 0x01);
    write_joycon_output(device, &enable_imu)
        .map_err(|error| format!("could not enable the 6-axis sensor: {error}"))?;
    thread::sleep(Duration::from_millis(20));

    let native_reports = joycon_subcommand(next_output_packet_counter(state), 0x03, 0x30);
    write_joycon_output(device, &native_reports)
        .map_err(|error| format!("could not select native 0x30 reports: {error}"))?;
    Ok(())
}

fn rumble_report(counter: u8, frame: [u8; 4]) -> [u8; 10] {
    [
        0x10,
        counter & 0x0f,
        frame[0],
        frame[1],
        frame[2],
        frame[3],
        frame[0],
        frame[1],
        frame[2],
        frame[3],
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

fn send_pointer_feedback_rumble(device: Arc<Mutex<hidapi::HidDevice>>, state: StreamState) {
    thread::spawn(move || {
        let enable = joycon_subcommand(next_output_packet_counter(&state), 0x48, 0x01);
        let pulse = rumble_report(next_output_packet_counter(&state), GENTLE_RUMBLE_FRAME);
        let neutral = rumble_report(next_output_packet_counter(&state), NEUTRAL_RUMBLE_FRAME);
        if write_joycon_output(&device, &enable).is_err() {
            return;
        }
        thread::sleep(Duration::from_millis(12));
        if write_joycon_output(&device, &pulse).is_err() {
            return;
        }
        thread::sleep(Duration::from_millis(45));
        let _ = write_joycon_output(&device, &neutral);
    });
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
    let enable = joycon_subcommand(next_output_packet_counter(state.inner()), 0x48, 0x01);
    let pulse = rumble_report(
        next_output_packet_counter(state.inner()),
        GENTLE_RUMBLE_FRAME,
    );
    let neutral = rumble_report(
        next_output_packet_counter(state.inner()),
        NEUTRAL_RUMBLE_FRAME,
    );
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

#[tauri::command]
fn pointer_runtime_status(state: tauri::State<MappingRuntimeState>) -> PointerRuntimeStatus {
    let pointer = state
        .config
        .lock()
        .map(|config| config.pointer.clone())
        .unwrap_or_default();
    let mode = PointerMode::from_u8(state.pointer_mode.load(Ordering::Relaxed));
    PointerRuntimeStatus {
        active: state.active.load(Ordering::Relaxed),
        enabled: pointer.enabled,
        mode: mode.as_str(),
        accessibility_granted: platform_pointer::accessibility_granted(),
        backend: platform_pointer::backend_name(),
        executable_path: platform_pointer::permission_target(),
    }
}

#[tauri::command]
fn request_accessibility_permission() -> Result<bool, String> {
    platform_pointer::request_accessibility_permission()
}

fn verify_pointer_move(
    requested_x: f32,
    requested_y: f32,
) -> Result<PointerMoveTestResult, String> {
    let before = platform_pointer::cursor_location()?;
    platform_pointer::post_move(requested_x, requested_y, false)?;
    thread::sleep(Duration::from_millis(80));
    let after = platform_pointer::cursor_location()?;
    let actual_x = after.0 - before.0;
    let actual_y = after.1 - before.1;
    if actual_x.hypot(actual_y) < 1.0 {
        return Err(
            "macOS accepted the CGEvent call, but the cursor position did not change. Recheck Accessibility for this exact VibeCon build and reopen the app."
                .to_owned(),
        );
    }
    Ok(PointerMoveTestResult {
        requested_x: f64::from(requested_x),
        requested_y: f64::from(requested_y),
        actual_x,
        actual_y,
    })
}

#[tauri::command]
fn test_pointer_move() -> Result<PointerMoveTestResult, String> {
    verify_pointer_move(80.0, 60.0)
}

#[cfg(debug_assertions)]
pub fn run_pointer_self_test() -> Result<String, String> {
    if !platform_pointer::accessibility_granted() {
        return Err(format!(
            "Accessibility is not granted to this signed VibeCon development identity ({})",
            platform_pointer::permission_target(),
        ));
    }
    let result = verify_pointer_move(80.0, 60.0)?;
    Ok(format!(
        "Accessibility granted; requested ({:.0}, {:.0}) px and observed ({:.1}, {:.1}) px",
        result.requested_x, result.requested_y, result.actual_x, result.actual_y,
    ))
}

#[cfg(debug_assertions)]
pub fn request_pointer_accessibility() -> Result<String, String> {
    let granted = platform_pointer::request_accessibility_permission()?;
    Ok(if granted {
        "Accessibility is already granted to VibeCon Dev.app".to_owned()
    } else {
        format!(
            "Accessibility was requested for {}; enable VibeCon Dev in System Settings, then restart pnpm tauri dev",
            platform_pointer::permission_target(),
        )
    })
}

#[tauri::command]
fn mapping_accessibility_status() -> Result<String, String> {
    Ok(if platform_pointer::accessibility_granted() {
        "Accessibility: granted to this running VibeCon process.".to_owned()
    } else {
        format!(
            "Accessibility: not granted to this running process. Grant access to this exact build and reopen it: {}",
            platform_pointer::permission_target(),
        )
    })
}

#[tauri::command]
fn open_accessibility_settings() -> Result<(), String> {
    platform_pointer::open_accessibility_settings()
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

/// Version 3 returns the built-in library to verified mappings only.
/// User-created presets remain untouched; superseded built-ins are removed.
fn migrate_mapping_config(mut config: MappingConfig) -> Result<(MappingConfig, bool), String> {
    match config.version {
        1 => {
            config.version = 3;
            prune_unverified_builtins(&mut config);
            Ok((config, true))
        }
        2 => {
            config.version = 3;
            prune_unverified_builtins(&mut config);
            Ok((config, true))
        }
        3 => Ok((config, false)),
        _ => Err("Unsupported mapping configuration version".to_owned()),
    }
}

fn prune_unverified_builtins(config: &mut MappingConfig) {
    config
        .presets
        .retain(|preset| !matches!(preset.id.as_str(), "code" | "keyboard-focus"));
    if let Some(preset) = config
        .presets
        .iter_mut()
        .find(|preset| preset.id == "codex-cowork")
    {
        preset.bindings.retain(|binding| {
            !matches!(
                binding.id.as_str(),
                "focus-vibecon-left" | "focus-vibecon-right"
            )
        });
    }
    if !config
        .presets
        .iter()
        .any(|preset| preset.id == config.active_preset_id)
    {
        config.active_preset_id = "codex-cowork".to_owned();
    }
}

fn validate_mapping_config(config: &MappingConfig) -> Result<(), String> {
    if config.version != 3 {
        return Err("Unsupported mapping configuration version".to_owned());
    }
    if config.presets.is_empty() {
        return Err("At least one mapping preset is required".to_owned());
    }
    if !config
        .presets
        .iter()
        .any(|preset| preset.id == config.active_preset_id)
    {
        return Err("The active mapping preset does not exist".to_owned());
    }
    if !(250..=2000).contains(&config.pointer.mode_switch_hold_ms) {
        return Err("Pointer mode switch hold time must be between 250 and 2000 ms".to_owned());
    }
    let stick = &config.pointer.stick;
    if !stick.deadzone.is_finite()
        || !(0.0..=0.8).contains(&stick.deadzone)
        || !stick.max_speed.is_finite()
        || !(100.0..=4000.0).contains(&stick.max_speed)
        || !stick.acceleration.is_finite()
        || !(0.5..=4.0).contains(&stick.acceleration)
    {
        return Err("Pointer stick settings are outside their supported range".to_owned());
    }
    let motion = &config.pointer.motion;
    if !motion.sweep_degrees.is_finite()
        || !(30.0..=120.0).contains(&motion.sweep_degrees)
        || !motion.vertical_ratio.is_finite()
        || !(0.25..=2.0).contains(&motion.vertical_ratio)
        || !motion.noise_threshold.is_finite()
        || !(0.0..=0.5).contains(&motion.noise_threshold)
    {
        return Err("Pointer motion settings are outside their supported range".to_owned());
    }
    let mut preset_ids = HashSet::new();
    let mut binding_ids = HashSet::new();
    for preset in &config.presets {
        if preset.id.trim().is_empty() || !preset_ids.insert(&preset.id) {
            return Err("Each mapping preset needs a unique non-empty id".to_owned());
        }
        for binding in &preset.bindings {
            if binding.id.trim().is_empty()
                || !binding_ids.insert(format!("{}:{}", preset.id, binding.id))
            {
                return Err(format!(
                    "Preset {} has duplicate or empty binding ids",
                    preset.id
                ));
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
        "joycon_left.stick_left",
        "joycon_left.stick_right",
        "joycon_left.dpad_up",
        "joycon_left.dpad_down",
        "joycon_left.dpad_left",
        "joycon_left.dpad_right",
        "joycon_left.stick_press",
        "joycon_left.minus",
        "joycon_left.capture",
        "joycon_left.sl",
        "joycon_left.sr",
        "joycon_left.l",
        "joycon_left.zl",
        "joycon_right.x",
        "joycon_right.y",
        "joycon_right.a",
        "joycon_right.b",
        "joycon_right.stick_press",
        "joycon_right.plus",
        "joycon_right.home",
        "joycon_right.sl",
        "joycon_right.sr",
        "joycon_right.r",
        "joycon_right.zr",
    ]
}

fn known_mapping_actions() -> &'static [&'static str] {
    &["window_previous", "window_next", "focus_codex"]
}

#[tauri::command]
fn load_mapping_config() -> Result<MappingConfig, String> {
    let path = mapping_config_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("Could not read mapping configuration: {error}"))?;
        let raw_config: serde_json::Value = serde_json::from_str(&content).map_err(|error| {
            format!(
                "Invalid mapping configuration in {}: {error}",
                path.display()
            )
        })?;
        let legacy_motion_sensitivity = raw_config.pointer("/pointer/motion/sensitivity").is_some()
            && raw_config.pointer("/pointer/motion/sweepDegrees").is_none();
        let config: MappingConfig = serde_json::from_value(raw_config).map_err(|error| {
            format!(
                "Invalid mapping configuration in {}: {error}",
                path.display()
            )
        })?;
        let (config, migrated) = migrate_mapping_config(config)?;
        validate_mapping_config(&config)?;
        if migrated || legacy_motion_sensitivity {
            write_mapping_config(&config)?;
        }
        return Ok(config);
    }
    let legacy_path = mapping_settings_path()?;
    let config = if legacy_path.exists() {
        let content = fs::read_to_string(&legacy_path)
            .map_err(|error| format!("Could not read legacy mapping settings: {error}"))?;
        migrated_mapping_config(serde_json::from_str(&content).map_err(|error| {
            format!(
                "Invalid legacy mapping settings in {}: {error}",
                legacy_path.display()
            )
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

fn decode_joycon_imu_samples(bytes: &[u8]) -> [Option<ImuSample>; 3] {
    let mut samples = [None, None, None];
    if bytes.first() != Some(&0x30) {
        return samples;
    }
    let signed = |offset: usize| i16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
    // Each native report contains up to three chronological IMU samples. A
    // partial HID packet still yields every complete sample it contains.
    for (index, offset) in [13_usize, 25, 37].into_iter().enumerate() {
        if bytes.len() < offset + 12 {
            continue;
        }
        samples[index] = Some(ImuSample {
            acceleration: [signed(offset), signed(offset + 2), signed(offset + 4)],
            gyroscope: [signed(offset + 6), signed(offset + 8), signed(offset + 10)],
        });
    }
    samples
}

/// Native Joy-Con 0x30 reports carry packed 12-bit stick axes and three IMU
/// samples. macOS's generic HID driver can also expose a compact 0x3f report.
fn decode_joycon_report(
    bytes: &[u8],
    product_id: u16,
) -> (
    Option<Stick>,
    Option<Stick>,
    Option<[u8; 3]>,
    Option<ImuSample>,
) {
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
            (
                Some(axes_stick),
                Some(hat_stick),
                Some([bytes[1], bytes[2], bytes[3]]),
                None,
            )
        } else {
            (
                Some(hat_stick),
                Some(axes_stick),
                Some([bytes[1], bytes[2], bytes[3]]),
                None,
            )
        };
    }
    if bytes.len() < 12 || bytes[0] != 0x30 {
        return (None, None, None, None);
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
    // The report preview keeps the first raw sample for backward-compatible
    // logs. Fusion consumes all complete samples via decode_joycon_imu_samples.
    let imu = decode_joycon_imu_samples(bytes)
        .into_iter()
        .flatten()
        .next();
    (
        Some(left_stick),
        Some(decode_stick(9)),
        Some([bytes[3], bytes[4], bytes[5]]),
        imu,
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
            test_pointer_move,
            pointer_runtime_status,
            request_accessibility_permission,
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
    fn legacy_v3_mapping_config_receives_default_pointer_settings() {
        let config: MappingConfig = serde_json::from_str(
            r#"{"version":3,"activePresetId":"inspect-only","presets":[{"id":"inspect-only","name":"Inspect Only","enabled":false,"bindings":[]}]}"#,
        )
        .expect("v3 config without pointer settings remains readable");
        assert!(!config.pointer.enabled);
        assert_eq!(config.pointer.mode, PointerMode::Stick);
        assert_eq!(config.pointer.mode_switch_hold_ms, 600);
        assert_eq!(config.pointer.motion.sweep_degrees, 60.0);
    }

    #[test]
    fn legacy_motion_sensitivity_is_ignored_in_favor_of_default_sweep() {
        let mut value = serde_json::to_value(MappingConfig::default()).unwrap();
        let motion = value
            .pointer_mut("/pointer/motion")
            .and_then(serde_json::Value::as_object_mut)
            .unwrap();
        motion.remove("sweepDegrees");
        motion.insert("sensitivity".to_owned(), serde_json::json!(8.0));
        let config: MappingConfig = serde_json::from_value(value).unwrap();
        assert_eq!(config.pointer.motion.sweep_degrees, 60.0);
        assert!(validate_mapping_config(&config).is_ok());
    }

    #[test]
    fn motion_sweep_presets_step_in_both_directions() {
        assert_eq!(adjusted_motion_sweep(60.0, true), 45.0);
        assert_eq!(adjusted_motion_sweep(60.0, false), 90.0);
        assert_eq!(adjusted_motion_sweep(30.0, true), 30.0);
        assert_eq!(adjusted_motion_sweep(120.0, false), 120.0);
        assert_eq!(adjusted_motion_sweep(55.0, true), 45.0);
        assert_eq!(adjusted_motion_sweep(55.0, false), 60.0);
    }

    #[test]
    fn adaptive_motion_gain_has_smooth_expected_anchors() {
        assert!((motion_adaptive_gain(0.0) - 0.45).abs() < f32::EPSILON);
        assert!((motion_adaptive_gain(10.0) - 0.45).abs() < f32::EPSILON);
        assert!((motion_adaptive_gain(60.0) - 1.0).abs() < f32::EPSILON);
        assert!((motion_adaptive_gain(180.0) - 1.8).abs() < f32::EPSILON);
        assert!((0.45..1.0).contains(&motion_adaptive_gain(35.0)));
        assert!((1.0..1.8).contains(&motion_adaptive_gain(120.0)));
    }

    #[test]
    fn right_motion_projection_only_inverts_horizontal_axis() {
        let left = motion_projection(4.0, 7.0, JOYCON_LEFT_PRODUCT_ID);
        let right = motion_projection(4.0, 7.0, JOYCON_RIGHT_PRODUCT_ID);
        assert_eq!(left, (7.0, -4.0));
        assert_eq!(right, (-7.0, -4.0));
    }

    #[test]
    fn motion_origin_rezeroes_absolute_pose_without_a_jump() {
        let half_angle = 15.0_f32.to_radians();
        let pose = [0.0, 0.0, half_angle.sin(), half_angle.cos()];
        let recentered = motion_angles_from_origin(pose, pose, JOYCON_RIGHT_PRODUCT_ID);
        assert!(recentered.0.abs() < 0.001);
        assert!(recentered.1.abs() < 0.001);

        let identity = [0.0, 0.0, 0.0, 1.0];
        let before_recenter = motion_angles_from_origin(pose, identity, JOYCON_RIGHT_PRODUCT_ID);
        assert!((before_recenter.0 + 30.0).abs() < 0.001);
    }

    #[test]
    fn stick_pointer_deadzone_and_acceleration_are_radial() {
        assert_eq!(
            pointer_stick_velocity(0.05, 0.05, 0.12, 1400.0, 1.6),
            (0.0, 0.0)
        );
        let (x, y) = pointer_stick_velocity(1.0, 0.0, 0.12, 1400.0, 1.6);
        assert!((x - 1400.0).abs() < 0.001);
        assert!(y.abs() < f32::EPSILON);
    }

    #[test]
    fn quaternion_delta_reports_rotation_around_z() {
        let half_angle = 5.0_f32.to_radians();
        let current = [0.0, 0.0, half_angle.sin(), half_angle.cos()];
        let delta = quaternion_delta_degrees(current, [0.0, 0.0, 0.0, 1.0]);
        assert!(delta[0].abs() < 0.001);
        assert!(delta[1].abs() < 0.001);
        assert!((delta[2] - 10.0).abs() < 0.001);
    }

    #[test]
    fn compact_right_report_exposes_plus_for_mode_switching() {
        let mut bytes = vec![0_u8; 12];
        bytes[0] = 0x3f;
        bytes[2] = 0x01;
        let report = report_from_bytes(bytes, JOYCON_RIGHT_PRODUCT_ID);
        assert!(pressed_button_controls(&report, JOYCON_RIGHT_PRODUCT_ID)
            .contains(&"joycon_right.plus"));
    }

    #[test]
    fn v2_mapping_config_prunes_unverified_builtins() {
        let mut config = MappingConfig {
            version: 2,
            ..MappingConfig::default()
        };
        config.presets.push(MappingPreset {
            id: "keyboard-focus".to_owned(),
            name: "Keyboard Focus".to_owned(),
            enabled: false,
            bindings: vec![],
        });
        config.presets.push(MappingPreset {
            id: "code".to_owned(),
            name: "Code".to_owned(),
            enabled: true,
            bindings: vec![],
        });
        config.active_preset_id = "code".to_owned();
        let (config, migrated) = migrate_mapping_config(config).expect("v2 config migrates");
        assert!(migrated);
        assert_eq!(config.version, 3);
        assert_eq!(config.active_preset_id, "codex-cowork");
        assert!(!config
            .presets
            .iter()
            .any(|preset| preset.id == "keyboard-focus"));
        assert!(!config.presets.iter().any(|preset| preset.id == "code"));
        assert!(validate_mapping_config(&config).is_ok());
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
        let report = joycon_subcommand(0x1f, 0x48, 0x01);
        assert_eq!(report[0], 0x01);
        assert_eq!(report[1], 0x0f);
        assert_eq!(
            &report[2..10],
            &[0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40]
        );
        assert_eq!(&report[10..], &[0x48, 0x01]);
    }

    #[test]
    fn motion_initialization_subcommands_enable_imu_and_native_reports() {
        let enable_imu = joycon_subcommand(0x02, 0x40, 0x01);
        let native_reports = joycon_subcommand(0x03, 0x03, 0x30);
        assert_eq!(&enable_imu[10..], &[0x40, 0x01]);
        assert_eq!(&native_reports[10..], &[0x03, 0x30]);
    }

    #[test]
    fn native_report_decodes_first_raw_imu_sample() {
        let mut report = vec![0_u8; 25];
        report[0] = 0x30;
        report[13..15].copy_from_slice(&100_i16.to_le_bytes());
        report[15..17].copy_from_slice(&(-200_i16).to_le_bytes());
        report[17..19].copy_from_slice(&300_i16.to_le_bytes());
        report[19..21].copy_from_slice(&(-400_i16).to_le_bytes());
        report[21..23].copy_from_slice(&500_i16.to_le_bytes());
        report[23..25].copy_from_slice(&(-600_i16).to_le_bytes());
        let (_, _, _, imu) = decode_joycon_report(&report, JOYCON_LEFT_PRODUCT_ID);
        let imu = imu.expect("native packet has IMU data");
        assert_eq!(imu.acceleration, [100, -200, 300]);
        assert_eq!(imu.gyroscope, [-400, 500, -600]);
    }

    #[test]
    fn native_report_decodes_all_three_imu_samples_in_order() {
        let mut report = vec![0_u8; 49];
        report[0] = 0x30;
        for (index, offset) in [13_usize, 25, 37].into_iter().enumerate() {
            let base = (index as i16 + 1) * 100;
            for (axis, value) in [base, base + 1, base + 2, base + 3, base + 4, base + 5]
                .into_iter()
                .enumerate()
            {
                let start = offset + axis * 2;
                report[start..start + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        let samples = decode_joycon_imu_samples(&report);
        assert_eq!(samples.iter().flatten().count(), 3);
        assert_eq!(samples[0].unwrap().acceleration, [100, 101, 102]);
        assert_eq!(samples[1].unwrap().gyroscope, [203, 204, 205]);
        assert_eq!(samples[2].unwrap().acceleration, [300, 301, 302]);
    }

    #[test]
    fn right_joycon_imu_is_remapped_into_shared_body_axes() {
        let sample = ImuSample {
            acceleration: [4096, 2048, -1024],
            gyroscope: [164, 328, -82],
        };
        let (left_gyro, left_accel) = calibrated_imu_vectors(&sample, JOYCON_LEFT_PRODUCT_ID);
        let (right_gyro, right_accel) = calibrated_imu_vectors(&sample, JOYCON_RIGHT_PRODUCT_ID);
        assert_eq!(left_gyro, Vector3::new(10.0, 5.0, 20.0));
        assert_eq!(left_accel, Vector3::new(1.0, 0.25, 0.5));
        assert_eq!(right_gyro, Vector3::new(10.0, -5.0, -20.0));
        assert_eq!(right_accel, Vector3::new(1.0, -0.25, -0.5));
    }
}
