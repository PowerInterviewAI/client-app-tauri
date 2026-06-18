import { RefreshCcw, ZoomIn, ZoomOut } from 'lucide-react';
import { useEffect, useState } from 'react';

import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { formatHotkey, type HotkeyInfo } from '@/lib/hotkeys';

// Per-key zoom combos. The Rust backend registers these globally with a Ctrl+Shift base on
// Windows/Linux and Ctrl+Option (Alt) on macOS (see register_hotkeys in src-tauri/src/lib.rs);
// formatHotkey renders the right one (and mac modifier symbols) for the current platform.
const ZOOM_RESET: HotkeyInfo = {
  combo: 'Ctrl+Shift+0',
  comboMac: 'Ctrl+Alt+0',
  title: 'Reset zoom',
  description: '',
};
const ZOOM_IN: HotkeyInfo = {
  combo: 'Ctrl+Shift+=',
  comboMac: 'Ctrl+Alt+=',
  title: 'Zoom in',
  description: '',
};
const ZOOM_OUT: HotkeyInfo = {
  combo: 'Ctrl+Shift+-',
  comboMac: 'Ctrl+Alt+-',
  title: 'Zoom out',
  description: '',
};

/**
 * Render the set of zoom buttons that live in the titlebar.
 *
 * This component handles its own state (current zoom percent) and
 * communicates with the Tauri bridge API directly.  The
 * parent (Titlebar) simply places the controls in the layout and
 * is free to show/hide an enclosing divider if necessary.
 */
export default function ZoomControl() {
  const [zoomPercent, setZoomPercent] = useState(100);

  useEffect(() => {
    const api = window.tauriApi;
    if (!api?.zoom) return;

    api.zoom
      .getFactor()
      .then((f) => setZoomPercent(Math.round(f * 100)))
      .catch(() => {});

    const cleanup = api.zoom.onChange((p) => setZoomPercent(p));
    return cleanup;
  }, []);

  const handleZoomIn = () => {
    window.tauriApi?.zoom.increase();
  };

  const handleZoomOut = () => {
    window.tauriApi?.zoom.decrease();
  };

  const handleZoomReset = () => {
    window.tauriApi?.zoom.reset();
  };

  return (
    <div className="flex items-center gap-1">
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            onClick={handleZoomReset}
            aria-label="Reset zoom"
            title="Reset zoom"
            className="h-7 w-16 flex items-center justify-center rounded hover:bg-muted"
            // eslint-disable-next-line
            style={{ WebkitAppRegion: 'no-drag' } as any}
          >
            <RefreshCcw className="h-4 w-4" />
            <span className="ml-1 text-xs">{zoomPercent}%</span>
          </button>
        </TooltipTrigger>
        <TooltipContent>
          <p>Reset zoom ({formatHotkey(ZOOM_RESET)})</p>
        </TooltipContent>
      </Tooltip>

      <Tooltip>
        <TooltipTrigger asChild>
          <button
            onClick={handleZoomIn}
            aria-label="Zoom in"
            title="Zoom in"
            className="h-7 w-7 flex items-center justify-center rounded hover:bg-muted"
            // eslint-disable-next-line
            style={{ WebkitAppRegion: 'no-drag' } as any}
          >
            <ZoomIn className="h-4 w-4" />
          </button>
        </TooltipTrigger>
        <TooltipContent>
          <p>Zoom in ({formatHotkey(ZOOM_IN)})</p>
        </TooltipContent>
      </Tooltip>

      <Tooltip>
        <TooltipTrigger asChild>
          <button
            onClick={handleZoomOut}
            aria-label="Zoom out"
            title="Zoom out"
            className="h-7 w-7 flex items-center justify-center rounded hover:bg-muted"
            // eslint-disable-next-line
            style={{ WebkitAppRegion: 'no-drag' } as any}
          >
            <ZoomOut className="h-4 w-4" />
          </button>
        </TooltipTrigger>
        <TooltipContent>
          <p>Zoom out ({formatHotkey(ZOOM_OUT)})</p>
        </TooltipContent>
      </Tooltip>
    </div>
  );
}
