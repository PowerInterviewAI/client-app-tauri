/**
 * Tauri Bridge
 * Implements the TauriApi interface (src/types/tauri-api.d.ts) using Tauri's
 * invoke() and listen() under the hood. Assigned to window.tauriApi
 * in main.tsx so all hooks/components can use it via getTauriApi().
 */

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { save } from '@tauri-apps/plugin-dialog';
import { writeFile } from '@tauri-apps/plugin-fs';

// `@mohtasham/md-to-docx` statically imports `undici` (for Node-only image fetching),
// and undici reads `process.versions.node` / `process.platform` / etc. at module-evaluation
// time. The Tauri webview has no `process` global, so loading that chunk throws
// "process is not defined" before our code ever runs. Stub just enough of `process`
// for undici's module init to complete; the image-fetch path itself is never used here.
function ensureProcessPolyfill(): void {
  const target = globalThis as { process?: unknown };
  if (target.process !== undefined) return;
  target.process = {
    env: {},
    platform: 'browser',
    arch: 'x64',
    version: 'v20.0.0',
    versions: { node: '20.0.0' },
    emitWarning: () => {},
    nextTick: (fn: (...args: unknown[]) => void, ...args: unknown[]) => {
      queueMicrotask(() => fn(...args));
    },
  };
}

// Helper: subscribe to a Tauri event and return a synchronous cleanup fn
function onEvent<T>(event: string, callback: (payload: T) => void): () => void {
  const unlistenPromise = listen<T>(event, (e) => callback(e.payload));
  return () => {
    unlistenPromise.then((unlisten) => unlisten());
  };
}

// Helper: wrap a Tauri command in the `{ success, data, error }` shape that
// the hooks (use-auth, use-payment, llm-group, ...) expect.
async function invokeResult<T = unknown>(
  cmd: string,
  args?: Record<string, unknown>
): Promise<{ success: boolean; data?: T; error?: string }> {
  try {
    const data = await invoke<T>(cmd, args);
    return { success: true, data };
  } catch (error) {
    return { success: false, error: String(error) };
  }
}

export const tauriApi = {
  // ---- Hotkey events ----
  onHotkeyScroll: (callback: (section: string, direction: string) => void) =>
    onEvent<{ section: string; direction: string }>('hotkey-scroll', ({ section, direction }) =>
      callback(section, direction)
    ),

  onHotkeyStopAssistant: (callback: () => void) =>
    onEvent<void>('hotkey-stop-assistant', () => callback()),

  // ---- Configuration ----
  config: {
    get: () => invoke('config_get'),
    update: (updates: unknown) => invoke('config_update', { updates }),
  },

  // ---- Auth ----
  auth: {
    signup: (username: string, email: string, password: string) =>
      invokeResult('auth_signup', { username, email, password }),
    login: (email: string, password: string) => invokeResult('auth_login', { email, password }),
    logout: () => invokeResult('auth_logout'),
    changePassword: (currentPassword: string, newPassword: string) =>
      invokeResult('auth_change_password', { currentPassword, newPassword }),
  },

  // ---- Payment ----
  payment: {
    getPlans: async () => {
      const result = await invokeResult<Array<Record<string, unknown>>>('payment_get_plans');
      if (!result.success) return { success: false, error: result.error };
      // Backend returns price_usd (snake_case); the renderer expects priceUsd.
      const data = (result.data ?? []).map(({ price_usd, ...rest }) => ({
        ...rest,
        priceUsd: price_usd,
      }));
      return { success: true, data };
    },
    getCurrencies: () => invokeResult('payment_get_currencies'),
    create: (data: unknown) => invokeResult('payment_create', { data }),
    getStatus: (paymentId: string) => invokeResult('payment_get_status', { paymentId }),
    getHistory: () => invokeResult('payment_get_history'),
    getCredits: async () => {
      const result = await invokeResult<{ credits?: number }>('payment_get_credits');
      if (!result.success) return { success: false, error: result.error };
      return { success: true, credits: result.data?.credits ?? 0 };
    },
  },

  // ---- LLM ----
  llm: {
    listModels: () => invokeResult('llm_list_models'),
    validate: (config: unknown) => invokeResult('llm_validate', { config }),
  },

  // ---- App State ----
  appState: {
    get: () => invoke('app_state_get'),
    update: (updates: unknown) => invoke('app_state_update', { updates }),
  },

  onAppStateUpdated: (callback: (state: unknown) => void) => onEvent('app-state-updated', callback),

  // ---- Transcription ----
  transcription: {
    clear: () => invoke('transcription_clear'),
    start: () => invoke('transcription_start'),
    stop: () => invoke('transcription_stop'),
    ingest: (payload: { channel: string; type: string; text: string }) =>
      invoke('transcription_ingest', {
        channel: payload.channel,
        transcriptType: payload.type,
        text: payload.text,
      }),
    setSessionToken: (token: string) => invoke('transcription_set_session_token', { token }),
    enableLoopbackAudio: () => invoke('enable_loopback_audio'),
    disableLoopbackAudio: () => invoke('disable_loopback_audio'),
  },

  // ---- Live Suggestion ----
  liveSuggestion: {
    clear: () => invoke('live_suggestion_clear'),
    stop: () => invoke('live_suggestion_stop'),
  },

  // ---- Action Suggestion ----
  actionSuggestion: {
    clear: () => invoke('action_suggestion_clear'),
    stop: () => invoke('action_suggestion_stop'),
  },

  // ---- Push Notifications ----
  onPushNotification: (callback: (notification: unknown) => void) =>
    onEvent('push-notification', callback),

  // ---- Tools ----
  tools: {
    exportTranscript: async () => {
      // Rust gathers the data and the LLM-generated summary; JS assembles the
      // Markdown and renders the DOCX (keeps the md-to-docx dependency on the frontend).
      const data = await invoke<ExportData>('tools_get_export_data');
      const markdown = buildExportMarkdown(data);
      try {
        ensureProcessPolyfill();
        const { convertMarkdownToDocx } = await import('@mohtasham/md-to-docx');
        const blob: Blob = await convertMarkdownToDocx(markdown, {
          documentType: 'document',
          style: {
            heading1Alignment: 'CENTER',
            heading5Alignment: 'CENTER',
          },
        });
        const arrayBuffer = await blob.arrayBuffer();
        const bytes = new Uint8Array(arrayBuffer);
        const filePath = await save({
          filters: [{ name: 'Word Document', extensions: ['docx'] }],
          defaultPath: generateExportFilename(),
        });
        if (filePath) {
          await writeFile(filePath, bytes);
        }
      } catch (e) {
        console.error('[Bridge] exportTranscript failed:', e);
        throw e;
      }
    },
    clearAll: () => invoke('tools_clear_all'),
    setPlaceholderData: () => invoke('tools_set_placeholder_data'),
  },

  // ---- Auto Updater ----
  autoUpdater: {
    checkForUpdates: () => invoke('updater_check_for_updates'),
    quitAndInstall: () => invoke('updater_quit_and_install'),
    getVersion: () => invoke('updater_get_version'),
    onStatusUpdate: (callback: (data: unknown) => void) => onEvent('auto-updater:status', callback),
  },

  // ---- Window Controls ----
  close: () => invoke('window_close'),

  zoom: {
    increase: () => invoke('zoom_in'),
    decrease: () => invoke('zoom_out'),
    reset: () => invoke('zoom_reset'),
    getFactor: () => invoke('zoom_get_factor'),
    onChange: (callback: (percent: number) => void) => onEvent('zoom-level-changed', callback),
  },

  // ---- Permissions ----
  // The Rust commands return objects ({ status } / { granted }); unwrap them here so
  // callers receive the bare string/boolean the typed API (tauri-api.d.ts) promises.
  permissions: {
    checkScreenRecording: () =>
      invoke('permissions_check_screen_recording').then(
        (r) => (r as { status?: string })?.status ?? 'unknown'
      ),
    checkScreenSources: () =>
      invoke('permissions_check_screen_sources').then((r) => !!(r as { granted?: boolean })?.granted),
    checkMicrophone: () =>
      invoke('permissions_check_microphone').then(
        (r) => (r as { status?: string })?.status ?? 'unknown'
      ),
    requestMicrophone: () =>
      invoke('permissions_request_microphone').then((r) => !!(r as { granted?: boolean })?.granted),
    showDeniedDialog: (type: 'screen-recording' | 'microphone') =>
      invoke('permissions_show_denied_dialog', { permission_type: type }),
    showRestartDialog: () => invoke('permissions_show_restart_dialog'),
  },

  // ---- External URLs ----
  openExternal: (url: string) => invoke('open_external', { url }),

  // ---- Stealth ----
  setStealth: (isStealth: boolean) => invoke('window_set_stealth', { isStealth }),
  toggleStealth: () => invoke('window_toggle_stealth'),
  toggleOpacity: () => invoke('window_toggle_opacity'),

  // ---- Drag ----
  startDrag: () => invoke('window_start_drag'),

  ping: () => {},
};

// Subscribe to stealth-changed events and apply body class (mirrors preload.cts behavior)
listen<boolean>('stealth-changed', (event) => {
  if (event.payload) {
    document.body.classList.add('stealth');
  } else {
    document.body.classList.remove('stealth');
  }
}).catch(() => {});

// Types for internal use
interface TranscriptItem {
  timestamp: number;
  text: string;
  speaker: string;
  isFinal: boolean;
  endTimestamp: number;
}

interface LiveSuggestionItem {
  timestamp: number;
  last_question: string;
  answer: string;
}

// Mirrors the Rust `ExportData` struct returned by `tools_get_export_data`.
interface ExportData {
  username: string;
  summary: string;
  transcripts: TranscriptItem[];
  liveSuggestions: LiveSuggestionItem[];
}

// Builds the "smart" export document: an LLM summary (with a Date/Time line),
// followed by the full transcript and the suggestions, matching the original
// power-interview-client report layout.
function buildExportMarkdown(data: ExportData): string {
  const username = data.username || 'You';

  // Summary section: insert a Date/Time line right after the summary's first line.
  let summaryPart = data.summary ?? '';
  if (summaryPart) {
    const lines = summaryPart.split('\n');
    const datetimeNow = new Date().toLocaleString();
    lines.splice(1, 0, `\n##### Date/Time: ${datetimeNow}`);
    summaryPart = lines.join('\n');
  }

  // Transcripts section.
  const transcriptLines = data.transcripts.map((t) => {
    const timeStr = new Date(t.timestamp).toLocaleString();
    const speakerName = t.speaker === 'other' ? 'Interviewer' : username;
    return `#### ***${timeStr} | ${speakerName}***\n${t.text}\n`;
  });
  const transcriptsPart = `# **Transcripts**\n\n${transcriptLines.join('\n')}`;

  // Suggestions section (paired question + suggested answer).
  const suggestionLines = data.liveSuggestions.map((s) => {
    const timeStr = new Date(s.timestamp).toLocaleString();
    return `#### ***${timeStr} | Interviewer***\n${s.last_question}\n\n#### ***Suggestion***\n${s.answer}\n`;
  });
  const suggestionsPart = `# **Suggestions**\n\n${suggestionLines.join('\n')}`;

  return [
    summaryPart,
    data.transcripts.length > 0 ? transcriptsPart : '',
    data.liveSuggestions.length > 0 ? suggestionsPart : '',
  ]
    .join('\n\n')
    .trim();
}

// Timestamped report filename, e.g. "report-2026-06-12_01-04-01.docx".
function generateExportFilename(): string {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, '0');
  const date = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  const time = `${pad(d.getHours())}-${pad(d.getMinutes())}-${pad(d.getSeconds())}`;
  return `report-${date}_${time}.docx`;
}
