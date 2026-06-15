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

/**
 * Extract a human-readable message from an unknown thrown value.
 *
 * Tauri's `invoke()` rejects with the *raw value* a Rust command returns on error, so a
 * command that returns `Err(String)` rejects with a bare string (not an `Error`). Code that
 * only reads `error.message` silently drops those, surfacing a generic fallback instead of
 * the real cause. This handles strings, `Error`s, and `{ message }`-shaped objects.
 */
export function getErrorMessage(error: unknown, fallback: string): string {
  if (typeof error === 'string') return error || fallback;
  if (error instanceof Error) return error.message || fallback;
  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === 'string' && message) return message;
  }
  return fallback;
}

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
