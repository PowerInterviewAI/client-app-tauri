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

export const useAssistantService = create<AssistantService>((set) => ({
  error: null,
  videoPanelRef: null,

  startAssistant: async () => {
    const tauri = getTauriApi();
    if (!tauri) {
      throw new Error('Tauri API not available');
    }

    try {
      set({ error: null });

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
      // Reset state to Idle so the button doesn't stay stuck on "Starting..."
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
  },

  stopAssistant: async () => {
    try {
      set({ error: null });

      const tauri = getTauriApi();
      if (!tauri) {
        throw new Error('Tauri API not available');
      }
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
      const errorMessage = getErrorMessage(error, 'Failed to stop assistant');
      set({
        error: errorMessage,
      });
      console.error('Stop assistant error:', error);
      throw error;
    }
  },

  setError: (error: string | null) => {
    set({ error });
  },

  setVideoPanelRef: (ref: React.RefObject<VideoPanelHandle> | null) => {
    set({ videoPanelRef: ref });
  },
}));
