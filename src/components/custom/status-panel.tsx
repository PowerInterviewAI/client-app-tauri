import { Keyboard } from 'lucide-react';
import { useEffect, useState } from 'react';

import CreditsDisplay from '@/components/custom/credits-display';
import useIsStealthMode from '@/hooks/use-is-stealth-mode';
import { formatHotkey, Hotkey, HOTKEY_GROUPS, HOTKEYS } from '@/lib/hotkeys';
import { RunningState } from '@/types/app-state';

import { RunningIndicator } from './running-indicator';

type Props = {
  runningState: RunningState;
  credits: number;
  llmModel: string;
};

export default function StatusPanel({ runningState, llmModel, credits }: Props) {
  // The hotkeys panel works in both modes: the Ctrl+Shift+H global hotkey toggles it. The status
  // row (running indicator + credits + button) is a stealth-only HUD.
  const isStealth = useIsStealthMode();
  const [showHotkeys, setShowHotkeys] = useState(false);

  useEffect(() => {
    if (!window?.tauriApi?.onHotkeyToggleHotkeys) return;
    return window.tauriApi.onHotkeyToggleHotkeys(() => setShowHotkeys((v) => !v));
  }, []);

  // Raise window opacity while the modal is open so it stays readable if the window is dimmed,
  // then restore on close. Handled natively and works in any mode.
  useEffect(() => {
    window?.tauriApi?.setHotkeysOverlay?.(showHotkeys);
  }, [showHotkeys]);

  return (
    <>
      {isStealth && (
        <div
          id="status-panel"
          className="flex items-center justify-between text-muted-foreground p-1"
        >
          <RunningIndicator runningState={runningState} />
          <CreditsDisplay credits={credits} llmModel={llmModel} className="ml-2" />
          <div className="flex-1" />
          <button
            onClick={() => setShowHotkeys((v) => !v)}
            className="h-6 flex items-center justify-center rounded border border-border/50 text-muted-foreground bg-muted/50 text-xs font-bold gap-1 px-2"
            aria-label="Hotkeys"
            title={`Show keyboard shortcuts (${formatHotkey(HOTKEYS[Hotkey.ToggleHotkeys])})`}
          >
            <Keyboard className="h-4 w-4" /> Show Hotkeys (
            {formatHotkey(HOTKEYS[Hotkey.ToggleHotkeys])})
          </button>
        </div>
      )}

      {showHotkeys && (
        // Centered modal. Uses the normal (non-inverted) theme so it doesn't look reversed.
        // Click the backdrop to close (works in normal mode; stealth is click-through).
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
          onClick={() => setShowHotkeys(false)}
        >
          <div
            className="w-[92%] max-w-4xl rounded-lg border border-border bg-popover text-popover-foreground p-4 shadow-xl"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="mb-3 flex items-center justify-between">
              <h3 className="flex items-center gap-2 text-sm font-semibold text-foreground">
                <Keyboard className="h-4 w-4" /> Keyboard Shortcuts
              </h3>
              <span className="text-[10px] text-muted-foreground">
                {formatHotkey(HOTKEYS[Hotkey.ToggleHotkeys])} to close
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
                          <div className="rounded px-1.5 py-0.5 text-[11px] font-medium bg-muted text-foreground">
                            {formatHotkey(info)}
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
    </>
  );
}
