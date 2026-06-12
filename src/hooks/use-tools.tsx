import { useState } from 'react';

import { getTauriApi } from '@/lib/utils';

export default function useTools() {
  const [exporting, setExporting] = useState(false);

  const exportTranscript = async () => {
    setExporting(true);
    try {
      const tauri = getTauriApi();
      if (!tauri) {
        throw new Error('Tauri API not available');
      }
      return await tauri.tools.exportTranscript();
    } catch (error) {
      console.error('Failed to export transcript:', error);
      throw error;
    } finally {
      setExporting(false);
    }
  };

  const openPath = async (path: string) => {
    const tauri = getTauriApi();
    if (!tauri) {
      throw new Error('Tauri API not available');
    }
    await tauri.tools.openPath(path);
  };

  const revealPath = async (path: string) => {
    const tauri = getTauriApi();
    if (!tauri) {
      throw new Error('Tauri API not available');
    }
    await tauri.tools.revealPath(path);
  };

  const clearAll = async () => {
    const tauri = getTauriApi();
    if (!tauri) {
      throw new Error('Tauri API not available');
    }
    await tauri.tools.clearAll();
  };

  const setPlaceholderData = async () => {
    const tauri = getTauriApi();
    if (!tauri) {
      throw new Error('Tauri API not available');
    }
    await tauri.tools.setPlaceholderData();
  };

  return {
    exporting,
    exportTranscript,
    openPath,
    revealPath,
    clearAll,
    setPlaceholderData,
  } as const;
}
