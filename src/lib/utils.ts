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
