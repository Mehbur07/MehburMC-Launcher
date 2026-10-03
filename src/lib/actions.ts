import { useApp } from "../stores/app";
import { useLogs } from "../stores/logs";
import { ipc, toErrorPayload } from "./ipc";

const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });

export async function play(instanceId: string) {
  try {
    useLogs.getState().clear(instanceId);
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
