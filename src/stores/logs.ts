import { create } from "zustand";

import type { GameLogEvent } from "../lib/ipc";
import type { LogLevel } from "../lib/ipc/bindings/LogLevel";
import type { LogStream } from "../lib/ipc/bindings/LogStream";

/** Lines kept per instance (ring buffer). */
export const MAX_LINES = 5000;

export interface LogLine {
  seq: number;
  text: string;
  level: LogLevel | null;
  stream: LogStream;
  timeMs: number;
  thread: string | null;
}

export interface ExitInfo {
  code: number | null;
  crashReport: string | null;
  at: number;
}

interface LogsStore {
  lines: Record<string, LogLine[]>;
  exits: Record<string, ExitInfo>;
  append: (instanceId: string, events: GameLogEvent[], now?: number) => void;
  setExit: (instanceId: string, exit: ExitInfo) => void;
  clear: (instanceId: string) => void;
}

let seq = 0;

export const useLogs = create<LogsStore>((set) => ({
  lines: {},
  exits: {},

  append: (instanceId, events, now = Date.now()) =>
    set((s) => {
      const added = events.map<LogLine>((e) => ({
        seq: ++seq,
        text: e.line,
        level: e.level,
        stream: e.stream,
        timeMs: e.timeMs ?? now,
        thread: e.thread,
      }));
      const merged = (s.lines[instanceId] ?? []).concat(added);
      const trimmed = merged.length > MAX_LINES ? merged.slice(merged.length - MAX_LINES) : merged;
      return { lines: { ...s.lines, [instanceId]: trimmed } };
    }),

  setExit: (instanceId, exit) => set((s) => ({ exits: { ...s.exits, [instanceId]: exit } })),

  clear: (instanceId) =>
    set((s) => {
      const lines = { ...s.lines };
      const exits = { ...s.exits };
      delete lines[instanceId];
      delete exits[instanceId];
      return { lines, exits };
    }),
}));

/** Effective severity for colouring (stderr without a level counts as warn). */
export function severity(l: LogLine): LogLevel {
  if (l.level) return l.level;
  // `java.lang.IllegalStateException: …`, stack frames, causes.
  if (/(Exception|Error)(:|\s|$)|^\s+at\s|Caused by:/.test(l.text)) return "error";
  return l.stream === "stderr" ? "warn" : "info";
}
