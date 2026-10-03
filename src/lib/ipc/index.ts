import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { Account } from "./bindings/Account";
import type { AccountsView } from "./bindings/AccountsView";
import type { Bootstrap } from "./bindings/Bootstrap";
import type { CoreEvent } from "./bindings/CoreEvent";
import type { ErrorPayload } from "./bindings/ErrorPayload";
import type { FileEntry } from "./bindings/FileEntry";
import type { Folder } from "./bindings/Folder";
import type { Instance } from "./bindings/Instance";
import type { InstancePatch } from "./bindings/InstancePatch";
import type { InstancesView } from "./bindings/InstancesView";
import type { JavaInstall } from "./bindings/JavaInstall";
import type { LoaderKind } from "./bindings/LoaderKind";
import type { LoaderVersion } from "./bindings/LoaderVersion";
import type { ManifestEntry } from "./bindings/ManifestEntry";
import type { NewInstance } from "./bindings/NewInstance";
import type { OptifineInfo } from "./bindings/OptifineInfo";
import type { Settings } from "./bindings/Settings";
import type { ShaderSetup } from "./bindings/ShaderSetup";
import type { TaskInfo } from "./bindings/TaskInfo";

/** Typed wrappers around the Rust commands (see src-tauri/src/commands). */
export const ipc = {
  getBootstrap: () => invoke<Bootstrap>("get_bootstrap"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  openDataDir: () => invoke<void>("open_data_dir"),
  listVersions: () => invoke<ManifestEntry[]>("list_versions"),
  listJava: () => invoke<JavaInstall[]>("list_java"),
  saveTextFile: (path: string, contents: string) =>
    invoke<void>("save_text_file", { path, contents }),

  listInstances: () => invoke<InstancesView>("list_instances"),
  createInstance: (req: NewInstance) => invoke<Instance>("create_instance", { req }),
  updateInstance: (id: string, patch: InstancePatch) =>
    invoke<Instance>("update_instance", { id, patch }),
  deleteInstance: (id: string) => invoke<InstancesView>("delete_instance", { id }),
  copyInstance: (id: string, name: string) => invoke<Instance>("copy_instance", { id, name }),
  reorderInstances: (ids: string[]) => invoke<void>("reorder_instances", { ids }),
  selectInstance: (id: string) => invoke<void>("select_instance", { id }),
  exportInstance: (id: string, dest: string) => invoke<void>("export_instance", { id, dest }),
  importInstance: (src: string) => invoke<Instance>("import_instance", { src }),
  openInstanceFolder: (id: string, folder?: Folder) =>
    invoke<void>("open_instance_folder", { id, folder }),

  launchInstance: (id: string) => invoke<string>("launch_instance", { id }),
  repairInstance: (id: string) => invoke<string>("repair_instance", { id }),
  stopInstance: (id: string) => invoke<boolean>("stop_instance", { id }),
  listTasks: () => invoke<TaskInfo[]>("list_tasks"),
  cancelTask: (id: string, pause: boolean) => invoke<boolean>("cancel_task", { id, pause }),
  clearTasks: () => invoke<TaskInfo[]>("clear_tasks"),

  listAccounts: () => invoke<AccountsView>("list_accounts"),
  addOfflineAccount: (name: string) => invoke<Account>("add_offline_account", { name }),
  removeAccount: (id: string) => invoke<AccountsView>("remove_account", { id }),
  selectAccount: (id: string) => invoke<AccountsView>("select_account", { id }),

  listInstanceFiles: (id: string, folder: Folder) =>
    invoke<FileEntry[]>("list_instance_files", { id, folder }),
  instanceFolderPath: (id: string, folder: Folder) =>
    invoke<string>("instance_folder_path", { id, folder }),
  toggleInstanceFile: (id: string, folder: Folder, name: string) =>
    invoke<FileEntry>("toggle_instance_file", { id, folder, name }),
  deleteInstanceFile: (id: string, folder: Folder, name: string) =>
    invoke<void>("delete_instance_file", { id, folder, name }),
  readInstanceLog: (id: string, folder: Folder, name: string) =>
    invoke<string>("read_instance_log", { id, folder, name }),

  listLoaderVersions: (kind: LoaderKind, mc: string) =>
    invoke<LoaderVersion[]>("list_loader_versions", { kind, mc }),
  importOptifine: (path: string) => invoke<OptifineInfo>("import_optifine", { path }),
  installShaderSupport: (id: string) => invoke<ShaderSetup>("install_shader_support", { id }),
};

export type GameLogEvent = Extract<CoreEvent, { type: "gameLog" }>;

/** Subscribes to backend events (see src-tauri/src/bridge.rs). */
export async function subscribe(handlers: {
  onEvent: (e: CoreEvent) => void;
  onLogs: (lines: GameLogEvent[]) => void;
}): Promise<UnlistenFn> {
  const a = await listen<CoreEvent>("core://event", (e) => handlers.onEvent(e.payload));
  const b = await listen<GameLogEvent[]>("core://logs", (e) => handlers.onLogs(e.payload));
  return () => {
    a();
    b();
  };
}

export function isErrorPayload(e: unknown): e is ErrorPayload {
  return typeof e === "object" && e !== null && "code" in e && "detail" in e;
}

/** Normalizes anything thrown by `invoke` into an ErrorPayload. */
export function toErrorPayload(e: unknown): ErrorPayload {
  if (isErrorPayload(e)) return e;
  return { code: "unknown", params: {}, detail: e instanceof Error ? e.message : String(e) };
}
