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
    // Safety net: ensure native loopback capture stops even if the renderer's
    // disable_loopback_audio call did not run.
    services.loopback.stop();
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

/// Start native system-audio (loopback) capture and streaming for the `ch_0`
/// (interviewer) channel. Capture runs in the Rust backend: WASAPI loopback on Windows,
/// ScreenCaptureKit on macOS. Returns an error if the device is unavailable or the OS
/// permission (macOS screen recording) is denied.
#[tauri::command]
pub fn enable_loopback_audio(services: State<'_, AppServices>) -> Result<(), String> {
    services.loopback.start()
}

/// Stop native loopback capture and streaming.
#[tauri::command]
pub fn disable_loopback_audio(services: State<'_, AppServices>) -> Result<(), String> {
    services.loopback.stop();
    Ok(())
}
