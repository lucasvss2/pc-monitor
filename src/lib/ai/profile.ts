import type { AiData, AiReading } from "../core";

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
