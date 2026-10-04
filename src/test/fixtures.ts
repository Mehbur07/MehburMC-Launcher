import type { Instance } from "../lib/ipc/bindings/Instance";
import type { TaskInfo } from "../lib/ipc/bindings/TaskInfo";

/** Minimal valid objects for tests; override what matters. */
export function instance(over: Partial<Instance> = {}): Instance {
  return {
    schemaVersion: 1,
    id: "pack-1",
    name: "My Pack",
    icon: "grass",
    mcVersion: "1.20.1",
    loader: { kind: "fabric", version: "0.16.0" },
    javaPath: null,
    memoryMb: null,
    jvmArgs: "",
    resolution: null,
    fullscreen: false,
    createdAt: 1,
    lastPlayed: null,
    playTimeSecs: 0,
    ...over,
  };
}

export function task(over: Partial<TaskInfo> = {}): TaskInfo {
  return {
    id: "task-1",
    kind: "launch",
    instanceId: "pack-1",
    title: "My Pack",
    status: "preparing",
    stage: null,
    done: 0,
    total: 0,
    bytesDone: 0,
    bytesTotal: 0,
    error: null,
    startedAt: 1,
    finishedAt: null,
    ...over,
  };
}
