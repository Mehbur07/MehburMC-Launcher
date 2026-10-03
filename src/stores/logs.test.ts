import { beforeEach, describe, expect, it } from "vitest";

import type { GameLogEvent } from "../lib/ipc";
import { filterLines } from "../features/console/ConsolePage";
import { MAX_LINES, severity, useLogs } from "./logs";

const ev = (line: string, over: Partial<GameLogEvent> = {}): GameLogEvent => ({
  type: "gameLog",
  task: "t",
  stream: "stdout",
  line,
  level: null,
  timeMs: null,
  thread: null,
  ...over,
});

describe("logs store", () => {
  beforeEach(() => useLogs.setState({ lines: {}, exits: {} }));

  it("keeps a bounded ring buffer per instance", () => {
    const batch = Array.from({ length: MAX_LINES + 10 }, (_, i) => ev(`line ${i}`));
    useLogs.getState().append("a", batch);
    const lines = useLogs.getState().lines["a"]!;
    expect(lines).toHaveLength(MAX_LINES);
    expect(lines[0]!.text).toBe("line 10");
    expect(useLogs.getState().lines["b"]).toBeUndefined();
  });

  it("derives severity", () => {
    useLogs
      .getState()
      .append("a", [
        ev("ok", { level: "info" }),
        ev("plain stderr", { stream: "stderr" }),
        ev("java.lang.IllegalStateException: boom"),
        ev("bad", { level: "error" }),
      ]);
    const s = useLogs.getState().lines["a"]!.map(severity);
    expect(s).toEqual(["info", "warn", "error", "error"]);
  });

  it("filters by level and text", () => {
    useLogs
      .getState()
      .append("a", [
        ev("Sound engine started", { level: "info" }),
        ev("Missing texture", { level: "warn" }),
        ev("Crash!", { level: "fatal" }),
      ]);
    const all = useLogs.getState().lines["a"]!;
    expect(filterLines(all, "all", "")).toHaveLength(3);
    expect(filterLines(all, "warn", "").map((l) => l.text)).toEqual(["Missing texture", "Crash!"]);
    expect(filterLines(all, "error", "")).toHaveLength(1);
    expect(filterLines(all, "all", "sound")).toHaveLength(1);
  });

  it("clear removes lines and exit info", () => {
    useLogs.getState().append("a", [ev("x")]);
    useLogs.getState().setExit("a", { code: 1, crashReport: null, at: 0 });
    useLogs.getState().clear("a");
    expect(useLogs.getState().lines["a"]).toBeUndefined();
    expect(useLogs.getState().exits["a"]).toBeUndefined();
  });
});
