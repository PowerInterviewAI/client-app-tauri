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
