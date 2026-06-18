import { useEffect } from 'react';

// Run the warm-up at most once per app session.
let warmedUp = false;

/**
 * Proactively request the microphone permission the assistant needs, once, at startup
 * ("warm-up"), so the user grants it up front instead of mid-interview. The OS still shows its
 * one-time prompt; this only triggers it early.
 *
 * - Microphone (Windows + macOS): `getUserMedia` triggers the native prompt, then the stream is
 *   released immediately. There is no way to grant the mic with no prompt at all, the OS always
 *   asks once; after that it is silent.
 *
 * Screen Recording (macOS) is intentionally NOT warmed up here: it is only needed for the
 * optional coding-challenge screenshot feature, so prompting every user at login is wasteful and
 * (when macOS reports the grant as missing) shows the prompt even to users who already allowed
 * it. It is requested lazily the first time a screenshot is actually captured.
 *
 * Note: macOS system-audio (CoreAudio tap / "Audio Capture") is likewise not warmed up here; it
 * prompts on the first assistant start.
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
    })();
  }, [enabled]);
}
