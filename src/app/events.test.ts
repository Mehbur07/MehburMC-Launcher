import { beforeEach, describe, expect, it, vi } from "vitest";

import type { CoreEvent } from "../lib/ipc/bindings/CoreEvent";
import type { GameLogEvent } from "../lib/ipc";

const handlers = vi.hoisted(() => ({
  onEvent: null as null | ((e: CoreEvent) => void),
  onLogs: null as null | ((l: GameLogEvent[]) => void),
}));
const ipcMock = vi.hoisted(() => ({ listTasks: vi.fn(), listInstances: vi.fn() }));
vi.mock("../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../lib/ipc")>()),
  ipc: ipcMock,
  subscribe: vi.fn(async (h: typeof handlers) => {
    handlers.onEvent = h.onEvent;
    handlers.onLogs = h.onLogs;
    return () => {};
  }),
}));

import { useCrash } from "../stores/crash";
import { useInstances } from "../stores/instances";
import { useLogs } from "../stores/logs";
import { useTasks } from "../stores/tasks";
import { instance, task } from "../test/fixtures";
import { connectEvents } from "./events";

const log = (line: string): GameLogEvent => ({
  type: "gameLog",
  task: "task-1",
  stream: "stdout",
  line,
  level: "info",
  timeMs: 1,
  thread: "main",
});

describe("connectEvents", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useTasks.setState({ tasks: {} });
    useLogs.setState({ lines: {}, exits: {} });
    useCrash.setState({ info: null });
    ipcMock.listTasks.mockResolvedValue([task({ id: "old", status: "completed" })]);
    ipcMock.listInstances.mockResolvedValue({ instances: [instance()], selected: "pack-1" });
  });

  it("loads existing tasks and routes events into the stores", async () => {
    await connectEvents();
    expect(Object.keys(useTasks.getState().tasks)).toEqual(["old"]);

    handlers.onEvent!({ type: "task", task: task() });
    expect(useTasks.getState().tasks["task-1"]?.status).toBe("preparing");

    handlers.onLogs!([log("hello"), { ...log("orphan"), task: "unknown" }]);
    expect(useLogs.getState().lines["pack-1"]?.map((l) => l.text)).toEqual(["hello"]);

    handlers.onEvent!({ type: "gameExited", task: "task-1", code: 1, crashReport: "c.txt" });
    expect(useLogs.getState().exits["pack-1"]).toMatchObject({ code: 1, crashReport: "c.txt" });

    handlers.onEvent!({
      type: "gameCrashed",
      task: "task-1",
      info: {
        instanceId: "pack-1",
        instanceName: "My Pack",
        exitCode: 1,
        diagnoses: [{ kind: "outOfMemory" }],
        summary: null,
        crashReport: null,
        hsErr: null,
        logFile: null,
      },
    });
    expect(useCrash.getState().info?.diagnoses[0]?.kind).toBe("outOfMemory");
  });

  it("reloads instances once a session ends (play time)", async () => {
    await connectEvents();
    handlers.onEvent!({ type: "task", task: task({ status: "playing" }) });
    expect(ipcMock.listInstances).not.toHaveBeenCalled();
    handlers.onEvent!({ type: "task", task: task({ status: "completed" }) });
    await vi.waitFor(() => expect(ipcMock.listInstances).toHaveBeenCalledTimes(1));
    expect(useInstances.getState().instances[0]?.id).toBe("pack-1");
  });
});
