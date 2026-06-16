import { useEffect, useState } from 'react';

import { getTauriApi, isMacPlatform } from '@/lib/utils';
import { type AudioDevice } from '@/types/audio-device';

/**
 * Whether the page already holds microphone permission, without triggering a prompt.
 *
 * macOS WKWebView doesn't support the Permissions API for 'microphone', so there we use the
 * native check (tauri-plugin-macos-permissions, accurate on macOS). Elsewhere we use the web
 * Permissions API, which reports the real state on Chromium/WebView2, so we never auto-prompt.
 */
async function hasMicPermission(): Promise<boolean> {
  if (isMacPlatform()) {
    try {
      const tauri = getTauriApi();
      return tauri ? await tauri.permissions.checkMicrophone() : false;
    } catch {
      return false;
    }
  }
  try {
    const status = await navigator.permissions.query({
      name: 'microphone' as PermissionName,
    });
    return status.state === 'granted';
  } catch {
    return false;
  }
}

// Poll interval for hot-plug detection. The `devicechange` event is unreliable in WKWebView
// (often never fires on macOS), so we also poll and diff to catch plug/unplug there.
const DEVICE_POLL_MS = 2500;

function sameDevices(a: AudioDevice[], b: AudioDevice[]): boolean {
  return a.length === b.length && a.every((d, i) => d.name === b[i].name);
}

function useAudioDevices(kind: 'audioinput' | 'audiooutput', deviceType: string) {
  const [devices, setDevices] = useState<AudioDevice[]>([]);

  useEffect(() => {
    let cancelled = false;
    // Guard so the label-unlock stream is attempted at most once per mount.
    let unlockAttempted = false;

    async function enumerate() {
      try {
        const allDevices = await navigator.mediaDevices.enumerateDevices();
        if (cancelled) return;

        const matching = allDevices.filter((device) => device.kind === kind);

        // Browsers hide device labels (and macOS/WKWebView hides non-default devices
        // entirely) until the page has active media access. If we already hold mic
        // permission but labels are still blank, briefly open a stream once to unlock
        // them, then re-enumerate. Gated on existing permission so this never prompts.
        if (
          kind === 'audioinput' &&
          !unlockAttempted &&
          matching.length > 0 &&
          matching.every((device) => !device.label) &&
          (await hasMicPermission())
        ) {
          unlockAttempted = true;
          try {
            const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
            stream.getTracks().forEach((track) => track.stop());
            if (!cancelled) await enumerate();
            return;
          } catch {
            // Device busy or permission revoked between checks; use what we have.
          }
        }

        const filtered = matching
          .filter((device) => {
            const label = device.label.toLowerCase();
            return !label.includes('default') && !label.includes('communications');
          })
          .map((device, index) => ({
            name: device.label || `${deviceType} Device ${index + 1}`,
            index,
          }));
        // Only update when the list actually changed, so polling doesn't churn re-renders.
        setDevices((prev) => (sameDevices(prev, filtered) ? prev : filtered));
      } catch (error) {
        console.error(`Error enumerating ${kind} devices:`, error);
      }
    }

    enumerate();
    // Refresh on hot-plug (devicechange), on window focus (e.g. returning from System Settings),
    // and via polling, since devicechange is unreliable in WKWebView.
    navigator.mediaDevices.addEventListener('devicechange', enumerate);
    window.addEventListener('focus', enumerate);
    const poll = window.setInterval(enumerate, DEVICE_POLL_MS);

    return () => {
      cancelled = true;
      navigator.mediaDevices.removeEventListener('devicechange', enumerate);
      window.removeEventListener('focus', enumerate);
      window.clearInterval(poll);
    };
  }, [kind, deviceType]);

  return devices;
}

export function useAudioInputDevices() {
  return useAudioDevices('audioinput', 'Input');
}

export function useAudioOutputDevices() {
  return useAudioDevices('audiooutput', 'Output');
}
