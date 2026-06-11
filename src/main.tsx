import './index.css';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import App from './App.tsx';
import { tauriApi } from './lib/tauri-bridge.ts';

// Expose the Tauri bridge as window.tauriApi for all hooks/components to use.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(window as any).tauriApi = tauriApi;

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>
);
