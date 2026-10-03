import { ipc, subscribe, type GameLogEvent } from "../lib/ipc";
import { useInstances } from "../stores/instances";
import { useLogs } from "../stores/logs";
import { useTasks } from "../stores/tasks";

const instanceOf = (taskId: string) => useTasks.getState().tasks[taskId]?.instanceId ?? null;

/** Wires backend events into the stores. Returns an unsubscribe function. */
export async function connectEvents(): Promise<() => void> {
  const unlisten = await subscribe({
    onEvent: (e) => {
      switch (e.type) {
        case "task": {
          const prev = useTasks.getState().tasks[e.task.id];
          useTasks.getState().applyTask(e.task);
          const finished = e.task.status !== "preparing" && e.task.status !== "playing";
          // Refresh play time / last played once a session ends.
          if (finished && prev && prev.status !== e.task.status)
            void useInstances.getState().load();
          break;
        }
        case "progress":
          useTasks.getState().applyProgress(e);
          break;
        case "gameExited": {
          const id = instanceOf(e.task);
          if (id) {
            useLogs
              .getState()
              .setExit(id, { code: e.code, crashReport: e.crashReport, at: Date.now() });
          }
          break;
        }
        case "gameLog":
          break; // logs arrive batched on their own channel
      }
    },
    onLogs: (lines) => {
      const byInstance = new Map<string, GameLogEvent[]>();
      for (const l of lines) {
        const id = instanceOf(l.task);
        if (!id) continue;
        const list = byInstance.get(id) ?? [];
        list.push(l);
        byInstance.set(id, list);
      }
      for (const [id, list] of byInstance) useLogs.getState().append(id, list);
    },
  });
  // Tasks that started before this window loaded (e.g. after a reload).
  try {
    useTasks.getState().setAll(await ipc.listTasks());
  } catch {
    // Non-fatal: the Downloads screen will simply start empty.
  }
  return unlisten;
}
