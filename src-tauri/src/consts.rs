// Backend URL - set via environment variable or defaults
pub const BACKEND_BASE_URL: &str = if cfg!(debug_assertions) {
    "http://localhost:8080"
} else {
    "https://api.powerinterviewai.com"
};

// User-Agent sent with every backend request, including app version, OS, OS version
// and CPU arch. Examples:
//   Windows: "power-interview-ai/2.0.0 (Windows 10.0.26200; x86_64)"
//   macOS:   "power-interview-ai/2.0.0 (Mac OS 15.1.0; aarch64)"
// Computed once at first use since the OS info requires a runtime lookup.
pub fn app_user_agent() -> &'static str {
    use std::sync::OnceLock;
    static UA: OnceLock<String> = OnceLock::new();
    UA.get_or_init(|| {
        let info = os_info::get();
        format!(
            "{}/{} ({} {}; {})",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            info.os_type(),
            info.version(),
            std::env::consts::ARCH,
        )
    })
}

// API endpoints
pub const API_AUTH_SIGNUP: &str = "/api/auth/signup";
pub const API_AUTH_LOGIN: &str = "/api/auth/login";
pub const API_AUTH_LOGOUT: &str = "/api/auth/logout";
pub const API_AUTH_CHANGE_PASSWORD: &str = "/api/auth/change-password";
pub const API_HEALTH_CHECK_PING: &str = "/api/health-check/ping";
pub const API_HEALTH_CHECK_PING_CLIENT: &str = "/api/health-check/ping-client";
pub const API_LLM_MODELS: &str = "/api/llm/models";
pub const API_LLM_VALIDATE: &str = "/api/llm/validate";
pub const API_LLM_UPLOAD_IMAGE: &str = "/api/llm/upload-image";
pub const API_LLM_ACTION_SUGGESTION: &str = "/api/llm/action-suggestion";
pub const API_LLM_LIVE_SUGGESTION: &str = "/api/llm/live-suggestion";
pub const API_ASR_STREAMING: &str = "/api/asr/streaming";
pub const API_LLM_GET_THUMB: &str = "/api/llm/get-thumb";
pub const API_PAYMENT_PLANS: &str = "/api/payment/plans";
pub const API_PAYMENT_CURRENCIES: &str = "/api/payment/currencies";
pub const API_PAYMENT_CREATE: &str = "/api/payment/create";
pub const API_PAYMENT_STATUS: &str = "/api/payment/status";
pub const API_PAYMENT_HISTORY: &str = "/api/payment/history";

pub const MIN_WIDTH: u32 = 860;
pub const MIN_HEIGHT: u32 = 540;

pub const TRANSCRIPT_INTER_TRANSCRIPT_GAP_MS: i64 = 5_000;
pub const LIVE_SUGGESTION_GAP_MS: i64 = 2_000;
pub const LIVE_SUGGESTION_NO_SUGGESTION: &str = "NO_SUGGESTION_NEEDED";

pub const ACTION_SUGGESTION_MAX_CAPTURES: u32 = 4;

pub const ZOOM_STEP: f64 = 0.1;
pub const ZOOM_MIN_FACTOR: f64 = 0.5;
pub const ZOOM_MAX_FACTOR: f64 = 3.0;
