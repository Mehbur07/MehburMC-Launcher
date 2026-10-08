import { beforeEach, describe, expect, it } from "vitest";

import type { TaskInfo } from "../lib/ipc/bindings/TaskInfo";
import { activeTaskFor, eta, fraction, useTasks } from "./tasks";

const task = (over: Partial<TaskInfo> = {}): TaskInfo => ({
  id: "task-1",
  kind: "launch",
  instanceId: "a",
  title: "A",
  account: null,
  status: "preparing",
  stage: null,
  done: 0,
  total: 0,
  bytesDone: 0,
  bytesTotal: 0,
  error: null,
  startedAt: 0,
  finishedAt: null,
  ...over,
});

describe("tasks store", () => {
  beforeEach(() => useTasks.setState({ tasks: {}, speed: {} }));

  it("applies progress and smooths speed", () => {
    useTasks.getState().applyTask(task());
    const p = (bytesDone: number, now: number) =>
      useTasks.getState().applyProgress(
        {
          type: "progress",
          task: "task-1",
          stage: "assets",
          done: 1,
          total: 10,
          bytesDone,
          bytesTotal: 10_000_000,
        },
        now,
      );
    p(0, 1000);
    p(1_000_000, 2000); // 1 MB/s
    const t = useTasks.getState().tasks["task-1"]!;
    expect(t.stage).toBe("assets");
    expect(fraction(t)).toBeCloseTo(0.1);
    const bps = useTasks.getState().speed["task-1"]!.bps;
    expect(bps).toBeCloseTo(1_000_000);
    expect(eta(t, bps)).toBeCloseTo(9);
  });

  it("ignores progress for unknown tasks", () => {
    useTasks.getState().applyProgress({
      type: "progress",
      task: "nope",
      stage: "assets",
      done: 1,
      total: 1,
      bytesDone: 1,
      bytesTotal: 1,
    });
    expect(useTasks.getState().tasks).toEqual({});
  });

  it("finds the active task of an instance", () => {
    const tasks = {
      a: task({ id: "a", status: "completed" }),
      b: task({ id: "b", status: "playing" }),
    };
    expect(activeTaskFor(tasks, "a")?.id).toBe("b");
    expect(activeTaskFor(tasks, "other")).toBeNull();
  });

  it("fraction falls back to file counts", () => {
    expect(fraction(task({ done: 3, total: 4 }))).toBe(0.75);
    expect(fraction(task())).toBe(0);
  });
});
