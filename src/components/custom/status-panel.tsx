import { Keyboard } from 'lucide-react';
import { useEffect, useState } from 'react';

import CreditsDisplay from '@/components/custom/credits-display';
import { Hotkey, HOTKEY_GROUPS, HOTKEYS, formatCombo } from '@/lib/hotkeys';
import { cn } from '@/lib/utils';
import { RunningState } from '@/types/app-state';

import { RunningIndicator } from './running-indicator';

type Props = {
  runningState: RunningState;
  credits: number;
  llmModel: string;
};

export default function StatusPanel({ runningState, llmModel, credits }: Props) {
  // In stealth the window is click-through (set_ignore_cursor_events), so the webview
  // never receives hover/click events. The Ctrl+Shift+H global hotkey toggles the
  // centered hotkeys modal below (the button is just a visible affordance).
  const [showHotkeys, setShowHotkeys] = useState(false);

  useEffect(() => {
    if (!window?.tauriApi?.onHotkeyToggleHotkeys) return;
    return window.tauriApi.onHotkeyToggleHotkeys(() => setShowHotkeys((v) => !v));
  }, []);

  return (
    <div id="status-panel" className="flex items-center justify-between text-muted-foreground p-1">
      <RunningIndicator runningState={runningState} />
      <CreditsDisplay credits={credits} llmModel={llmModel} className="ml-2" />
      <div className="flex-1" />
      <button
        onClick={() => setShowHotkeys((v) => !v)}
        className="h-6 flex items-center justify-center rounded border border-border/50 text-muted-foreground bg-muted/50 text-xs font-bold gap-1 px-2"
        aria-label="Hotkeys"
        title={`Show keyboard shortcuts (${formatCombo('Ctrl+Shift+H')})`}
      >
        <Keyboard className="h-4 w-4" /> Show Hotkeys ({formatCombo('Ctrl+Shift+H')})
      </button>

      {showHotkeys && (
        // Centered modal. Uses the normal (non-inverted) theme so it doesn't look reversed.
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
          <div className="w-[92%] max-w-4xl rounded-lg border border-border bg-popover text-popover-foreground p-4 shadow-xl">
            <div className="mb-3 flex items-center justify-between">
              <h3 className="flex items-center gap-2 text-sm font-semibold text-foreground">
                <Keyboard className="h-4 w-4" /> Keyboard Shortcuts
              </h3>
              <span className="text-[10px] text-muted-foreground">
                {formatCombo('Ctrl+Shift+H')} to close
              </span>
            </div>
            <div className="space-y-3">
              {HOTKEY_GROUPS.map((group) => (
                <div key={group.label}>
                  <div className="mb-1 text-[10px] font-semibold uppercase text-muted-foreground">
                    {group.label}
                  </div>
                  <div className="grid grid-cols-3 gap-1">
                    {group.keys.map((hk) => {
                      const info = HOTKEYS[hk];
                      return (
                        <div key={hk} className="flex items-center gap-1">
                          <div
                            className={cn(
                              'rounded px-1 py-0.5 text-[11px] font-semibold',
                              hk === Hotkey.StopAll
                                ? 'bg-destructive text-destructive-foreground'
                                : hk === Hotkey.ToggleStealth
                                  ? 'bg-primary text-primary-foreground'
                                  : 'bg-muted text-foreground'
                            )}
                          >
                            {formatCombo(info.combo)}
                          </div>
                          <div className="text-[11px] font-medium text-foreground">
                            {info.title}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
