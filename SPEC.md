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

Capture is split by source: the microphone runs in the renderer, system audio runs natively.

**Microphone (`ch_1`, self):** captured in the renderer with `getUserMedia`, resampled to
16 kHz mono PCM16 in an `AudioWorklet`, and streamed over a WebSocket to the ASR endpoint
(`/api/asr/streaming`). Transcripts are forwarded to Rust via `transcription_ingest`. See
`src/services/live-transcription.service.ts`.

**System/interviewer audio (`ch_0`, other):** captured natively in Rust, because
`getDisplayMedia` system-audio capture is unreliable in the macOS WKWebView. See
`src-tauri/src/services/loopback.rs`:

- Windows: WASAPI loopback via `cpal` (the default render endpoint opened as an input
  device, which transparently captures system output).
- macOS: ScreenCaptureKit audio capture (requires screen-recording permission).
- Captured audio is downmixed to mono, resampled to 16 kHz PCM16 (streaming linear
  resampler), and streamed over its own WebSocket to the same ASR endpoint. Returned
  transcripts are ingested directly as `ch_0`, with no renderer round-trip.

The renderer starts/stops native capture through the `enable_loopback_audio` /
`disable_loopback_audio` commands. Both the mic and loopback channels feed the same
`src-tauri/src/services/transcript.rs`, which aggregates, de-dupes, and merges them.

### Platform note: macOS capture is unverified locally

The macOS ScreenCaptureKit path is gated behind `#[cfg(target_os = "macos")]` and is not
compiled by `cargo check` on Windows. It is written against the documented
`screencapturekit` 1.x API and must be validated by the macOS CI build / on real hardware.

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
