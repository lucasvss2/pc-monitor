const time = new Intl.DateTimeFormat("pt-BR", {
  timeZone: "UTC", hourCycle: "h23", hour: "2-digit", minute: "2-digit", second: "2-digit",
});
const date = new Intl.DateTimeFormat("pt-BR", {
  timeZone: "UTC", day: "2-digit", month: "2-digit", year: "numeric",
});

/** The core already applied the offset for this instant. UTC formatting avoids a cached local zone. */
export function formatPanelClock(civilMillis: number) {
  return `${time.format(civilMillis)} · ${date.format(civilMillis)}`;
}
