import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { Account } from "./bindings/Account";
import type { AppUpdate } from "./bindings/AppUpdate";
import type { Assignment } from "./bindings/Assignment";
import type { AccountsView } from "./bindings/AccountsView";
import type { Bootstrap } from "./bindings/Bootstrap";
import type { CoreEvent } from "./bindings/CoreEvent";
import type { CrashInfo } from "./bindings/CrashInfo";
import type { ErrorPayload } from "./bindings/ErrorPayload";
import type { FileEntry } from "./bindings/FileEntry";
import type { ImportResult } from "./bindings/ImportResult";
import type { InstallRequest } from "./bindings/InstallRequest";
import type { InstallResult } from "./bindings/InstallResult";
import type { InstalledItem } from "./bindings/InstalledItem";
import type { Folder } from "./bindings/Folder";
import type { Instance } from "./bindings/Instance";
import type { InstancePatch } from "./bindings/InstancePatch";
import type { InstancesView } from "./bindings/InstancesView";
import type { JavaInstall } from "./bindings/JavaInstall";
import type { LibraryView } from "./bindings/LibraryView";
import type { LoaderKind } from "./bindings/LoaderKind";
import type { LoaderVersion } from "./bindings/LoaderVersion";
import type { DefaultSkin } from "./bindings/DefaultSkin";
import type { ManifestEntry } from "./bindings/ManifestEntry";
import type { MoveProgress } from "./bindings/MoveProgress";
import type { NewInstance } from "./bindings/NewInstance";
import type { NewsItem } from "./bindings/NewsItem";
import type { OptifineInfo } from "./bindings/OptifineInfo";
import type { PickPurpose } from "./bindings/PickPurpose";
import type { ProjectType } from "./bindings/ProjectType";
import type { SearchPage } from "./bindings/SearchPage";
import type { SearchQuery } from "./bindings/SearchQuery";
import type { SkinModel } from "./bindings/SkinModel";
import type { VersionSummary } from "./bindings/VersionSummary";
import type { Settings } from "./bindings/Settings";
import type { TaskInfo } from "./bindings/TaskInfo";
import type { TextureKind } from "./bindings/TextureKind";

/** Typed wrappers around the Rust commands (see src-tauri/src/commands). */
export const ipc = {
  getBootstrap: () => invoke<Bootstrap>("get_bootstrap"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  openDataDir: () => invoke<void>("open_data_dir"),
  listVersions: () => invoke<ManifestEntry[]>("list_versions"),
  listJava: () => invoke<JavaInstall[]>("list_java"),
  /** Writes to the path chosen with `pickPath("consoleLog")`. */
  saveTextFile: (contents: string) => invoke<void>("save_text_file", { contents }),

  listInstances: () => invoke<InstancesView>("list_instances"),
  createInstance: (req: NewInstance) => invoke<Instance>("create_instance", { req }),
  updateInstance: (id: string, patch: InstancePatch) =>
    invoke<Instance>("update_instance", { id, patch }),
  deleteInstance: (id: string) => invoke<InstancesView>("delete_instance", { id }),
  copyInstance: (id: string, name: string) => invoke<Instance>("copy_instance", { id, name }),
  reorderInstances: (ids: string[]) => invoke<void>("reorder_instances", { ids }),
  selectInstance: (id: string) => invoke<void>("select_instance", { id }),
  exportInstance: (id: string) => invoke<void>("export_instance", { id }),
  importInstance: () => invoke<Instance>("import_instance"),
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
  importOptifine: () => invoke<OptifineInfo>("import_optifine"),
  installShaderSupport: (id: string) => invoke<InstallResult>("install_shader_support", { id }),

  searchModrinth: (query: SearchQuery) => invoke<SearchPage>("search_modrinth", { query }),
  projectVersions: (project: string, projectType: ProjectType, instanceId?: string) =>
    invoke<VersionSummary[]>("project_versions", { project, projectType, instanceId }),
  installContent: (id: string, requests: InstallRequest[]) =>
    invoke<InstallResult>("install_content", { id, requests }),
  scanContent: (id: string, folder: Folder, checkUpdates: boolean) =>
    invoke<InstalledItem[]>("scan_content", { id, folder, checkUpdates }),
  updateContent: (id: string, folder: Folder, files: string[]) =>
    invoke<string[]>("update_content", { id, folder, files }),
  importModpack: () => invoke<ImportResult>("import_modpack"),
  installModrinthModpack: (versionId: string) =>
    invoke<ImportResult>("install_modrinth_modpack", { versionId }),
  contentIcon: (url: string) => invoke<string>("content_icon", { url }),
  openExternal: (url: string) => invoke<void>("open_external", { url }),

  listSkins: () => invoke<LibraryView>("list_skins"),
  importSkinFile: (kind: TextureKind, model?: SkinModel) =>
    invoke<string>("import_skin_file", { kind, model }),
  addSkinBytes: (kind: TextureKind, name: string, pngBase64: string, model?: SkinModel) =>
    invoke<string>("add_skin_bytes", { kind, name, model, pngBase64 }),
  listDefaultSkins: () => invoke<DefaultSkin[]>("list_default_skins"),
  updateSkin: (kind: TextureKind, id: string, name?: string, model?: SkinModel) =>
    invoke<void>("update_skin", { kind, id, name, model }),
  deleteSkin: (kind: TextureKind, id: string) => invoke<void>("delete_skin", { kind, id }),
  assignSkin: (accountId: string, kind: TextureKind, id: string | null) =>
    invoke<Record<string, Assignment>>("assign_skin", { accountId, kind, id }),
  exportSkin: (id: string) => invoke<void>("export_skin", { id }),

  listNews: () => invoke<NewsItem[]>("list_news"),
  analyzeCrashReport: (id: string, name: string) =>
    invoke<CrashInfo>("analyze_crash_report", { id, name }),
  openDataFile: (path: string) => invoke<void>("open_data_file", { path }),
  moveDataFolder: () => invoke<number>("move_data_folder"),
  defaultDataFolder: () => invoke<string>("default_data_folder"),
  restartApp: () => invoke<void>("restart_app"),
  checkAppUpdate: () => invoke<AppUpdate>("check_app_update"),
  installAppUpdate: () => invoke<void>("install_app_update"),

  /**
   * Opens a native file dialog in Rust. Returns the chosen path for display
   * (null = cancelled); the command that needs it reads it on the Rust side.
   */
  pickPath: (purpose: PickPurpose, defaultName?: string, title?: string) =>
    invoke<string | null>("pick_path", { purpose, defaultName, title }),
};

/** Progress of "Move data folder". */
export const onDataMove = (f: (p: MoveProgress) => void) =>
  listen<MoveProgress>("core://datamove", (e) => f(e.payload));

/** Update download progress as `[doneBytes, totalBytes]`. */
export const onUpdateProgress = (f: (p: [number, number]) => void) =>
  listen<[number, number]>("core://update", (e) => f(e.payload));

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
