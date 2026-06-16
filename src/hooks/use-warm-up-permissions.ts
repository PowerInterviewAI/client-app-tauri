import { useEffect } from 'react';

import { getTauriApi, isMacPlatform } from '@/lib/utils';

// Run the warm-up at most once per app session.
let warmedUp = false;

/**
 * Proactively request the permissions the assistant needs, once, at startup ("warm-up"), so the
 * user grants them up front instead of mid-interview. The OS still shows its one-time prompts;
 * this only triggers them early.
 *
 * - Microphone (Windows + macOS): `getUserMedia` triggers the native prompt, then the stream is
 *   released immediately. There is no way to grant the mic with no prompt at all, the OS always
 *   asks once; after that it is silent.
 * - Screen Recording (macOS only): needed for coding-challenge screenshots; requested via the
 *   plugin, which also registers the app in System Settings. Not a concept on Windows.
 *
 * Note: macOS system-audio (CoreAudio tap / "Audio Capture") is not warmed up here; it prompts on
 * the first assistant start.
 */
export function useWarmUpPermissions(enabled: boolean) {
  useEffect(() => {
    if (!enabled || warmedUp) return;
    warmedUp = true;

    void (async () => {
      // Microphone, both platforms.
      try {
        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
        stream.getTracks().forEach((track) => track.stop());
      } catch {
        // Denied/unavailable for now; the start flow surfaces it later.
      }

      // Screen recording, macOS only.
      if (isMacPlatform()) {
        try {
          const tauri = getTauriApi();
          if (tauri && !(await tauri.permissions.checkScreenRecording())) {
            await tauri.permissions.requestScreenRecording();
          }
        } catch {
          // ignore
        }
      }
    })();
  }, [enabled]);
}
