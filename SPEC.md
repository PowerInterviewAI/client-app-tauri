# SPEC.md

Technical specification for the Power Interview Tauri desktop client. See [CLAUDE.md](CLAUDE.md) for the day-to-day command and convention summary.

## Overview

The app is built as a Tauri desktop client with a React frontend, targeting Windows and macOS.

## Architecture

### Frontend

- `src/` - React, Tailwind, hooks, components, pages.
- `src/lib/tauri-bridge.ts` exposes the IPC compatibility API used by renderer hooks.

### Native Backend

- `src-tauri/src/` - Tauri command handlers, services, state, and native utilities.
- `src-tauri/tauri.conf.json` - macOS and Windows bundle settings.
- `src-tauri/Cargo.toml` - Rust dependency manifest.

### IPC Bridge

- Tauri `invoke()` is exposed through `tauriApi` and assigned to `window.electronAPI` for compatibility.
- Transcription, permissions, payment, config, and window control are handled through Tauri commands.

## Audio Capture and Transcription

Audio capture and STT streaming run **in the renderer**, not in Rust:

- Microphone audio is captured with `getUserMedia`; system/interviewer audio
  (loopback) is captured with `getDisplayMedia({ audio: true })`, after which the
  video track is stopped and dropped. See `src/services/live-transcription.service.ts`.
- Each channel resamples to 16 kHz mono PCM16 in an `AudioWorklet` and streams it over
  a WebSocket to the backend ASR endpoint (`/api/asr/streaming`). Partial/final
  transcripts come back as JSON.
- Channels map to speakers as: `ch_1` = microphone = self, `ch_0` = loopback =
  interviewer. The renderer forwards transcripts to Rust via `transcription_ingest`,
  where `src-tauri/src/services/transcript.rs` aggregates, de-dupes, and merges them.

The `enable_loopback_audio` / `disable_loopback_audio` commands in
`src-tauri/src/commands/transcription.rs` do **not** capture audio. `enable_loopback_audio`
only performs a macOS screen-recording permission pre-check (Windows is a no-op), and
`disable_loopback_audio` is retained for IPC symmetry.

### Platform note: macOS loopback

`getDisplayMedia` system-audio capture is well supported on Windows (Chromium WebView2)
but is unreliable in the macOS WKWebView. Loopback (interviewer) audio capture on macOS
should be verified on real hardware; if unsupported, it requires a native capture path.

## Key Implementation Notes

- Electron has been removed from the repository.
- The build flows are now Tauri-first.
- macOS screen recording permission is validated natively before loopback capture.
- The GitHub Actions workflow builds Tauri bundles for Windows and macOS.

## Build and Release Workflow

The workflow at `.github/workflows/manual-cross-platform-release.yml`:

- builds on Windows and macOS in parallel
- installs pnpm dependencies
- runs `pnpm tauri:build` (which builds the frontend via `beforeBuildCommand` automatically)
- uploads bundle artifacts
- publishes a GitHub release when the `publish` input is enabled

## Platform Support

- Windows 11+
- macOS 14.4+

## Notes for Developers

- There is no `src/main/` Electron host code in this repo anymore.
- Use the Tauri app as the single desktop implementation.
- Update native dependencies in `src-tauri/Cargo.toml` and frontend dependencies in `package.json`.
- Package manager is pnpm - do not use npm or yarn.
