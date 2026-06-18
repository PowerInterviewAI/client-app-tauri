import { create } from 'zustand';

import { type VideoPanelHandle } from '@/components/custom/video-panel';
import { getErrorMessage, getTauriApi } from '@/lib/utils';
import { liveTranscriptionService } from '@/services/live-transcription.service';
import { RunningState } from '@/types/app-state';

import { useConfigStore } from './use-config-store';

interface AssistantService {
  error: string | null;
  videoPanelRef: React.RefObject<VideoPanelHandle> | null;

  // Actions
  startAssistant: () => Promise<void>;
  stopAssistant: () => Promise<void>;
  setError: (error: string | null) => void;
  setVideoPanelRef: (ref: React.RefObject<VideoPanelHandle> | null) => void;
}

// Serialize start/stop transitions. The native macOS system-audio capture (CoreAudio process
// tap in loopback.rs) and the mic AudioContext must never be created and torn down at the same
// time: the global stop hotkey can fire mid-start, interleaving enableLoopbackAudio() with
// disableLoopbackAudio() and orphaning a process tap. Orphaned global taps accumulate and
// eventually crash the macOS audio HAL (the "stop then start rapidly" crash). Chaining every
// transition through a single promise guarantees a stop always runs to completion before the
// next start (and vice versa).
let transitionChain: Promise<void> = Promise.resolve();
// Logical "assistant is active" flag, flipped only inside a queued transition. It blocks a
// duplicate start from opening a second mic stream, and makes a stray stop (e.g. the stop
// hotkey pressed while already idle) a safe no-op.
let assistantActive = false;

function enqueueTransition(task: () => Promise<void>): Promise<void> {
  const run = transitionChain.then(task, task);
  // Keep the chain alive even if a task rejects, so later transitions still run.
  transitionChain = run.then(
    () => undefined,
    () => undefined
  );
  return run;
}

export const useAssistantService = create<AssistantService>((set) => ({
  error: null,
  videoPanelRef: null,

  startAssistant: () =>
    enqueueTransition(async () => {
      const tauri = getTauriApi();
      if (!tauri) {
        throw new Error('Tauri API not available');
      }
      // Already running or mid-start: ignore the duplicate so we never open a second
      // mic stream / loopback tap.
      if (assistantActive) return;

      try {
        set({ error: null });
        assistantActive = true;

        // No permission pre-flight here, both inputs request/surface their own permission:
        //   - Microphone: getUserMedia() in liveTranscriptionService.start() triggers the native
        //     prompt, awaits the response, and registers the app; a denial becomes NotAllowedError
        //     (handled in the catch below).
        //   - System audio: captured via CoreAudio process taps (loopback.rs), gated by the Audio
        //     Capture permission (not Screen Recording). A tap failure surfaces from
        //     enableLoopbackAudio() and is handled in liveTranscriptionService with a guidance
        //     dialog. Screen Recording is only needed for coding-challenge screenshots, which
        //     prompt when the user actually captures one.

        tauri.appState.update({ runningState: RunningState.Starting });

        // Clear previous history
        await tauri.tools.clearAll();

        const config = useConfigStore.getState().config;

        // Start transcription services
        await tauri.transcription.start();
        await liveTranscriptionService.start(
          config?.audioInputDeviceName ?? '',
          config?.sessionToken ?? ''
        );

        // Sleep 3 seconds to ensure the assistant has fully started before allowing stop actions
        await new Promise((resolve) => setTimeout(resolve, 3000));

        // Update running state to Running after successful start
        tauri.appState.update({ runningState: RunningState.Running });
      } catch (error) {
        // Roll back any partially-started services in the same serialized step so nothing
        // (mic stream, native loopback tap, Rust transcript) is left running, then return to
        // Idle so the button doesn't stay stuck on "Starting...".
        assistantActive = false;
        try {
          await liveTranscriptionService.stop();
          await tauri.transcription.stop();
        } catch (cleanupError) {
          console.error('Start rollback cleanup failed:', cleanupError);
        }
        tauri.appState.update({ runningState: RunningState.Idle });

        // A microphone denial at the native getUserMedia prompt surfaces here as
        // NotAllowedError - show the actionable dialog instead of a generic message.
        if (error instanceof DOMException && error.name === 'NotAllowedError') {
          await tauri.permissions.showDeniedDialog('microphone');
          set({ error: 'Microphone permission denied' });
          return;
        }

        const errorMessage = getErrorMessage(error, 'Failed to start assistant');
        set({ error: errorMessage });
        console.error('Start assistant error:', error);
        throw error;
      }
    }),

  stopAssistant: () =>
    enqueueTransition(async () => {
      const tauri = getTauriApi();
      if (!tauri) {
        throw new Error('Tauri API not available');
      }
      // Already idle or mid-stop: ignore the stray stop (e.g. stop hotkey pressed twice, or
      // while not running) so we don't tear down state that isn't set up.
      if (!assistantActive) return;

      try {
        set({ error: null });
        assistantActive = false;

        tauri.appState.update({ runningState: RunningState.Stopping });

        // Stop assistant services
        await Promise.all([
          liveTranscriptionService.stop(),
          tauri.transcription.stop(),
          tauri.liveSuggestion.stop(),
          tauri.actionSuggestion.stop(),
        ]);

        tauri.setStealth(false); // Ensure stealth mode is turned off when stopping assistant

        // Sleep 3 seconds to ensure the assistant has fully stopped before allowing start actions
        await new Promise((resolve) => setTimeout(resolve, 3000));

        // Update running state to Idle after successful stop
        tauri.appState.update({ runningState: RunningState.Idle });
      } catch (error) {
        // Even on a stop error the assistant must end up Idle, not stuck on "Stopping...".
        tauri.appState.update({ runningState: RunningState.Idle });
        const errorMessage = getErrorMessage(error, 'Failed to stop assistant');
        set({
          error: errorMessage,
        });
        console.error('Stop assistant error:', error);
        throw error;
      }
    }),

  setError: (error: string | null) => {
    set({ error });
  },

  setVideoPanelRef: (ref: React.RefObject<VideoPanelHandle> | null) => {
    set({ videoPanelRef: ref });
  },
}));
