use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use crate::consts::{MIN_HEIGHT, MIN_WIDTH};

fn set_window_opacity(win: &tauri::WebviewWindow, opacity: f64) {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        extern "system" {
            fn GetWindowLongW(hwnd: isize, index: i32) -> i32;
            fn SetWindowLongW(hwnd: isize, index: i32, value: i32) -> i32;
            fn SetLayeredWindowAttributes(hwnd: isize, key: u32, alpha: u8, flags: u32) -> i32;
        }
        const GWL_EXSTYLE: i32 = -20;
        const WS_EX_LAYERED: i32 = 0x80000;
        const LWA_ALPHA: u32 = 2;
        if let Ok(handle) = win.window_handle() {
            if let RawWindowHandle::Win32(h) = handle.as_raw() {
                let hwnd = h.hwnd.get() as isize;
                unsafe {
                    let ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
                    SetWindowLongW(hwnd, GWL_EXSTYLE, ex | WS_EX_LAYERED);
                    SetLayeredWindowAttributes(
                        hwnd,
                        0,
                        (opacity.clamp(0.0, 1.0) * 255.0) as u8,
                        LWA_ALPHA,
                    );
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(handle) = win.window_handle() {
            if let RawWindowHandle::AppKit(h) = handle.as_raw() {
                unsafe {
                    use objc2::msg_send;
                    use objc2::runtime::AnyObject;
                    let ns_view = h.ns_view.as_ptr() as *mut AnyObject;
                    let ns_window: *mut AnyObject = msg_send![ns_view, window];
                    if !ns_window.is_null() {
                        let _: () = msg_send![ns_window, setAlphaValue: opacity as f64];
                    }
                }
            }
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (win, opacity);
    }
}

/// Apply window opacity on the main/UI thread. `set_window_opacity` calls AppKit
/// (`setAlphaValue:`) and Win32 window APIs, which must run on the main thread; this is
/// often invoked from the global-shortcut callback (a background thread), so marshal it.
/// `run_on_main_thread` runs synchronously if already on the main thread.
fn apply_window_opacity(win: &tauri::WebviewWindow, opacity: f64) {
    let win = win.clone();
    let handle = win.app_handle().clone();
    let _ = handle.run_on_main_thread(move || set_window_opacity(&win, opacity));
}

/// Re-apply the Dock icon after returning from `Accessory` to `Regular` activation policy.
///
/// AppKit rebuilds the Dock tile from `NSApp.applicationIconImage` (not the bundle's
/// `CFBundleIconFile`) on that transition, so it drops our icon and shows the generic
/// placeholder. We embed the app icon and set it explicitly. Must run on the main thread.
///
/// The `NSImage` is built once and cached for the process lifetime (the app icon is
/// long-lived, so the single `alloc`/`init` reference is intentionally never released).
#[cfg(target_os = "macos")]
fn restore_dock_icon() {
    use std::sync::OnceLock;

    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    const ICON_BYTES: &[u8] = include_bytes!("../../icons/icon.png");
    // Pointer stored as usize so it is Send; only ever touched on the main thread.
    static ICON: OnceLock<usize> = OnceLock::new();

    let image = *ICON.get_or_init(|| unsafe {
        // NSData *data = [NSData dataWithBytes:length:] (autoreleased, not owned)
        let data: *mut AnyObject = msg_send![
            class!(NSData),
            dataWithBytes: ICON_BYTES.as_ptr() as *const std::ffi::c_void,
            length: ICON_BYTES.len()
        ];
        if data.is_null() {
            return 0;
        }
        // NSImage *image = [[NSImage alloc] initWithData:data] (owned +1, kept forever)
        let image: *mut AnyObject = msg_send![class!(NSImage), alloc];
        let image: *mut AnyObject = msg_send![image, initWithData: data];
        image as usize
    }) as *mut AnyObject;

    if image.is_null() {
        return;
    }
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, setApplicationIconImage: image];
    }
}

use crate::services::app_state::AppStateService;
use crate::services::push_notification::PushNotificationService;
use crate::store::ConfigStore;

// Window-opacity levels the stealth toggle cycles through. Dimming is applied to the
// native window itself (set_window_opacity), so these are the actual alpha values.
const OPACITY_LEVELS: [f64; 4] = [0.2, 0.5, 0.75, 0.9];
/// Opacity applied while the hotkeys modal is open in stealth, so it stays readable.
const HOTKEYS_OVERLAY_OPACITY: f64 = 0.9;
const MOVE_AMOUNT: i32 = 20;
const RESIZE_AMOUNT: i32 = 20;

pub struct WindowControlService {
    stealth: parking_lot::Mutex<bool>,
    opacity_index: parking_lot::Mutex<usize>,
    // Generation counter for the held move/resize auto-repeat (see bump_repeat/stop_repeat).
    arrow_repeat: Arc<AtomicU64>,
    app_handle: AppHandle,
    app_state: Arc<AppStateService>,
    push_notification: Arc<PushNotificationService>,
    config_store: Arc<ConfigStore>,
}

impl WindowControlService {
    pub fn new(
        app_handle: AppHandle,
        app_state: Arc<AppStateService>,
        push_notification: Arc<PushNotificationService>,
        config_store: Arc<ConfigStore>,
    ) -> Self {
        config_store.set_stealth(false); // always start non-stealth
                                         // Restore the last-used opacity level (clamped in case the count changed), default mid-high.
        let opacity_index = config_store
            .get_opacity_level()
            .map(|i| i % OPACITY_LEVELS.len())
            .unwrap_or(2);
        Self {
            stealth: parking_lot::Mutex::new(false),
            opacity_index: parking_lot::Mutex::new(opacity_index),
            arrow_repeat: Arc::new(AtomicU64::new(0)),
            app_handle,
            app_state,
            push_notification,
            config_store,
        }
    }

    fn window(&self) -> Option<tauri::WebviewWindow> {
        self.app_handle.get_webview_window("main")
    }

    pub fn enable_stealth(&self) {
        let Some(win) = self.window() else { return };
        let _ = win.set_ignore_cursor_events(true);
        let _ = win.set_always_on_top(true);
        // Apply the current window-opacity level when entering stealth.
        apply_window_opacity(&win, OPACITY_LEVELS[*self.opacity_index.lock()]);
        *self.stealth.lock() = true;
        self.config_store.set_stealth(true);
        self.app_state.set_stealth(true);
        let _ = self.app_handle.emit("stealth-changed", true);

        #[cfg(target_os = "macos")]
        {
            // hide dock icon in stealth mode
            use tauri::ActivationPolicy;
            let _ = self
                .app_handle
                .set_activation_policy(ActivationPolicy::Accessory);
        }
    }

    pub fn disable_stealth(&self) {
        let Some(win) = self.window() else { return };
        let _ = win.set_ignore_cursor_events(false);
        let _ = win.set_always_on_top(false);
        apply_window_opacity(&win, 1.0);
        let _ = win.show();
        let _ = win.set_focus();
        *self.stealth.lock() = false;
        self.config_store.set_stealth(false);
        self.app_state.set_stealth(false);
        let _ = self.app_handle.emit("stealth-changed", false);

        #[cfg(target_os = "macos")]
        {
            use tauri::ActivationPolicy;
            let _ = self
                .app_handle
                .set_activation_policy(ActivationPolicy::Regular);
            // Re-apply the Dock icon, which AppKit drops on the Accessory -> Regular
            // transition. Marshal to the main thread (toggle may run on a shortcut thread).
            let _ = self.app_handle.run_on_main_thread(restore_dock_icon);
        }
    }

    pub fn toggle_stealth(&self) {
        if !self.app_state.get_state().is_logged_in.unwrap_or(false) {
            self.push_notification
                .error("You must be logged in to use stealth mode.");
            return;
        }
        if *self.stealth.lock() {
            self.disable_stealth();
        } else {
            self.enable_stealth();
        }
    }

    pub fn toggle_opacity(&self) {
        let Some(win) = self.window() else { return };
        if !*self.stealth.lock() {
            self.push_notification
                .warning("Opacity toggle is only available in stealth mode.");
            return;
        }
        let level = {
            let mut idx = self.opacity_index.lock();
            *idx = (*idx + 1) % OPACITY_LEVELS.len();
            *idx
        };
        // Persist so the level is restored on the next launch / next stealth entry.
        self.config_store.save_opacity_level(level);
        // Apply the new opacity to the native window.
        apply_window_opacity(&win, OPACITY_LEVELS[level]);
    }

    pub fn set_stealth(&self, enabled: bool) {
        if enabled {
            self.enable_stealth()
        } else {
            self.disable_stealth()
        }
    }

    /// Keep the hotkeys modal readable under stealth dimming: raise opacity to
    /// `HOTKEYS_OVERLAY_OPACITY` while it is open, then restore the saved stealth level when it
    /// closes. No-op outside stealth, where the window is already fully opaque.
    pub fn set_hotkeys_overlay(&self, open: bool) {
        if !*self.stealth.lock() {
            return;
        }
        let Some(win) = self.window() else { return };
        let opacity = if open {
            HOTKEYS_OVERLAY_OPACITY
        } else {
            OPACITY_LEVELS[*self.opacity_index.lock()]
        };
        apply_window_opacity(&win, opacity);
    }

    pub fn move_to_position(&self, position: &str) {
        let Some(win) = self.window() else { return };
        let Ok(_scale) = win.scale_factor() else {
            return;
        };
        let Ok(size) = win.inner_size() else { return };
        let monitor = win
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| win.primary_monitor().ok().flatten());
        let Some(monitor) = monitor else { return };
        let screen_size = monitor.size();
        let screen_pos = monitor.position();
        let win_w = size.width as i32;
        let win_h = size.height as i32;
        let screen_w = screen_size.width as i32;
        let screen_h = screen_size.height as i32;
        let base_x = screen_pos.x;
        let base_y = screen_pos.y;

        let (x, y) = match position {
            "top-left" => (0, 0),
            "top-center" => ((screen_w - win_w) / 2, 0),
            "top-right" => (screen_w - win_w, 0),
            "middle-left" => (0, (screen_h - win_h) / 2),
            "center" => ((screen_w - win_w) / 2, (screen_h - win_h) / 2),
            "middle-right" => (screen_w - win_w, (screen_h - win_h) / 2),
            "bottom-left" => (0, screen_h - win_h),
            "bottom-center" => ((screen_w - win_w) / 2, screen_h - win_h),
            "bottom-right" => (screen_w - win_w, screen_h - win_h),
            _ => ((screen_w - win_w) / 2, (screen_h - win_h) / 2),
        };

        let _ = win.set_position(tauri::PhysicalPosition::new(base_x + x, base_y + y));
    }

    pub fn move_by_arrow(&self, direction: &str) {
        let Some(win) = self.window() else { return };
        let Ok(pos) = win.outer_position() else {
            return;
        };
        let (nx, ny) = match direction {
            "up" => (pos.x, pos.y - MOVE_AMOUNT),
            "down" => (pos.x, pos.y + MOVE_AMOUNT),
            "left" => (pos.x - MOVE_AMOUNT, pos.y),
            "right" => (pos.x + MOVE_AMOUNT, pos.y),
            _ => (pos.x, pos.y),
        };
        let _ = win.set_position(tauri::PhysicalPosition::new(nx, ny));
    }

    pub fn resize_by_arrow(&self, direction: &str) {
        let Some(win) = self.window() else { return };
        let Ok(size) = win.inner_size() else { return };
        let w = size.width as i32;
        let h = size.height as i32;
        let (nw, nh) = match direction {
            "up" => (w, (h - RESIZE_AMOUNT).max(MIN_HEIGHT as i32)),
            "down" => (w, h + RESIZE_AMOUNT),
            "left" => ((w - RESIZE_AMOUNT).max(MIN_WIDTH as i32), h),
            "right" => (w + RESIZE_AMOUNT, h),
            _ => (w, h),
        };
        let _ = win.set_size(tauri::PhysicalSize::new(nw as u32, nh as u32));
    }

    // ---- Held move/resize auto-repeat ----
    // Global shortcuts only fire once per physical press (no OS key-repeat), so holding an
    // arrow wouldn't keep moving/resizing. The hotkey handler calls `bump_repeat()` on press to
    // start a loop keyed to the returned token, and `stop_repeat()` on release to end it.

    /// Start a new repeat generation; returns its token. A running loop continues only while
    /// `repeat_token()` still equals its token.
    pub fn bump_repeat(&self) -> u64 {
        self.arrow_repeat.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn repeat_token(&self) -> u64 {
        self.arrow_repeat.load(Ordering::Acquire)
    }

    /// Invalidate any running repeat loop (on key release).
    pub fn stop_repeat(&self) {
        self.arrow_repeat.fetch_add(1, Ordering::AcqRel);
    }

    pub fn is_stealth(&self) -> bool {
        *self.stealth.lock()
    }
}
