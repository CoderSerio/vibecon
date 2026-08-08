#[derive(Clone, Copy)]
pub enum PointerButton {
    Left,
    Right,
}

#[cfg(target_os = "macos")]
mod platform {
    use super::PointerButton;
    use core_foundation::{
        base::TCFType,
        boolean::CFBoolean,
        dictionary::{CFDictionary, CFDictionaryRef},
        string::CFString,
    };
    use core_graphics::{
        display::CGDisplay,
        event::{CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton},
        event_source::{CGEventSource, CGEventSourceStateID},
        geometry::CGPoint,
    };
    use std::process::Command;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    }

    pub fn backend_name() -> &'static str {
        "macOS Core Graphics (CGEvent)"
    }

    pub fn accessibility_granted() -> bool {
        // SAFETY: AXIsProcessTrusted has no arguments and only reports TCC state.
        unsafe { AXIsProcessTrusted() }
    }

    pub fn request_accessibility_permission() -> Result<bool, String> {
        let options: CFDictionary<CFString, CFBoolean> = CFDictionary::from_CFType_pairs(&[(
            CFString::new("AXTrustedCheckOptionPrompt"),
            CFBoolean::true_value(),
        )]);
        // SAFETY: the dictionary remains alive for the duration of the call and
        // contains the documented prompt option expected by ApplicationServices.
        let trusted = unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) };
        if !trusted {
            open_accessibility_settings()?;
        }
        Ok(trusted)
    }

    pub fn open_accessibility_settings() -> Result<(), String> {
        Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map_err(|error| format!("Could not open macOS Accessibility settings: {error}"))?;
        Ok(())
    }

    pub fn permission_target() -> String {
        let Ok(executable) = std::env::current_exe() else {
            return "could not determine the running VibeCon path".to_owned();
        };
        executable
            .ancestors()
            .find(|path| path.extension().is_some_and(|extension| extension == "app"))
            .unwrap_or(executable.as_path())
            .display()
            .to_string()
    }

    pub fn cursor_location() -> Result<(f64, f64), String> {
        let source = event_source()?;
        let point = current_location(&source)?;
        Ok((point.x, point.y))
    }

    pub fn display_size_at_cursor() -> Result<(f32, f32), String> {
        let source = event_source()?;
        let point = current_location(&source)?;
        let display = CGDisplay::displays_with_point(point, 1)
            .ok()
            .and_then(|(displays, count)| (count > 0).then(|| displays[0]))
            .map(CGDisplay::new)
            .unwrap_or_else(CGDisplay::main);
        let size = display.bounds().size;
        if size.width <= 0.0 || size.height <= 0.0 {
            return Err("Could not determine the active macOS display size".to_owned());
        }
        Ok((size.width as f32, size.height as f32))
    }

    fn event_source() -> Result<CGEventSource, String> {
        CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|_| "Could not create a macOS pointer event source".to_owned())
    }

    fn current_location(source: &CGEventSource) -> Result<CGPoint, String> {
        CGEvent::new(source.clone())
            .map(|event| event.location())
            .map_err(|_| "Could not read the macOS pointer position".to_owned())
    }

    pub fn post_move(dx: f32, dy: f32, dragging: bool) -> Result<(), String> {
        if !accessibility_granted() {
            return Err("Accessibility is not granted to VibeCon".to_owned());
        }
        let source = event_source()?;
        let current = current_location(&source)?;
        let destination = CGPoint::new(current.x + f64::from(dx), current.y + f64::from(dy));
        let event_type = if dragging {
            CGEventType::LeftMouseDragged
        } else {
            CGEventType::MouseMoved
        };
        let event = CGEvent::new_mouse_event(source, event_type, destination, CGMouseButton::Left)
            .map_err(|_| "Could not create a macOS pointer move event")?;
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    pub fn post_button(button: PointerButton, down: bool) -> Result<(), String> {
        if !accessibility_granted() {
            return Err("Accessibility is not granted to VibeCon".to_owned());
        }
        let source = event_source()?;
        let location = current_location(&source)?;
        let (event_type, mouse_button) = match (button, down) {
            (PointerButton::Left, true) => (CGEventType::LeftMouseDown, CGMouseButton::Left),
            (PointerButton::Left, false) => (CGEventType::LeftMouseUp, CGMouseButton::Left),
            (PointerButton::Right, true) => (CGEventType::RightMouseDown, CGMouseButton::Right),
            (PointerButton::Right, false) => (CGEventType::RightMouseUp, CGMouseButton::Right),
        };
        let event = CGEvent::new_mouse_event(source, event_type, location, mouse_button)
            .map_err(|_| "Could not create a macOS pointer button event")?;
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    pub fn post_scroll(delta: i32) -> Result<(), String> {
        if !accessibility_granted() {
            return Err("Accessibility is not granted to VibeCon".to_owned());
        }
        let source = event_source()?;
        // Pixel units avoid coarse one-line jumps and let the stick velocity
        // accumulate into smooth trackpad-like scrolling.
        let event = CGEvent::new_scroll_event(source, 0, 1, delta, 0, 0)
            .map_err(|_| "Could not create a macOS scroll event")?;
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    pub fn post_control_modifier(down: bool) -> Result<(), String> {
        if !accessibility_granted() {
            return Err("Accessibility is not granted to VibeCon".to_owned());
        }
        let source = event_source()?;
        // kVK_Control (left Control) from Carbon HIToolbox/Events.h.
        let event = CGEvent::new_keyboard_event(source, 59, down)
            .map_err(|_| "Could not create a macOS Control event")?;
        event.set_flags(if down {
            CGEventFlags::CGEventFlagControl
        } else {
            CGEventFlags::empty()
        });
        event.post(CGEventTapLocation::HID);
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::PointerButton;

    pub fn backend_name() -> &'static str {
        "Unavailable on this platform"
    }

    pub fn accessibility_granted() -> bool {
        false
    }

    pub fn request_accessibility_permission() -> Result<bool, String> {
        Err("Accessibility permission is only required on macOS".to_owned())
    }

    pub fn open_accessibility_settings() -> Result<(), String> {
        Err("Accessibility settings are not implemented for this platform yet".to_owned())
    }

    pub fn permission_target() -> String {
        "Pointer control is not implemented for this platform yet".to_owned()
    }

    pub fn cursor_location() -> Result<(f64, f64), String> {
        Err("Pointer control is not implemented for this platform yet".to_owned())
    }

    pub fn display_size_at_cursor() -> Result<(f32, f32), String> {
        Err("Pointer control is not implemented for this platform yet".to_owned())
    }

    pub fn post_move(_dx: f32, _dy: f32, _dragging: bool) -> Result<(), String> {
        Err("Pointer control is not implemented for this platform yet".to_owned())
    }

    pub fn post_button(_button: PointerButton, _down: bool) -> Result<(), String> {
        Err("Pointer control is not implemented for this platform yet".to_owned())
    }

    pub fn post_scroll(_delta: i32) -> Result<(), String> {
        Err("Pointer control is not implemented for this platform yet".to_owned())
    }

    pub fn post_control_modifier(_down: bool) -> Result<(), String> {
        Err("Pointer control is not implemented for this platform yet".to_owned())
    }
}

pub use platform::*;
