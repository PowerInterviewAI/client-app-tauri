use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::consts::API_LLM_SUMMARIZE;
use crate::services::api_client::ApiClient;
use crate::types::app_state::{LiveSuggestion, Transcript};
use crate::AppServices;

#[tauri::command]
pub fn tools_clear_all(services: State<'_, AppServices>) {
    services.tools.clear_all();
}

/// Open the exported report with the OS default application ("Open file" in the
/// export success toast). Invoked from Rust via `OpenerExt`, which bypasses the
/// opener plugin's capability path scope; that is intentional here, the path is
/// one the user just chose in the native save dialog.
#[tauri::command]
pub fn tools_open_path(app: AppHandle, path: String) -> Result<(), String> {
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Reveal the exported report in its containing folder, highlighting the file in
/// the OS file browser ("Open folder" in the export success toast).
#[tauri::command]
pub fn tools_reveal_path(app: AppHandle, path: String) -> Result<(), String> {
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tools_set_placeholder_data(services: State<'_, AppServices>) {
    services.tools.set_placeholder_data();
}

/// Everything the frontend needs to build a "smart" export document: the
/// LLM-generated summary plus the raw transcripts and live suggestions. The
/// summary is produced by the backend `/api/llm/summarize` endpoint here so the
/// frontend only has to assemble the Markdown and render the DOCX.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportData {
    pub username: String,
    pub summary: String,
    pub transcripts: Vec<Transcript>,
    pub live_suggestions: Vec<LiveSuggestion>,
}

/// Collect the export payload: requests the summary from the backend, then
/// returns it alongside the current transcripts and suggestions. Errors (no
/// transcript yet, expired session, LLM failure) propagate to the frontend.
#[tauri::command]
pub async fn tools_get_export_data(services: State<'_, AppServices>) -> Result<ExportData, String> {
    let state = services.app_state.get_state();
    if state.transcripts.is_empty() {
        return Err("No transcript to export yet".to_string());
    }

    let conf = services.config_store.get_config();
    let username = conf.interview_conf.username.clone();
    let token = conf.session_token.clone();

    let client = if token.is_empty() {
        ApiClient::new()
    } else {
        ApiClient::new().with_token(&token)
    };

    let body = serde_json::json!({
        "config": conf.llm_conf,
        "username": username,
        "transcripts": state.transcripts,
    });

    let summary = match client.post(API_LLM_SUMMARIZE, &body).await {
        Ok(value) => value.as_str().unwrap_or_default().to_string(),
        Err(e) => return Err(crate::utils::llm_error_message(&e)),
    };

    Ok(ExportData {
        username,
        summary,
        transcripts: state.transcripts,
        live_suggestions: state.live_suggestions,
    })
}
