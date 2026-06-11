//! Native system-audio (loopback) capture and streaming.
//!
//! The interviewer/system-audio channel (`ch_0`) is captured natively instead of in
//! the renderer, because `getDisplayMedia` system-audio capture is unreliable in the
//! macOS WKWebView. Captured audio is downmixed to mono, resampled to 16 kHz PCM16, and
//! streamed over a WebSocket to the ASR endpoint; returned transcripts are fed straight
//! into the shared [`TranscriptService`] as the `ch_0` (interviewer) channel.
//!
//! Capture backends:
//! - Windows: WASAPI loopback via `cpal` (the default render endpoint opened as input).
//! - macOS: ScreenCaptureKit audio capture.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::consts::{API_ASR_STREAMING, BACKEND_BASE_URL};
use crate::services::transcript::TranscriptService;
use crate::store::ConfigStore;

const OUTPUT_SAMPLE_RATE: u32 = 16_000;
/// Bound the capture->stream queue so a stalled socket can't grow memory without limit.
/// Each item is one capture callback buffer (~10 ms), so this is a few seconds of slack.
const CAPTURE_QUEUE_CAPACITY: usize = 256;
const WS_RECONNECT_MAX_DELAY_MS: u64 = 8_000;

/// One capture callback's worth of mono samples in [-1.0, 1.0].
type MonoChunk = Vec<f32>;
type ChunkSender = mpsc::Sender<MonoChunk>;

pub struct LoopbackService {
    transcript: Arc<TranscriptService>,
    config_store: Arc<ConfigStore>,
    running: Mutex<Option<RunningLoopback>>,
}

struct RunningLoopback {
    stop: Arc<AtomicBool>,
    capture: capture::CaptureHandle,
    streamer: tauri::async_runtime::JoinHandle<()>,
}

impl LoopbackService {
    pub fn new(transcript: Arc<TranscriptService>, config_store: Arc<ConfigStore>) -> Self {
        Self {
            transcript,
            config_store,
            running: Mutex::new(None),
        }
    }

    /// Start native loopback capture and streaming. Idempotent: a no-op if already running.
    pub fn start(&self) -> Result<(), String> {
        let mut guard = self.running.lock();
        if guard.is_some() {
            return Ok(());
        }

        let token = self.config_store.get_config().session_token;
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel::<MonoChunk>(CAPTURE_QUEUE_CAPACITY);

        // Capture setup is synchronous so device/permission errors surface to the caller.
        let (source_rate, capture) = capture::start_capture(tx, Arc::clone(&stop))?;

        let transcript = Arc::clone(&self.transcript);
        let stop_for_stream = Arc::clone(&stop);
        let streamer = tauri::async_runtime::spawn(async move {
            run_streamer(rx, source_rate, token, transcript, stop_for_stream).await;
        });

        *guard = Some(RunningLoopback { stop, capture, streamer });
        Ok(())
    }

    /// Stop capture and streaming. Idempotent.
    pub fn stop(&self) {
        if let Some(running) = self.running.lock().take() {
            running.stop.store(true, Ordering::Release);
            running.capture.join(); // joins the capture thread, dropping the native stream
            running.streamer.abort();
        }
    }
}

/// Build the `wss?://.../api/asr/streaming?token=...` URL from the configured backend base.
fn streaming_url(token: &str) -> String {
    let ws_base = if let Some(rest) = BACKEND_BASE_URL.strip_prefix("https") {
        format!("wss{rest}")
    } else if let Some(rest) = BACKEND_BASE_URL.strip_prefix("http") {
        format!("ws{rest}")
    } else {
        BACKEND_BASE_URL.to_string()
    };
    let mut url = format!("{}{}", ws_base.trim_end_matches('/'), API_ASR_STREAMING);
    // Session tokens are JWT-style (URL-safe characters only), so no escaping is needed.
    if !token.is_empty() {
        url.push_str("?token=");
        url.push_str(token);
    }
    url
}

/// Reconnecting stream loop: pulls mono chunks, resamples to 16 kHz PCM16, and sends them;
/// concurrently reads transcript JSON back and ingests it as the `ch_0` channel.
async fn run_streamer(
    mut rx: mpsc::Receiver<MonoChunk>,
    source_rate: u32,
    token: String,
    transcript: Arc<TranscriptService>,
    stop: Arc<AtomicBool>,
) {
    let mut attempt: u32 = 0;
    while !stop.load(Ordering::Acquire) {
        match connect_and_stream(&mut rx, source_rate, &token, &transcript, &stop).await {
            Ok(CaptureEnded(true)) => break, // capture stopped; nothing left to stream
            Ok(CaptureEnded(false)) => {
                attempt = 0; // remote closed cleanly; reconnect
            }
            Err(e) => {
                log::warn!("[Loopback] streaming connection error: {e}");
            }
        }
        if stop.load(Ordering::Acquire) {
            break;
        }
        attempt = attempt.saturating_add(1);
        let delay = (1_000u64 << attempt.min(3)).min(WS_RECONNECT_MAX_DELAY_MS);
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    }
}

/// Returned by [`connect_and_stream`]: `true` when the local capture ended (stop the loop),
/// `false` when only the remote socket closed (reconnect).
struct CaptureEnded(bool);

async fn connect_and_stream(
    rx: &mut mpsc::Receiver<MonoChunk>,
    source_rate: u32,
    token: &str,
    transcript: &Arc<TranscriptService>,
    stop: &Arc<AtomicBool>,
) -> Result<CaptureEnded, String> {
    let url = streaming_url(token);
    let (ws, _response) = tokio_tungstenite::connect_async(url.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let (mut write, mut read) = ws.split();

    // Reader: transcript JSON -> TranscriptService.
    let transcript_reader = Arc::clone(transcript);
    let stop_reader = Arc::clone(stop);
    let reader = tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            if stop_reader.load(Ordering::Acquire) {
                break;
            }
            match msg {
                Ok(Message::Text(text)) => ingest_transcript(&text, &transcript_reader).await,
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
    });

    let mut resampler = LinearResampler::new(source_rate, OUTPUT_SAMPLE_RATE);
    let outcome = loop {
        match rx.recv().await {
            Some(chunk) => {
                if stop.load(Ordering::Acquire) {
                    break CaptureEnded(true);
                }
                let resampled = resampler.process(&chunk);
                if resampled.is_empty() {
                    continue;
                }
                let bytes = pcm16_le_bytes(&resampled);
                if let Err(e) = write.send(Message::Binary(bytes)).await {
                    reader.abort();
                    return Err(e.to_string());
                }
            }
            None => break CaptureEnded(true), // capture thread dropped the sender
        }
    };

    let _ = write.send(Message::Close(None)).await;
    reader.abort();
    Ok(outcome)
}

async fn ingest_transcript(text: &str, transcript: &TranscriptService) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return;
    };
    let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let content = value
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if (kind == "partial" || kind == "final") && !content.is_empty() {
        transcript.ingest("ch_0", kind, content).await;
    }
}

fn pcm16_le_bytes(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let scaled = if clamped < 0.0 {
            clamped * 32_768.0
        } else {
            clamped * 32_767.0
        };
        bytes.extend_from_slice(&(scaled as i16).to_le_bytes());
    }
    bytes
}

/// Streaming linear-interpolation resampler. Carries the last input sample across chunk
/// boundaries so there are no discontinuities between successive callback buffers.
struct LinearResampler {
    step: f64, // input samples advanced per output sample
    pos: f64,  // fractional read position within the current input buffer
    prev: f32, // last sample of the previous buffer (input index -1)
    primed: bool,
}

impl LinearResampler {
    fn new(in_rate: u32, out_rate: u32) -> Self {
        let in_rate = in_rate.max(1) as f64;
        let out_rate = out_rate.max(1) as f64;
        Self {
            step: in_rate / out_rate,
            pos: 0.0,
            prev: 0.0,
            primed: false,
        }
    }

    fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if input.is_empty() {
            return Vec::new();
        }
        let len = input.len() as f64;
        let sample = |index: isize| -> f32 {
            if index < 0 {
                self.prev
            } else {
                input[index as usize]
            }
        };

        let mut out = Vec::with_capacity(((len / self.step) as usize).saturating_add(1));
        loop {
            let i = self.pos.floor() as isize;
            let next = i + 1;
            if next > input.len() as isize - 1 {
                break;
            }
            let frac = (self.pos - i as f64) as f32;
            let a = sample(i);
            let b = sample(next);
            out.push(a + (b - a) * frac);
            self.pos += self.step;
        }

        // Shift the read position into the next buffer's coordinate space and carry the
        // final sample so index -1 is valid on the next call.
        self.pos -= len;
        self.prev = input[input.len() - 1];
        self.primed = true;
        out
    }
}

// ---- Platform capture backends -------------------------------------------------------

#[cfg(target_os = "windows")]
mod capture {
    use super::ChunkSender;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::SampleFormat;

    /// Owns the capture thread; joining it drops the underlying (non-Send) cpal stream.
    pub struct CaptureHandle {
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl CaptureHandle {
        pub fn join(mut self) {
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    /// Start WASAPI loopback capture. Returns the device sample rate and a join handle.
    pub fn start_capture(
        tx: ChunkSender,
        stop: Arc<AtomicBool>,
    ) -> Result<(u32, CaptureHandle), String> {
        let (setup_tx, setup_rx) = std::sync::mpsc::channel::<Result<u32, String>>();
        let thread = std::thread::spawn(move || run_capture(tx, stop, setup_tx));
        match setup_rx.recv() {
            Ok(Ok(rate)) => Ok((rate, CaptureHandle { thread: Some(thread) })),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => {
                let _ = thread.join();
                Err("Loopback capture thread exited during setup".into())
            }
        }
    }

    fn run_capture(
        tx: ChunkSender,
        stop: Arc<AtomicBool>,
        setup_tx: std::sync::mpsc::Sender<Result<u32, String>>,
    ) {
        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(d) => d,
            None => {
                let _ = setup_tx.send(Err("No default output device for loopback".into()));
                return;
            }
        };

        // The render endpoint's mix format is what loopback delivers.
        let supported = match device.default_output_config() {
            Ok(c) => c,
            Err(e) => {
                let _ = setup_tx.send(Err(format!("Failed to query output format: {e}")));
                return;
            }
        };
        let sample_format = supported.sample_format();
        let channels = supported.channels() as usize;
        let sample_rate = supported.sample_rate().0;
        let config: cpal::StreamConfig = supported.into();

        let err_fn = |e| log::error!("[Loopback] WASAPI stream error: {e}");

        // Opening an input stream on a render device transparently enables WASAPI loopback.
        let build = || -> Result<cpal::Stream, cpal::BuildStreamError> {
            match sample_format {
                SampleFormat::F32 => {
                    let tx = tx.clone();
                    device.build_input_stream(
                        &config,
                        move |data: &[f32], _| {
                            let _ = tx.try_send(downmix(data, channels, |s| s));
                        },
                        err_fn,
                        None,
                    )
                }
                SampleFormat::I16 => {
                    let tx = tx.clone();
                    device.build_input_stream(
                        &config,
                        move |data: &[i16], _| {
                            let _ = tx.try_send(downmix(data, channels, |s| s as f32 / 32_768.0));
                        },
                        err_fn,
                        None,
                    )
                }
                SampleFormat::U16 => {
                    let tx = tx.clone();
                    device.build_input_stream(
                        &config,
                        move |data: &[u16], _| {
                            let _ = tx.try_send(downmix(data, channels, |s| {
                                (s as f32 - 32_768.0) / 32_768.0
                            }));
                        },
                        err_fn,
                        None,
                    )
                }
                other => return Err(unsupported_format(other)),
            }
        };

        let stream = match build() {
            Ok(s) => s,
            Err(e) => {
                let _ = setup_tx.send(Err(format!("Failed to build loopback stream: {e}")));
                return;
            }
        };
        if let Err(e) = stream.play() {
            let _ = setup_tx.send(Err(format!("Failed to start loopback stream: {e}")));
            return;
        }

        let _ = setup_tx.send(Ok(sample_rate));

        // cpal delivers audio on its own threads; keep this thread alive (and the stream
        // owned here) until asked to stop, then drop the stream.
        while !stop.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(50));
        }
        drop(stream);
    }

    /// Downmix an interleaved buffer to mono by averaging channels, converting each
    /// sample to f32 via `convert`.
    fn downmix<T: Copy>(data: &[T], channels: usize, convert: impl Fn(T) -> f32) -> Vec<f32> {
        if channels <= 1 {
            return data.iter().map(|&s| convert(s)).collect();
        }
        let mut mono = Vec::with_capacity(data.len() / channels);
        for frame in data.chunks_exact(channels) {
            let sum: f32 = frame.iter().map(|&s| convert(s)).sum();
            mono.push(sum / channels as f32);
        }
        mono
    }

    fn unsupported_format(format: SampleFormat) -> cpal::BuildStreamError {
        log::error!("[Loopback] Unsupported loopback sample format: {format:?}");
        cpal::BuildStreamError::StreamConfigNotSupported
    }
}

#[cfg(target_os = "macos")]
mod capture {
    use super::ChunkSender;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use screencapturekit::prelude::*;

    pub struct CaptureHandle {
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl CaptureHandle {
        pub fn join(mut self) {
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    struct AudioHandler {
        tx: ChunkSender,
        stop: Arc<AtomicBool>,
    }

    impl SCStreamOutputTrait for AudioHandler {
        fn did_output_sample_buffer(&self, sample: CMSampleBuffer, of_type: SCStreamOutputType) {
            match of_type {
                SCStreamOutputType::Audio => {}
                _ => return,
            }
            if self.stop.load(Ordering::Acquire) {
                return;
            }
            // ScreenCaptureKit delivers 32-bit float PCM. Use the first buffer as the mono
            // signal; for an interleaved buffer, take channel 0 of each frame.
            let Some(list) = sample.audio_buffer_list() else {
                return;
            };
            let Some(buffer) = list.get(0) else {
                return;
            };
            let bytes = buffer.data();
            let channels = (buffer.number_channels.max(1)) as usize;
            let frame_bytes = 4 * channels;
            if frame_bytes == 0 {
                return;
            }
            let mut mono = Vec::with_capacity(bytes.len() / frame_bytes);
            for frame in bytes.chunks_exact(frame_bytes) {
                let value = f32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]);
                mono.push(value);
            }
            if !mono.is_empty() {
                let _ = self.tx.try_send(mono);
            }
        }
    }

    pub fn start_capture(
        tx: ChunkSender,
        stop: Arc<AtomicBool>,
    ) -> Result<(u32, CaptureHandle), String> {
        let (setup_tx, setup_rx) = std::sync::mpsc::channel::<Result<u32, String>>();
        let thread = std::thread::spawn(move || run_capture(tx, stop, setup_tx));
        match setup_rx.recv() {
            Ok(Ok(rate)) => Ok((rate, CaptureHandle { thread: Some(thread) })),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => {
                let _ = thread.join();
                Err("Loopback capture thread exited during setup".into())
            }
        }
    }

    fn run_capture(
        tx: ChunkSender,
        stop: Arc<AtomicBool>,
        setup_tx: std::sync::mpsc::Sender<Result<u32, String>>,
    ) {
        const SAMPLE_RATE: u32 = 48_000;

        let content = match SCShareableContent::get() {
            Ok(c) => c,
            Err(e) => {
                let _ = setup_tx.send(Err(format!("Screen recording permission required: {e}")));
                return;
            }
        };
        let display = match content.displays().into_iter().next() {
            Some(d) => d,
            None => {
                let _ = setup_tx.send(Err("No display available for audio capture".into()));
                return;
            }
        };

        let filter = SCContentFilter::create()
            .with_display(&display)
            .with_excluding_windows(&[])
            .build();
        // Audio-only: SCK still produces video frames, but with no Screen handler they are
        // dropped. A tiny frame size keeps that overhead negligible.
        let config = SCStreamConfiguration::new()
            .with_captures_audio(true)
            .with_sample_rate(SAMPLE_RATE)
            .with_channel_count(1)
            .with_width(2)
            .with_height(2);

        let mut stream = SCStream::new(&filter, &config);
        stream.add_output_handler(
            AudioHandler {
                tx,
                stop: Arc::clone(&stop),
            },
            SCStreamOutputType::Audio,
        );

        if let Err(e) = stream.start_capture() {
            let _ = setup_tx.send(Err(format!("Failed to start audio capture: {e}")));
            return;
        }
        let _ = setup_tx.send(Ok(SAMPLE_RATE));

        while !stop.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = stream.stop_capture();
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod capture {
    use super::ChunkSender;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    pub struct CaptureHandle;

    impl CaptureHandle {
        pub fn join(self) {}
    }

    pub fn start_capture(
        _tx: ChunkSender,
        _stop: Arc<AtomicBool>,
    ) -> Result<(u32, CaptureHandle), String> {
        Err("Loopback capture is only supported on Windows and macOS".into())
    }
}
