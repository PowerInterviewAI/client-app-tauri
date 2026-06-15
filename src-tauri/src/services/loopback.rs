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
//! - macOS: CoreAudio process taps via `cidre` (Audio Capture permission, no Screen Recording).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
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

        *guard = Some(RunningLoopback {
            stop,
            capture,
            streamer,
        });
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

/// Build the `wss?://.../api/asr/streaming` URL from the configured backend base.
fn streaming_url() -> String {
    let ws_base = if let Some(rest) = BACKEND_BASE_URL.strip_prefix("https") {
        format!("wss{rest}")
    } else if let Some(rest) = BACKEND_BASE_URL.strip_prefix("http") {
        format!("ws{rest}")
    } else {
        BACKEND_BASE_URL.to_string()
    };
    format!("{}{}", ws_base.trim_end_matches('/'), API_ASR_STREAMING)
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
    let mut request = streaming_url()
        .into_client_request()
        .map_err(|e| e.to_string())?;
    if !token.is_empty() {
        // The backend's WS auth only checks the `session_token` cookie, not query params.
        let cookie = format!("session_token={token}");
        request.headers_mut().insert(
            "Cookie",
            cookie.parse().map_err(
                |e: tokio_tungstenite::tungstenite::http::header::InvalidHeaderValue| e.to_string(),
            )?,
        );
    }
    let (ws, _response) = tokio_tungstenite::connect_async(request)
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
            Ok(Ok(rate)) => Ok((
                rate,
                CaptureHandle {
                    thread: Some(thread),
                },
            )),
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
    //! System-audio capture via CoreAudio process taps (macOS 14.4+). A mono global tap is
    //! aggregated with the default output device and read through an IO proc. This uses the
    //! "Audio Capture" privacy permission, NOT Screen Recording, so it avoids ScreenCaptureKit's
    //! screen-recording requirement (and its restart-after-grant / capture-failure issues).
    //! Ported from Pluely's `speaker/macos.rs`.
    use super::ChunkSender;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use ca::aggregate_device_keys as agg_keys;
    use cidre::{arc, av, cat, cf, core_audio as ca, ns, os};

    /// Owns the capture thread; joining it drops the started device + tap, stopping capture.
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

    /// Client data for the CoreAudio IO proc. Boxed and kept alive for the device's lifetime.
    struct Ctx {
        format: arc::R<av::AudioFormat>,
        tx: ChunkSender,
        stop: Arc<AtomicBool>,
        sample_rate: Arc<AtomicU32>,
    }

    extern "C" fn proc(
        device: ca::Device,
        _now: &cat::AudioTimeStamp,
        input_data: &cat::AudioBufList<1>,
        _input_time: &cat::AudioTimeStamp,
        _output_data: &mut cat::AudioBufList<1>,
        _output_time: &cat::AudioTimeStamp,
        ctx: Option<&mut Ctx>,
    ) -> os::Status {
        let Some(ctx) = ctx else {
            return os::Status::NO_ERR;
        };
        ctx.sample_rate.store(
            device
                .actual_sample_rate()
                .unwrap_or(ctx.format.absd().sample_rate) as u32,
            Ordering::Release,
        );
        if ctx.stop.load(Ordering::Acquire) {
            return os::Status::NO_ERR;
        }
        // The tap is a mono global tap, so buffer 0 is the mono system-audio signal.
        if let Some(view) = av::AudioPcmBuf::with_buf_list_no_copy(&ctx.format, input_data, None) {
            if let Some(data) = view.data_f32_at(0) {
                if !data.is_empty() {
                    let _ = ctx.tx.try_send(data.to_vec());
                }
            }
        } else if ctx.format.common_format() == av::audio::CommonFormat::PcmF32 {
            let buffer = &input_data.buffers[0];
            let float_count = buffer.data_bytes_size as usize / std::mem::size_of::<f32>();
            if float_count > 0 && !buffer.data.is_null() {
                let data =
                    unsafe { std::slice::from_raw_parts(buffer.data as *const f32, float_count) };
                let _ = ctx.tx.try_send(data.to_vec());
            }
        }
        os::Status::NO_ERR
    }

    pub fn start_capture(
        tx: ChunkSender,
        stop: Arc<AtomicBool>,
    ) -> Result<(u32, CaptureHandle), String> {
        let (setup_tx, setup_rx) = std::sync::mpsc::channel::<Result<u32, String>>();
        let thread = std::thread::spawn(move || run_capture(tx, stop, setup_tx));
        match setup_rx.recv() {
            Ok(Ok(rate)) => Ok((
                rate,
                CaptureHandle {
                    thread: Some(thread),
                },
            )),
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
        // Mono global system-audio tap (excludes no processes).
        let tap_desc = ca::TapDesc::with_mono_global_tap_excluding_processes(&ns::Array::new());
        let tap = match tap_desc.create_process_tap() {
            Ok(tap) => tap,
            Err(e) => {
                let _ = setup_tx.send(Err(format!("System audio capture unavailable: {e}")));
                return;
            }
        };

        let output_uid = match ca::System::default_output_device().and_then(|d| d.uid()) {
            Ok(uid) => uid,
            Err(e) => {
                let _ = setup_tx.send(Err(format!("No default output device: {e}")));
                return;
            }
        };

        let sub_device = cf::DictionaryOf::with_keys_values(
            &[ca::sub_device_keys::uid()],
            &[output_uid.as_type_ref()],
        );
        let sub_tap = cf::DictionaryOf::with_keys_values(
            &[ca::sub_device_keys::uid()],
            &[tap.uid().unwrap().as_type_ref()],
        );
        let agg_desc = cf::DictionaryOf::with_keys_values(
            &[
                agg_keys::is_private(),
                agg_keys::is_stacked(),
                agg_keys::tap_auto_start(),
                agg_keys::name(),
                agg_keys::main_sub_device(),
                agg_keys::uid(),
                agg_keys::sub_device_list(),
                agg_keys::tap_list(),
            ],
            &[
                cf::Boolean::value_true().as_type_ref(),
                cf::Boolean::value_false(),
                cf::Boolean::value_true(),
                cf::str!(c"power-interview-system-audio-tap"),
                &output_uid,
                &cf::Uuid::new().to_cf_string(),
                &cf::ArrayOf::from_slice(&[sub_device.as_ref()]),
                &cf::ArrayOf::from_slice(&[sub_tap.as_ref()]),
            ],
        );

        let asbd = match tap.asbd() {
            Ok(asbd) => asbd,
            Err(e) => {
                let _ = setup_tx.send(Err(format!("Failed to read tap audio format: {e}")));
                return;
            }
        };
        let Some(format) = av::AudioFormat::with_asbd(&asbd) else {
            let _ = setup_tx.send(Err("Failed to build audio format".into()));
            return;
        };
        let sample_rate = Arc::new(AtomicU32::new(asbd.sample_rate as u32));

        let mut ctx = Box::new(Ctx {
            format,
            tx,
            stop: Arc::clone(&stop),
            sample_rate: Arc::clone(&sample_rate),
        });

        let agg_device = match ca::AggregateDevice::with_desc(&agg_desc) {
            Ok(device) => device,
            Err(e) => {
                let _ = setup_tx.send(Err(format!("Failed to create aggregate device: {e}")));
                return;
            }
        };
        let proc_id = match agg_device.create_io_proc_id(proc, Some(&mut ctx)) {
            Ok(id) => id,
            Err(e) => {
                let _ = setup_tx.send(Err(format!("Failed to create audio IO proc: {e}")));
                return;
            }
        };
        let started = match ca::device_start(agg_device, Some(proc_id)) {
            Ok(started) => started,
            Err(e) => {
                let _ = setup_tx.send(Err(format!("Failed to start audio capture: {e}")));
                return;
            }
        };

        let _ = setup_tx.send(Ok(sample_rate.load(Ordering::Acquire)));

        // Keep the started device, IO-proc context, and tap alive until asked to stop.
        while !stop.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(50));
        }

        drop(started);
        drop(ctx);
        drop(tap);
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
