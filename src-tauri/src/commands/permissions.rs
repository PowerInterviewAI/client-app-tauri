use serde_json::{json, Value};

#[tauri::command]
pub async fn permissions_check_screen_recording() -> Value {
    #[cfg(target_os = "macos")]
    {
        let status = check_screen_recording_macos();
        return json!({ "status": status });
    }
    #[cfg(not(target_os = "macos"))]
    {
        json!({ "status": "granted" })
    }
}

#[tauri::command]
pub async fn permissions_check_screen_sources() -> Result<Value, String> {
    // Attempt a screenshot as a proxy for screen recording permission
    match xcap::Monitor::all() {
        Ok(monitors) if !monitors.is_empty() => Ok(json!({ "granted": true })),
        _ => Ok(json!({ "granted": false })),
    }
}

#[tauri::command]
pub async fn permissions_check_microphone() -> Value {
    #[cfg(target_os = "macos")]
    {
        let status = check_microphone_macos();
        return json!({ "status": status });
    }
    #[cfg(not(target_os = "macos"))]
    {
        json!({ "status": "granted" })
    }
}

#[tauri::command]
pub async fn permissions_request_microphone() -> Value {
    // On macOS this requires system API; prompt via shell open as fallback
    json!({ "granted": false, "note": "Open System Settings to grant microphone access" })
}

#[tauri::command]
pub async fn permissions_show_denied_dialog(
    permission_type: String, // matches snake_case from bridge
    app: tauri::AppHandle,
) -> Result<(), String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

    // macOS does not re-show the native permission prompt after the first decision, so we
    // can only point the user at the right System Settings pane to re-grant it. These
    // `x-apple.systempreferences:` deep links open directly to the relevant Privacy section.
    let (label, settings_url) = if permission_type == "screen-recording" {
        (
            "Screen Recording",
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
        )
    } else {
        (
            "Microphone",
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
        )
    };

    // `blocking_show` returns true when the first (OK) button is pressed.
    let open_settings = app
        .dialog()
        .message(format!(
            "{label} permission was denied.\nEnable it in System Settings > Privacy & Security > {label}, then restart Power Interview AI."
        ))
        .kind(MessageDialogKind::Error)
        .title("Permission Required")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Open System Settings".to_string(),
            "Cancel".to_string(),
        ))
        .blocking_show();

    if open_settings {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_url(settings_url, None::<&str>)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn permissions_show_restart_dialog(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
    app.dialog()
        .message(
            "Permission granted. Please restart Power Interview AI for the changes to take effect.",
        )
        .kind(MessageDialogKind::Info)
        .title("Restart Required")
        .blocking_show();
    Ok(())
}

#[cfg(target_os = "macos")]
fn check_screen_recording_macos() -> &'static str {
    // Attempt to capture to check permission
    match xcap::Monitor::all().and_then(|m| {
        m.into_iter()
            .next()
            .ok_or(xcap::XCapError::new("no monitor"))
    }) {
        Ok(mon) => match mon.capture_image() {
            Ok(_) => "granted",
            Err(_) => "denied",
        },
        Err(_) => "denied",
    }
}

#[cfg(target_os = "macos")]
fn check_microphone_macos() -> &'static str {
    // Without AVFoundation bindings, report unknown
    "not-determined"
}
