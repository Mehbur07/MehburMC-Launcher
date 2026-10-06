/** Human-readable byte size (1 decimal above KB). */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const v = bytes / 1024 ** i;
  return `${i === 0 ? v.toFixed(0) : v.toFixed(1)} ${units[i]}`;
}

/** `2 sa 5 dk` / `2h 5m` style duration; `t` supplies the unit suffixes. */
export function formatDuration(
  secs: number,
  units: { h: string; m: string; s: string } = { h: "h", m: "m", s: "s" },
): string {
  const s = Math.max(0, Math.round(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h > 0) return `${h} ${units.h} ${m} ${units.m}`;
  if (m > 0) return `${m} ${units.m}`;
  return `${s} ${units.s}`;
}

/** Locale date for unix seconds. */
export function formatDate(unixSecs: number, locale: string): string {
  return new Date(unixSecs * 1000).toLocaleString(locale, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

/** An RFC 3339 timestamp from the server, in the user's locale. */
export function formatIso(iso: string, locale: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(locale, { dateStyle: "medium", timeStyle: "short" });
}

export function formatTime(ms: number, locale: string): string {
  return new Date(ms).toLocaleTimeString(locale, { hour12: false });
}
