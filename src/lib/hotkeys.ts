import { isMacPlatform } from '@/lib/utils';

export enum Hotkey {
  StopAll = 'StopAll',
  ToggleStealth = 'ToggleStealth',
  Opacity = 'Opacity',
  ToggleHotkeys = 'ToggleHotkeys',
  PlaceWin = 'PlaceWin',
  MoveWin = 'MoveWin',
  ResizeWin = 'ResizeWin',
  ZoomInOutReset = 'ZoomInOutReset',
  ScrollLiveSuggestionPanel = 'ScrollLiveSuggestionPanel',
  ScrollActionSuggestionPanel = 'ScrollActionSuggestionPanel',
  Capture = 'Capture',
  ClearCaptures = 'ClearCaptures',
  TriggerWithoutCaptures = 'TriggerWithoutCaptures',
  TriggerWithCaptures = 'TriggerWithCaptures',
}
export const HOTKEY_LIST: Hotkey[] = Object.values(Hotkey);

/**
 * Groups of hotkeys organized by functional area.  Useful for display
 * in menus or tooltips where related shortcuts should be clustered.
 */
export type HotkeyGroup = {
  label: string;
  keys: Hotkey[];
};

export const HOTKEY_GROUPS: HotkeyGroup[] = [
  {
    label: 'General',
    keys: [Hotkey.StopAll, Hotkey.ToggleStealth, Hotkey.Opacity, Hotkey.ToggleHotkeys],
  },
  {
    label: 'Window Management',
    keys: [Hotkey.PlaceWin, Hotkey.MoveWin, Hotkey.ResizeWin, Hotkey.ZoomInOutReset],
  },
  {
    label: 'Scroll Panels',
    keys: [Hotkey.ScrollLiveSuggestionPanel, Hotkey.ScrollActionSuggestionPanel],
  },
  {
    label: 'Triggered Suggestions',
    keys: [
      Hotkey.Capture,
      Hotkey.ClearCaptures,
      Hotkey.TriggerWithoutCaptures,
      Hotkey.TriggerWithCaptures,
    ],
  },
];

export interface HotkeyInfo {
  combo: string;
  comboMac: string;
  title: string;
  description: string;
}

// combo is the Windows/Linux key combination; comboMac is the macOS
// equivalent. The Rust backend registers a matching modifier set per
// platform (see register_hotkeys in src-tauri/src/lib.rs): the base
// modifier is Ctrl+Shift on Windows/Linux and Ctrl+Option (Alt) on macOS,
// move-window stays Ctrl+Alt+Shift on both, and resize-window is base+Win
// (Win = the Windows key or, on macOS, Cmd).
export const HOTKEYS: Record<Hotkey, HotkeyInfo> = {
  [Hotkey.StopAll]: {
    combo: 'Ctrl+Shift+Q',
    comboMac: 'Ctrl+Alt+Q',
    title: 'Stop All',
    description: 'Stop assistant and exit stealth mode',
  },
  [Hotkey.ToggleStealth]: {
    combo: 'Ctrl+Shift+M',
    comboMac: 'Ctrl+Alt+M',
    title: 'Toggle Stealth',
    description: 'Toggle stealth mode ON or OFF',
  },
  [Hotkey.Opacity]: {
    combo: 'Ctrl+Shift+N',
    comboMac: 'Ctrl+Alt+N',
    title: 'Toggle Opacity',
    description: 'Toggle window opacity in stealth mode',
  },
  [Hotkey.ToggleHotkeys]: {
    combo: 'Ctrl+Shift+H',
    comboMac: 'Ctrl+Alt+H',
    title: 'Show/Hide Hotkeys',
    description: 'Toggle this keyboard-shortcuts panel',
  },
  [Hotkey.PlaceWin]: {
    combo: 'Ctrl+Shift+[1-9]',
    comboMac: 'Ctrl+Alt+[1-9]',
    title: 'Place Window',
    description: 'Place window in a specific corner, side, or center',
  },
  [Hotkey.MoveWin]: {
    combo: 'Ctrl+Shift+Alt+Arrow',
    comboMac: 'Ctrl+Alt+Shift+Arrow',
    title: 'Move Window',
    description: 'Move window in the specified direction',
  },
  [Hotkey.ResizeWin]: {
    combo: 'Ctrl+Shift+Win+Arrow',
    comboMac: 'Ctrl+Alt+Win+Arrow',
    title: 'Resize Window',
    description: 'Resize window in the specified direction',
  },
  [Hotkey.ZoomInOutReset]: {
    combo: 'Ctrl+Shift+[=, -, 0]',
    comboMac: 'Ctrl+Alt+[=, -, 0]',
    title: 'Zoom In/Out/Reset',
    description: 'Adjust or reset UI zoom level',
  },
  [Hotkey.ScrollLiveSuggestionPanel]: {
    combo: 'Ctrl+Shift+[J, K, L]',
    comboMac: 'Ctrl+Alt+[J, K, L]',
    title: 'Scroll Live Panel',
    description: 'Scroll Down/Up/End in the live suggestions panel',
  },
  [Hotkey.ScrollActionSuggestionPanel]: {
    combo: 'Ctrl+Shift+[U, I, O]',
    comboMac: 'Ctrl+Alt+[U, I, O]',
    title: 'Scroll Triggered Panel',
    description: 'Scroll Down/Up/End in the triggered suggestions panel',
  },
  [Hotkey.Capture]: {
    combo: 'Ctrl+Shift+F9',
    comboMac: 'Ctrl+Alt+F9',
    title: 'Capture Screen',
    description: 'Take a screenshot for triggered suggestions',
  },
  [Hotkey.ClearCaptures]: {
    combo: 'Ctrl+Shift+F10',
    comboMac: 'Ctrl+Alt+F10',
    title: 'Clear Captures',
    description: 'Clear captured screenshots',
  },
  [Hotkey.TriggerWithoutCaptures]: {
    combo: 'Ctrl+Shift+F11',
    comboMac: 'Ctrl+Alt+F11',
    title: 'Trigger without Captures',
    description: 'Generate suggestion without captures',
  },
  [Hotkey.TriggerWithCaptures]: {
    combo: 'Ctrl+Shift+F12',
    comboMac: 'Ctrl+Alt+F12',
    title: 'Trigger with Captures',
    description:
      'Generate suggestion referencing screen captures. If no captures exist, attempts to take one before generating.',
  },
};

/**
 * Get the display string for a hotkey's combo on the current platform:
 * combo on Windows/Linux, comboMac on macOS with modifier names replaced by
 * their symbols (Ctrl, Alt, Shift, Win -> Control/Option/Shift/Command).
 */
export function formatHotkey(info: HotkeyInfo): string {
  if (!isMacPlatform()) return info.combo;
  return info.comboMac
    .replace(/\bCtrl\b/g, '⌃')
    .replace(/\bAlt\b/g, '⌥')
    .replace(/\bShift\b/g, '⇧')
    .replace(/\bWin\b/g, '⌘')
    .replace(/\+/g, ' ');
}
