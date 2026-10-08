import { useApp } from "../stores/app";
import { useLogs } from "../stores/logs";
import { activeTaskFor, useTasks } from "../stores/tasks";
import { ipc, toErrorPayload } from "./ipc";

const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });

export async function play(instanceId: string) {
  try {
    // A second account joining a running instance keeps the first one's log.
    if (!activeTaskFor(useTasks.getState().tasks, instanceId)) useLogs.getState().clear(instanceId);
    await ipc.launchInstance(instanceId);
  } catch (e) {
    notify(e);
  }
}

export async function repair(instanceId: string) {
  try {
    await ipc.repairInstance(instanceId);
    useApp.getState().setView("downloads");
  } catch (e) {
    notify(e);
  }
}

export async function stop(instanceId: string) {
  try {
    await ipc.stopInstance(instanceId);
  } catch (e) {
    notify(e);
  }
}

/** Stops one game (or download) when an instance runs several. */
export async function stopTask(taskId: string) {
  try {
    await ipc.cancelTask(taskId, false);
  } catch (e) {
    notify(e);
  }
}

export async function openFolder(
  instanceId: string,
  folder?: Parameters<typeof ipc.openInstanceFolder>[1],
) {
  try {
    await ipc.openInstanceFolder(instanceId, folder);
  } catch (e) {
    notify(e);
  }
}
