import { create } from 'zustand';

import { type VideoPanelHandle } from '@/components/custom/video-panel';
import { getTauriApi } from '@/lib/utils';
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

      // Pre-flight permission checks before any state change.
      //
      // Microphone: only hard-block when the OS reports an explicit denial. For
      // 'not-determined'/'unknown' we let getUserMedia() trigger the native OS prompt
      // in the start flow - that is the real request path on both platforms (there is
      // no reliable programmatic request API in the webview).
      // Screen recording: check only - the OS dialog fires automatically when
      // getDisplayMedia() is called.
      const micStatus = await tauri.permissions.checkMicrophone();
      if (micStatus === 'denied' || micStatus === 'restricted') {
        await tauri.permissions.showDeniedDialog('microphone');
        return;
      }

      // desktopCapturer.getSources() returns [] when screen recording is denied,
      // causing getDisplayMedia() to hang indefinitely - guard against it here.
      const screenStatus = await tauri.permissions.checkScreenRecording();
      if (screenStatus === 'denied' || screenStatus === 'restricted') {
        await tauri.permissions.showDeniedDialog('screen-recording');
        return;
      }

      // On macOS, even when getMediaAccessStatus returns 'granted', desktopCapturer.getSources()
      // returns [] if the app hasn't been restarted since permission was first granted.
      // Detect this early to avoid a 20-second getDisplayMedia timeout and show a clear message.
      if (screenStatus === 'granted') {
        const hasSources = await tauri.permissions.checkScreenSources();
        if (!hasSources) {
          await tauri.permissions.showRestartDialog();
          return;
        }
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

      const errorMessage = error instanceof Error ? error.message : 'Failed to start assistant';
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
      const errorMessage = error instanceof Error ? error.message : 'Failed to stop assistant';
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
