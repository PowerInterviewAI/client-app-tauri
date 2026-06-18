// Microphone and screen-recording permission checks/requests are handled by
// tauri-plugin-macos-permissions (accurate native APIs that also register the app in the
// System Settings Privacy lists). The commands below only render the guidance dialogs.

#[tauri::command]
pub async fn permissions_show_denied_dialog(
    permission_type: String, // sent from JS as camelCase `permissionType` (Tauri v2)
    app: tauri::AppHandle,
) -> Result<(), String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

    // macOS does not re-show the native permission prompt after the first decision, so we
    // can only point the user at the right System Settings pane to re-grant it. These
    // `x-apple.systempreferences:` deep links open directly to the relevant Privacy section.
    let (label, settings_url) = match permission_type.as_str() {
        "screen-recording" => (
            "Screen Recording",
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
        ),
        // System audio is captured via CoreAudio process taps, gated by the Audio Capture
        // permission (Privacy_AudioCapture), not Screen Recording.
        // Note: Audio Capture does NOT require an app restart after granting - unlike Screen
        // Recording, the TCC grant takes effect immediately so the user can start again directly.
        "system-audio" => (
            "Audio Capture",
            "x-apple.systempreferences:com.apple.preference.security?Privacy_AudioCapture",
        ),
        _ => (
            "Microphone",
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
        ),
    };

    // Build the dialog message. For system-audio (Audio Capture), the TCC prompt may have
    // appeared separately at the same time as this dialog, so the message covers both
    // first-run (grant the system prompt, then start again) and previously-denied (go to
    // System Settings) cases. Crucially, Audio Capture does NOT require an app restart after
    // granting - unlike Screen Recording - so we tell the user to start again, not restart.
    let message = if permission_type == "system-audio" {
        format!(
            "Power Interview AI could not access system audio.\n\n\
            If a macOS permission prompt appeared, click Allow there and then start the assistant again.\n\n\
            If no prompt appeared or permission was previously denied, open System Settings > \
            Privacy & Security > {label}, enable it, and start the assistant again.\n\n\
            (No app restart is needed after granting {label} permission.)"
        )
    } else {
        format!(
            "{label} permission was denied.\n\
            Enable it in System Settings > Privacy & Security > {label}, then restart Power Interview AI."
        )
    };

    // `blocking_show` returns true when the first (OK) button is pressed.
    let open_settings = app
        .dialog()
        .message(message)
        .kind(MessageDialogKind::Error)
        .title("Permission Required")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Open System Settings".to_string(),
            "Close".to_string(),
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
