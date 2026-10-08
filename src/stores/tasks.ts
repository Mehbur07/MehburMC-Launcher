import { create } from "zustand";

import type { CoreEvent } from "../lib/ipc/bindings/CoreEvent";
import type { TaskInfo } from "../lib/ipc/bindings/TaskInfo";

type ProgressEvent = Extract<CoreEvent, { type: "progress" }>;

interface Speed {
  /** Smoothed bytes per second. */
  bps: number;
  lastBytes: number;
  lastAt: number;
}

interface TasksStore {
  tasks: Record<string, TaskInfo>;
  speed: Record<string, Speed>;
  setAll: (list: TaskInfo[]) => void;
  applyTask: (t: TaskInfo) => void;
  applyProgress: (e: ProgressEvent, now?: number) => void;
}

const SMOOTHING = 0.3;

export const useTasks = create<TasksStore>((set) => ({
  tasks: {},
  speed: {},

  setAll: (list) => set({ tasks: Object.fromEntries(list.map((t) => [t.id, t])) }),

  applyTask: (t) => set((s) => ({ tasks: { ...s.tasks, [t.id]: t } })),

  applyProgress: (e, now = Date.now()) =>
    set((s) => {
      const prev = s.tasks[e.task];
      if (!prev) return s;
      const task: TaskInfo = {
        ...prev,
        stage: e.stage,
        done: e.done,
        total: e.total,
        bytesDone: e.bytesDone,
        bytesTotal: e.bytesTotal,
      };
      const old = s.speed[e.task];
      let speed: Speed = { bps: old?.bps ?? 0, lastBytes: e.bytesDone, lastAt: now };
      if (old && now > old.lastAt && e.bytesDone >= old.lastBytes) {
        const inst = ((e.bytesDone - old.lastBytes) * 1000) / (now - old.lastAt);
        speed = {
          ...speed,
          bps: old.bps === 0 ? inst : old.bps * (1 - SMOOTHING) + inst * SMOOTHING,
        };
      }
      return { tasks: { ...s.tasks, [e.task]: task }, speed: { ...s.speed, [e.task]: speed } };
    }),
}));

export const isActive = (t: TaskInfo) => t.status === "preparing" || t.status === "playing";

/** Every active task of an instance (several accounts may play it at once). */
export function activeTasksFor(tasks: Record<string, TaskInfo>, instanceId: string | null) {
  if (!instanceId) return [];
  return Object.values(tasks)
    .filter((t) => t.instanceId === instanceId && isActive(t))
    .sort((a, b) => a.startedAt - b.startedAt);
}

/**
 * Whether `account` cannot start the instance now: a repair/install owns
 * it, or that account already plays it (other accounts may join, K76).
 */
export function playBlocked(
  tasks: Record<string, TaskInfo>,
  instanceId: string | null,
  account: string | null,
) {
  return activeTasksFor(tasks, instanceId).some(
    (t) => t.kind !== "launch" || (account !== null && t.account === account),
  );
}

/** Active task for an instance, if any. */
export function activeTaskFor(tasks: Record<string, TaskInfo>, instanceId: string | null) {
  if (!instanceId) return null;
  return Object.values(tasks).find((t) => t.instanceId === instanceId && isActive(t)) ?? null;
}

/** 0–1 progress of a task's current stage (bytes when known, else files). */
export function fraction(t: TaskInfo): number {
  if (t.bytesTotal > 0) return Math.min(1, t.bytesDone / t.bytesTotal);
  if (t.total > 0) return Math.min(1, t.done / t.total);
  return 0;
}

/** Seconds left at the current speed, or null if unknown. */
export function eta(t: TaskInfo, bps: number | undefined): number | null {
  if (!bps || bps < 1 || t.bytesTotal <= 0) return null;
  return Math.max(0, (t.bytesTotal - t.bytesDone) / bps);
}
