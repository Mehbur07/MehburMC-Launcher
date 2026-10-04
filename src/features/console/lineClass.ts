import type { LogLevel } from "../../lib/ipc/bindings/LogLevel";

/** Tailwind text colour for a log level. */
export function lineClass(level: LogLevel) {
  switch (level) {
    case "fatal":
    case "error":
      return "text-danger";
    case "warn":
      return "text-warn";
    case "debug":
    case "trace":
      return "text-muted";
    default:
      return "text-fg/90";
  }
}
