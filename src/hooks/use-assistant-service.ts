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

      // Microphone is handled by getUserMedia() in liveTranscriptionService.start() below: it
      // triggers the native prompt, awaits the user's response, and registers the app. A denial
      // surfaces as NotAllowedError and is handled in the catch, so we do not pre-flight the mic
      // here (a non-awaiting plugin request would race the system prompt with our dialog).
      //
      // Screen recording gates system-audio capture (ScreenCaptureKit) and the coding-challenge
      // screenshots, and has no getUserMedia equivalent, so we pre-flight it. The check is the
      // accurate native state (CGPreflightScreenCaptureAccess; true off macOS). If not granted,
      // request it (prompts on first run and registers the app in System Settings), then guide
      // the user to enable it and restart, since macOS only applies the grant after relaunch.
      const screenGranted = await tauri.permissions.checkScreenRecording();
      if (!screenGranted) {
        await tauri.permissions.requestScreenRecording();
        await tauri.permissions.showDeniedDialog('screen-recording');
        return;
      }

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
