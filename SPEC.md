# SPEC.md

Technical specification for the Power Interview Tauri desktop client. See [CLAUDE.md](CLAUDE.md) for the day-to-day command and convention summary.

## Overview

Power Interview AI is a Tauri desktop client (Windows and macOS, React frontend, Rust native
backend) that assists a candidate during a live interview. While running it:

- transcribes both sides of the conversation in real time (candidate microphone and
  interviewer/system audio),
- automatically generates suggested answers when the interviewer finishes asking a question,
- lets the candidate manually capture screenshots (e.g. of a coding challenge or slide) and
  request an AI suggestion based on those images plus the transcript,
- offers a "stealth" window mode (click-through, always-on-top, reduced opacity, hidden from
  screen capture) so the assistant overlay stays out of the way during screen-shared interviews,
- handles authentication, credits/payment, and app updates against the
  `api.powerinterviewai.com` backend.

## Architecture

### Frontend

- `src/` - React 19, Tailwind 4, Zustand stores, hooks, components, pages.
- `src/lib/tauri-bridge.ts` exposes the IPC bridge API (`window.tauriApi`) used by
  renderer hooks; see [IPC Bridge & State Sync](#ipc-bridge--state-sync).
- `src/router.tsx` defines hash-based routes: `/` (index/splash), `/auth/login`,
  `/auth/signup`, `/main` (the assistant UI), `/payment`.
- `src/services/live-transcription.service.ts` owns the renderer-side microphone capture and
  ASR websocket.

### Native Backend

`src-tauri/src/` contains Tauri command handlers, services, persisted state, and native
utilities. All long-lived services are constructed once in `lib.rs::run()` and stored as
`AppServices` (Tauri managed state), then injected into command handlers via `State<AppServices>`:

| Service | Responsibility |
| --- | --- |
| `ConfigStore` | Persists `RuntimeConfig` (profile, credentials, device/LLM prefs) and window settings to `config.json` in the app data dir. |
| `AppStateService` | Holds the single `AppState` struct and emits `app-state-updated` on every change. |
| `TranscriptService` | Aggregates/merges mic (`ch_1`) and loopback (`ch_0`) transcripts, triggers live suggestions. |
| `LiveSuggestionService` | Streams automatic answer suggestions from the backend LLM. |
| `ActionSuggestionService` | Handles screenshot capture/upload and on-demand suggestion generation. |
| `ActionLockService` | Prevents overlapping screenshot/suggestion actions (two atomic locks). |
| `LoopbackService` | Native system-audio capture and streaming (Windows/macOS). |
| `ToolsService` | Transcript export support, "clear all", placeholder/demo data. |
| `WindowControlService` | Stealth mode, opacity cycling, window positioning/resizing. |
| `ZoomService` | Webview zoom level, persisted and clamped. |
| `PushNotificationService` | Emits `push-notification` events shown as toasts. |
| `HealthCheckService` | Polls backend liveness and (if logged in) credits/role/plan. |

- `src-tauri/tauri.conf.json` - window, bundle, and updater settings for macOS/Windows.
- `src-tauri/Cargo.toml` - Rust dependency manifest (notable: `reqwest` for REST + streaming,
  `tokio-tungstenite` for the loopback ASR websocket, `cpal`/`screencapturekit` for native audio
  capture, `xcap`/`image` for screenshots, Tauri plugins for global shortcuts, dialogs, shell,
  fs, and the updater).

### IPC Bridge & State Sync

- Tauri `invoke()` is exposed through `tauriApi` (`src/lib/tauri-bridge.ts`) and assigned to
  `window.tauriApi` in `main.tsx`, which renderer hooks access via `getTauriApi()`
  (`src/lib/utils.ts`).
- **AppState push model**: `AppStateService` holds one `AppState` struct (transcripts, live and
  action suggestions, running state, login/credits/role, stealth flag, etc.). Every mutation
  emits an `app-state-updated` Tauri event with the full state. The renderer's
  `AppStateManager` (`src/hooks/use-app-state.tsx`) is a singleton that subscribes to this event
  (falling back to 1s polling of `app_state_get` if events are unavailable) and fans state out
  to all `useAppState()` consumers. `updateAppState(partial)` calls `app_state_update`, which
  deep-merges the partial JSON into the Rust-side state via `merge_json` and re-emits.
- **Config**: `useConfigStore` (Zustand) loads `RuntimeConfig` via `config_get` and persists
  partial updates via `config_update`, which deep-merges into `StoredConfig` and writes
  `config.json` immediately.
- Other domains (auth, payment, LLM, transcription, suggestions, tools, window/zoom, permissions,
  updater) are exposed as one Tauri command per operation; see `tauri-bridge.ts` for the full
  list.

## Configuration & Persistence

`ConfigStore` (`src-tauri/src/store/config_store.rs`) persists a `StoredConfig` to
`<app-data-dir>/config.json`:

- `runtime: RuntimeConfig` - interview profile (`interviewConf`: photo, username, profile data,
  job description), `language`, `sessionToken`, `rememberMe`/`email`/`password`, audio input
  device name, video/face-swap settings (currently disabled, see migration below), `llmConf`
  (provider/model/API key), and per-panel auto-scroll preferences.
- `window: WindowConfig` - saved window bounds, stealth flag (always reset to `false` on
  launch), and zoom factor.

On load, `migrate()` forces `runtime.face_swap = false` (the face-swap/video-avatar feature is
disabled) and re-persists. `update_config` / `app_state_update` both use a generic `merge_json`
deep-merge so partial updates from the renderer only touch the fields they include.

## Authentication, Session & Health Check

- `AuthService` (`src-tauri/src/services/auth.rs`) wraps `/api/auth/{signup,login,logout,
  change-password}`. On signup/login success, the returned `session_token` (or `access_token`)
  is written into `RuntimeConfig.sessionToken`. Logout clears the token.
- `HealthCheckService` starts once at app launch (`lib.rs` setup):
  - Sets `isLoggedIn = null` (unknown) initially.
  - If a session token is stored, POSTs `/api/health-check/ping-client`; on success sets
    `isLoggedIn = true` and populates `credits`, `userRole`, `betaTesterExpiresAt`, and
    `providedLlmModel` from the response. On failure sets `isLoggedIn = false`.
  - Then loops forever: GETs `/api/health-check/ping` to set `isBackendLive`, and (while logged
    in) re-POSTs `ping-client` to refresh credits/role. Polls every 5s while the backend is
    live, or every 1s while it is down (`SUCCESS_INTERVAL_MS` / `FAILURE_INTERVAL_MS`).
- The renderer's `MainPage` redirects to `/auth/login` once `appState.isLoggedIn === false`, and
  shows loading states while `isLoggedIn` is `null` or the backend isn't live yet.

## Audio Capture and Transcription

Capture is split by source: the microphone runs in the renderer, system audio runs natively.
Both feed the same `TranscriptService`, which only ingests audio while the assistant is
"running" (`transcription_start` sets `is_active = true`; `transcription_stop` clears it and
also stops native loopback capture as a safety net).

**Microphone (`ch_1`, self):** captured in the renderer with `getUserMedia` (device chosen via
`audioInputDeviceName`), resampled to 16 kHz mono PCM16 in an `AudioWorklet`, and streamed over
a WebSocket to the ASR endpoint (`/api/asr/streaming?token=<sessionToken>`). The websocket
client (`AudioWsStream` in `src/services/live-transcription.service.ts`) retries connection with
exponential backoff (up to 5 attempts, capped at 8s), reconnects on unexpected close while
active, and drops outgoing audio if the socket's buffered amount exceeds ~0.3s of PCM to bound
latency/memory. Returned `partial`/`final` transcript messages are forwarded to Rust via
`transcription_ingest`.

**System/interviewer audio (`ch_0`, other):** captured natively in Rust, because
`getDisplayMedia` system-audio capture is unreliable in the macOS WKWebView. See
`src-tauri/src/services/loopback.rs`:

- Windows: WASAPI loopback via `cpal` (the default render endpoint opened as an input
  device, which transparently captures system output).
- macOS: ScreenCaptureKit audio capture (requires screen-recording permission); a 2x2 video
  config is used so SCK only delivers audio, dropping any frames since no screen handler is
  attached.
- Captured audio is downmixed to mono and resampled to 16 kHz PCM16 with a streaming
  linear-interpolation resampler (state carried across capture buffers to avoid discontinuities),
  then streamed over its own WebSocket to the same ASR endpoint. The streamer reconnects with
  exponential backoff (capped at 8s) if the remote socket closes, and exits cleanly when capture
  stops. Returned transcripts are ingested directly as `ch_0`, with no renderer round-trip.

The renderer starts/stops native capture through the `enable_loopback_audio` /
`disable_loopback_audio` commands, called from `liveTranscriptionService.start()` /`.stop()`. On
macOS, a loopback start failure (most commonly a denied screen-recording permission) surfaces a
native "permission denied" dialog instead of a generic error.

### Transcript aggregation, merging, and live-suggestion triggers

`TranscriptService::ingest` (`src-tauri/src/services/transcript.rs`) tracks separate "self" and
"other" transcript lists plus one in-progress partial transcript per speaker:

- A `final` transcript closes out (and replaces) the matching speaker's partial, using the
  partial's original timestamp if one existed.
- A `partial` transcript updates the in-progress entry for that speaker (creating it if absent).
- After each update, all finalized + partial transcripts are merged, sorted by timestamp, and run
  through `merge_consecutive`: consecutive entries from the **same speaker** whose gap is
  `<= TRANSCRIPT_INTER_TRANSCRIPT_GAP_MS` (5s) are concatenated into a single transcript entry.
  This merged list is what's pushed to `AppState.transcripts` and rendered in the transcript
  panel.
- A live suggestion is triggered when: the new transcript is from `Other` and is `final`, there
  is no pending `self` partial (the candidate isn't mid-sentence), and the candidate's last
  finalized utterance ended more than `LIVE_SUGGESTION_GAP_MS` (2s) ago. This avoids firing a
  suggestion while the candidate is actively answering.

### Platform note: macOS capture is unverified locally

The macOS ScreenCaptureKit path is gated behind `#[cfg(target_os = "macos")]` and is not
compiled by `cargo check` on Windows. It is written against the documented
`screencapturekit` 1.x API and must be validated by the macOS CI build / on real hardware.

## AI Suggestions

Both suggestion services build a request body of `{ config: llmConf, profile_data, context:
jobDescription, transcripts, ... }`, POST it to a streaming endpoint via `ApiClient::post_stream`,
and incrementally decode UTF-8 chunks (`crate::utils::drain_utf8`) into a growing `answer`
string. Each suggestion has a `state`: `pending -> loading -> success | stopped | error` (plus
`uploading`/`idle` for action suggestions while images are attached). Suggestions are stored in
`BTreeMap<i64, _>` keyed by start timestamp so `.values()` yields them in chronological order;
the renderer treats the last element as the active/streaming one.

### Live Suggestions (automatic)

`LiveSuggestionService::start_generate` (`src-tauri/src/services/live_suggestion.rs`):

1. Trims trailing "self" transcripts (the candidate's own latest words aren't part of the
   question being answered).
2. Aborts any in-flight generation (`abort_flags`, checked between stream chunks) and starts a
   fresh one.
3. POSTs the transcripts + LLM config + profile/context to `/api/llm/live-suggestion`.
4. Streams the response into the suggestion's `answer`. If the (partial or full) answer starts
   with the sentinel `NO_SUGGESTION_NEEDED`, the suggestion is removed entirely (no card is
   shown). Otherwise it ends in `success`.
5. Every change re-emits the full suggestion list to `AppState.liveSuggestions`.

`live_suggestion_clear` removes all suggestions; `live_suggestion_stop` aborts the in-flight
request without clearing history.

### Action Suggestions (manual / screenshot-triggered)

`ActionSuggestionService` (`src-tauri/src/services/action_suggestion.rs`) supports a manual,
screenshot-driven workflow, gated by `RunningState::Running` and the two `ActionLockService`
locks (`ScreenshotCapture`, `CaptureSuggestion`) so the candidate can't double-trigger:

- **Capture screenshot** (`action_capture_screenshot` / hotkey F9): grabs the primary monitor via
  `xcap`, converts it to grayscale with `image`, and uploads the PNG to
  `/api/llm/upload-image` (multipart). Up to `ACTION_SUGGESTION_MAX_CAPTURES` (4) images can be
  queued; the queue is shown as a pending "uploading"/"idle" suggestion card with thumbnail URLs
  built from `/api/llm/get-thumb/<name>`.
- **Clear images** (`action_clear_images` / hotkey F10): drops the queued screenshots.
- **Generate suggestion** (`action_start_generate` / hotkey F11): aborts any in-flight
  generation, drains the queued image names, and POSTs `{ config, profile_data, context,
  transcripts, image_names }` to `/api/llm/action-suggestion`, streaming the answer the same way
  as live suggestions (`pending -> loading -> success/stopped/error`, no `NO_SUGGESTION_NEEDED`
  sentinel).
- **Capture + generate** (hotkey F12): captures a screenshot first only if none are already
  queued, then immediately generates a suggestion.

`action_suggestion_clear` removes all suggestions and queued images; `action_suggestion_stop`
aborts the in-flight request.

## Window Control & Stealth Mode

`WindowControlService` (`src-tauri/src/services/window_control.rs`):

- **Stealth mode** (`window_toggle_stealth`, requires `isLoggedIn`): makes the window
  click-through (`set_ignore_cursor_events`) and always-on-top. On macOS it also switches the
  app's activation policy to `Accessory` (hides the Dock icon). The `stealth-changed` event
  toggles a `stealth` class on `<body>` (see `tauri-bridge.ts`), and `AppState.isStealth` /
  `ConfigStore` are updated. While in stealth, a compact `StatusPanel` is shown instead of the
  full `ControlPanel`.
- **Translucency**: dimming is applied to the native window itself, not in CSS. On Windows the
  window is made `WS_EX_LAYERED` and `SetLayeredWindowAttributes` sets its alpha; on macOS the
  `NSWindow`'s `setAlphaValue:` is used (both via `raw-window-handle`). The whole window
  (background, text, borders, images) dims uniformly.
- **Opacity toggle** (`window_toggle_opacity`, only while in stealth): `WindowControlService`
  cycles an index over `OPACITY_LEVELS` (`[0.2, 0.5, 0.75, 0.9]`) and applies the selected alpha to the
  native window (`set_window_opacity`). The level is persisted to `config.window.opacityLevel`,
  restored on launch, and re-applied each time stealth is entered.
- **Hotkeys panel**: stealth is click-through, so hover can't open it. `Ctrl+Shift+H` emits
  `hotkey-toggle-hotkeys`, which `StatusPanel` (`onHotkeyToggleHotkeys` in the bridge) uses to
  toggle a centered modal listing the shortcuts (normal, non-inverted theme).
- **Window positioning**: `window_move_to_position` snaps the window to one of nine
  screen-relative presets (corners/edges/center) on the current monitor;
  `window_move_by_arrow` / `window_resize_by_arrow` nudge position/size by 20px, clamped to
  `MIN_WIDTH`/`MIN_HEIGHT`. The held-arrow hotkeys auto-repeat: the handler acts once on press,
  then a bounded loop keyed to `WindowControlService`'s repeat token continues until release
  (`bump_repeat`/`repeat_token`/`stop_repeat`), since global shortcuts don't emit OS key-repeat.
- **Window buttons** (Windows titlebar, hidden in stealth): `window_minimize` and
  `window_toggle_maximize` (maximize/unmaximize) alongside `window_close`.
- **Window bounds** are saved on `CloseRequested` and restored on next launch (also saved
  explicitly before an auto-update install, since `install()` exits/restarts the process without
  firing the close event).
- **Zoom** (`ZoomService`): webview zoom factor, clamped to `[0.5, 3.0]` in `0.1` steps,
  persisted to `config.window.zoomFactor`, applied on launch, and broadcast via
  `zoom-level-changed` (rounded percent) for the `ZoomControl` UI.
- **Content protection**: unless `DISABLE_CONTENT_PROTECTION` is set in the environment, the main
  window is created with `set_content_protected(true)` so the app's own window is excluded from
  screen captures/recordings.

## Global Hotkeys

Registered once at startup (`lib.rs::register_hotkeys`). Most are `Ctrl+Shift+<key>`; the
move/resize bindings add `Alt` / `Win` respectively and are distinguished by modifier in the
handler:

| Hotkey | Action |
| --- | --- |
| `Q` | Emit `hotkey-stop-assistant` (renderer calls `stopAssistant()`). |
| `M` | Toggle stealth mode. |
| `N` | Toggle opacity (stealth only). |
| `H` | Show/hide the hotkeys panel (stealth is click-through, so this replaces hover). |
| `=` / `-` / `0` | Zoom in / out / reset. |
| `K` / `J` / `L` | Scroll live-suggestions panel up / down / to end (`hotkey-scroll`, section `"0"`). |
| `I` / `U` / `O` | Scroll action-suggestions panel up / down / to end (`hotkey-scroll`, section `"1"`). |
| `F9` | Capture screenshot for action suggestions. |
| `F10` | Clear queued screenshot images. |
| `F11` | Generate an action suggestion from queued images + transcript. |
| `F12` | Capture a screenshot (if none queued) then generate an action suggestion. |
| `1`-`9` | Move the window to one of nine screen positions (bottom/middle/top x left/center/right). |
| `Ctrl+Alt+Shift+Arrow` | Move the window 20px in the arrow direction; hold to repeat (`move_by_arrow`). |
| `Ctrl+Win+Shift+Arrow` | Resize the window 20px in the arrow direction; hold to repeat (`resize_by_arrow`). |

## Assistant Lifecycle

`useAssistantService` (`src/hooks/use-assistant-service.ts`) drives `AppState.runningState`
(`idle -> starting -> running -> stopping -> idle`):

- **Start**: pre-flight checks microphone and screen-recording permissions (showing native
  denial dialogs and aborting if either is denied), and on macOS verifies
  `getDisplayMedia`-capable screen sources are available (prompting a restart if not). Then sets
  `Starting`, clears all previous transcripts/suggestions (`tools.clearAll`), calls
  `transcription_start` (marks the transcript service active and flips `RunningState` to
  `Running` on the Rust side), starts `liveTranscriptionService` (mic capture + native loopback),
  waits 3s, then sets `Running`.
- **Stop**: sets `Stopping`, stops mic capture/loopback/live-transcription, calls
  `transcription_stop`/`live_suggestion_stop`/`action_suggestion_stop` in parallel, force-disables
  stealth mode, waits 3s, then sets `Idle`.
- The `ControlPanel` start button is also gated client-side by `checkCanStart()`, which requires a
  saved profile (username + profile data, plus a photo if face-swap is enabled) and a valid,
  available audio input device (and video device, if face-swap is enabled).

## Payment & Credits

`PaymentService` (`src-tauri/src/services/payment.rs`) wraps `/api/payment/{plans, currencies,
create, status/<id>, history}`, all authenticated with the session token when present.
`payment_get_credits` reuses `/api/health-check/ping-client` to read the current credit balance.
The `/payment` route and `usePayment` hook drive the buy-credits, payment-history, and
payment-status tabs.

## Tools

`ToolsService` (`src-tauri/src/services/tools.rs`) and the `tools` bridge namespace:

- **Export transcript** ("smart" report): `tools_get_export_data` requests an LLM summary from
  the backend (`/api/llm/summarize`) and returns it together with the current transcripts and
  live suggestions (`ExportData`). The renderer assembles a Markdown report (summary with a
  Date/Time line, a `# Transcripts` section, and a `# Suggestions` section), renders it to a
  `.docx` via `@mohtasham/md-to-docx` (centered H1/H5), and writes it via the Tauri dialog/fs
  plugins (`save` + `writeFile`) as `report-<timestamp>.docx`.
- **Clear all**: empties `transcripts`, `liveSuggestions`, and `actionSuggestions` in `AppState`
  (used at the start of each assistant run and from the UI).
- **Set placeholder data**: populates `AppState` with sample transcript/suggestion entries (used
  for first-run/demo display); this is also the shape of `AppState::default()`.

## Push Notifications

`PushNotificationService` emits a `push-notification` event (`{ message, type: error | warning |
success | info }`). `MainFrame` listens globally and renders the message as a `sonner` toast.
Used for permission/validation errors (e.g. "Cannot capture screenshot when assistant is not
running", max screenshots reached, upload failures).

## Auto Updater

- On launch, `lib.rs` schedules a background task that calls `check_and_download_update` 3s
  after startup, then every 4 hours.
- `check_and_download_update` checks the configured updater endpoint
  (`tauri-plugin-updater`, manifest at
  `https://github.com/PowerInterviewAI/client-app/releases/latest/download/latest.json`). If an
  update is available, it is **downloaded but not installed**, and the bytes are held in a
  process-wide `PENDING_UPDATE` static. Emits `auto-updater:status` with `{ status: "downloaded",
  version }` or `{ status: "error", error }`.
- `updater_quit_and_install` (user-triggered, e.g. from `UpdateNotification`) saves the current
  window bounds (since the install step bypasses the normal close handler), then calls
  `update.install(bytes)`. On Windows this exits the process immediately and the installer
  relaunches the app; on macOS/Linux the app explicitly calls `app.restart()` afterward.
- `updater_check_for_updates` lets the UI trigger an immediate check; `updater_get_version`
  returns the running app version.

## Permissions

Microphone and screen-recording permissions are checked and requested via
[`tauri-plugin-macos-permissions`](https://github.com/ayangweb/tauri-plugin-macos-permissions),
exposed to the renderer through the `permissions` bridge (`src/lib/tauri-bridge.ts`):

- **Checks** (`checkMicrophone`, `checkScreenRecording`) use the plugin's native APIs
  (`AVCaptureDevice.authorizationStatus`, `CGPreflightScreenCaptureAccess`) and return a bool.
  Off macOS they return `true` (always granted).
- **Requests** (`requestMicrophone`, `requestScreenRecording`) call
  `AVCaptureDevice.requestAccess` / `CGRequestScreenCaptureAccess`, which trigger the native
  prompt **and register the app in the System Settings Privacy lists**. The start flow checks,
  then requests when not granted.
- Screen recording gates both the system-audio loopback (ScreenCaptureKit) and the
  coding-challenge screenshots (`xcap`). macOS applies a newly granted screen-recording grant
  only after the app relaunches.

`src-tauri/src/commands/permissions.rs` now only renders the guidance dialogs:
`permissions_show_denied_dialog` shows a native error dialog with an "Open System Settings"
button that deep-links to the relevant Privacy pane; `permissions_show_restart_dialog` asks the
user to restart the app after granting a permission.

## Build and Release Workflow

The workflow at `.github/workflows/manual-cross-platform-release.yml`:

- builds on Windows and macOS in parallel
- installs pnpm dependencies
- runs `pnpm tauri:build` (which builds the frontend via `beforeBuildCommand` automatically)
- uploads bundle artifacts
- generates `latest.json` for the updater (`.github/scripts/generate-latest-json.mjs`)
- publishes a GitHub release when the `publish` input is enabled

## Platform Support

- Windows 11+
- macOS 14.4+

## Notes for Developers

- The face-swap/video-avatar feature is present in the config/types but force-disabled by
  `ConfigStore::migrate` (`face_swap = false`); `VideoGroup` is commented out in the control
  panel.
- Update native dependencies in `src-tauri/Cargo.toml` and frontend dependencies in
  `package.json`.
- Package manager is pnpm, do not use npm or yarn.
