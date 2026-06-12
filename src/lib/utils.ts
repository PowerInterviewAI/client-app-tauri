import { type ClassValue, clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// Helper to get the Tauri bridge API exposed on window
export const getTauriApi = () => {
  return typeof window !== 'undefined' ? window.tauriApi : undefined;
};

export const getCurrentTimestamp = () => {
  return Date.now();
};

export function isMacPlatform(): boolean {
  try {
    const nav =
      typeof navigator !== 'undefined'
        ? (navigator as Navigator & { userAgent?: string; platform?: string })
        : undefined;
    return !!(
      nav &&
      (/(Mac|iPhone|iPad|iPod)/.test(nav.platform || '') ||
        /Macintosh|Mac OS X/.test(nav.userAgent || ''))
    );
  } catch {
    return false;
  }
}
