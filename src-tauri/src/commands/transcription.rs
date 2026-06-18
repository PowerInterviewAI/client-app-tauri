use tauri::webview::cookie::{Cookie, SameSite};
use tauri::{AppHandle, Manager, State, Url};

use crate::consts::BACKEND_BASE_URL;
use crate::AppServices;

#[tauri::command]
pub fn transcription_start(services: State<'_, AppServices>) {
    services.transcript.start();
}

#[tauri::command]
pub fn transcription_stop(services: State<'_, AppServices>) {
    services.transcript.stop();
    // Safety net: ensure native loopback capture stops even if the renderer's
    // disable_loopback_audio call did not run.
    services.loopback.stop();
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

/// Stores the session token and mirrors it into the webview's cookie jar so the
/// renderer's mic-capture WebSocket (browser-native, no custom headers) authenticates
/// the same way the backend's `get_session_ws` dependency expects: a `session_token`
/// cookie, not a query parameter.
#[tauri::command]
pub fn transcription_set_session_token(
    token: String,
    app: AppHandle,
    services: State<'_, AppServices>,
) {
    services
        .config_store
        .update_config(serde_json::json!({ "sessionToken": token }));

    if token.is_empty() {
        return;
    }
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(backend_url) = Url::parse(BACKEND_BASE_URL) else {
        return;
    };
    let Some(host) = backend_url.host_str() else {
        return;
    };
    let secure = backend_url.scheme() == "https";
    let cookie = Cookie::build(("session_token", token))
        .domain(host.to_string())
        .path("/")
        .secure(secure)
        .http_only(false)
        .same_site(if secure {
            SameSite::None
        } else {
            SameSite::Lax
        })
        .build();
    let _ = window.set_cookie(cookie);
}

/// Start native system-audio (loopback) capture and streaming for the `ch_0`
/// (interviewer) channel. Capture runs in the Rust backend: WASAPI loopback on Windows,
/// CoreAudio process taps on macOS. Returns an error if the device is unavailable or the OS
/// permission (macOS system-audio capture) is denied.
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
