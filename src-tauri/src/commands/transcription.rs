use tauri::State;

use crate::AppServices;

#[tauri::command]
pub fn transcription_start(services: State<'_, AppServices>) {
    use crate::types::app_state::RunningState;
    services.transcript.start();
    services.app_state.set_running_state(RunningState::Running);
}

#[tauri::command]
pub fn transcription_stop(services: State<'_, AppServices>) {
    use crate::types::app_state::RunningState;
    services.transcript.stop();
    services.app_state.set_running_state(RunningState::Idle);
}

#[tauri::command]
pub fn transcription_clear(services: State<'_, AppServices>) {
    services.transcript.clear();
}

#[tauri::command]
pub async fn transcription_ingest(
    channel: String,
    transcript_type: String,
    text: String,
    services: State<'_, AppServices>,
) -> Result<(), String> {
    services
        .transcript
        .ingest(&channel, &transcript_type, &text)
        .await;
    Ok(())
}

#[tauri::command]
pub fn transcription_set_session_token(token: String, services: State<'_, AppServices>) {
    services
        .config_store
        .update_config(serde_json::json!({ "sessionToken": token }));
}

/// Pre-flight for loopback (system audio) capture.
///
/// The actual capture happens in the renderer via `getDisplayMedia({ audio: true })`.
/// On macOS we validate the screen-recording permission here so the renderer call
/// doesn't fail opaquely; on Windows there is nothing to prepare.
#[tauri::command]
pub fn enable_loopback_audio() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let monitors = xcap::Monitor::all()
            .map_err(|e| format!("Screen recording permission check failed: {e}"))?;
        let monitor = monitors
            .into_iter()
            .next()
            .ok_or("No display monitor found for screen recording check")?;
        monitor
            .capture_image()
            .map_err(|e| format!("Screen recording permission denied: {e}"))?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        // System audio is captured in the renderer through getDisplayMedia; no native
        // setup is required on Windows.
        Ok(())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Err("Loopback audio support is only available on Windows 11+ and macOS 14.4+.".into())
    }
}

/// Retained for IPC compatibility with the renderer's start/stop flow. Loopback
/// capture lives entirely in the renderer, so there is no native stream to tear down.
#[tauri::command]
pub fn disable_loopback_audio() -> Result<(), String> {
    Ok(())
}
