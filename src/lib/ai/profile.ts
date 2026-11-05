import type { AiData, AiReading, AiWindow } from "../core";

export function restarted(window: AiWindow, now: number) {
  return window.reset || (window.resetsAt !== null && window.resetsAt <= now);
}

/** An absent or restarted window has no known remaining allowance. */
export function remainingPercent(window: AiWindow | null, now: number) {
  if (!window || restarted(window, now) || window.usedPercent === null) return null;
  return Math.max(0, Math.min(100, Math.round(100 - window.usedPercent)));
}

export function installed(provider: AiData["codex"] | AiData["claude"]): boolean {
  if (!provider) return false;
  return provider.installed === undefined ? provider.state !== "missing" : provider.installed === true;
}

/** Find windows by duration, even when a lone weekly window is in the primary slot. */
export function usageWindows(reading: AiReading | null) {
  const bucket = reading?.buckets.find((b) => b.name === null) ?? reading?.buckets[0];
  if (!bucket) return { five: null, week: null };
  const windows = [bucket.primary, bucket.secondary];
  return {
    five: windows.find((window) => window?.durationMins === 300)
      ?? (bucket.primary?.durationMins == null ? bucket.primary : null),
    week: windows.find((window) => window?.durationMins === 10080)
      ?? (bucket.secondary?.durationMins == null ? bucket.secondary : null),
  };
}
