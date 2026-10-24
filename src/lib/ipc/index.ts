import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { Account } from "./bindings/Account";
import type { AppUpdate } from "./bindings/AppUpdate";
import type { AdminBan } from "./bindings/AdminBan";
import type { AdminEntry } from "./bindings/AdminEntry";
import type { AdminMod } from "./bindings/AdminMod";
import type { AdminModStatus } from "./bindings/AdminModStatus";
import type { AdminReport } from "./bindings/AdminReport";
import type { AdminTexture } from "./bindings/AdminTexture";
import type { AdminPlayerPage } from "./bindings/AdminPlayerPage";
import type { AdminUser } from "./bindings/AdminUser";
import type { PrivateTexture } from "./bindings/PrivateTexture";
import type { TextureGrant } from "./bindings/TextureGrant";
import type { AuthStatus } from "./bindings/AuthStatus";
import type { CodePurpose } from "./bindings/CodePurpose";
import type { ReportKind } from "./bindings/ReportKind";
import type { ScanReport } from "./bindings/ScanReport";
import type { SignUpResult } from "./bindings/SignUpResult";
import type { Assignment } from "./bindings/Assignment";
import type { AccountFaces } from "./bindings/AccountFaces";
import type { AccountsView } from "./bindings/AccountsView";
import type { Bootstrap } from "./bindings/Bootstrap";
import type { CoreEvent } from "./bindings/CoreEvent";
import type { CallPoll } from "./bindings/CallPoll";
import type { IceConfig } from "./bindings/IceConfig";
import type { OutgoingSignal } from "./bindings/OutgoingSignal";
import type { ChatGroup } from "./bindings/ChatGroup";
import type { ChatMessage } from "./bindings/ChatMessage";
import type { GroupMessage } from "./bindings/GroupMessage";
import type { CrashInfo } from "./bindings/CrashInfo";
import type { ErrorPayload } from "./bindings/ErrorPayload";
import type { FileEntry } from "./bindings/FileEntry";
import type { Friend } from "./bindings/Friend";
import type { FriendInstallResult } from "./bindings/FriendInstallResult";
import type { FriendsStatus } from "./bindings/FriendsStatus";
import type { FavoriteServer } from "./bindings/FavoriteServer";
import type { GameServer } from "./bindings/GameServer";
import type { ServerStatus } from "./bindings/ServerStatus";
import type { ReportReason } from "./bindings/ReportReason";
import type { SharedTexture } from "./bindings/SharedTexture";
import type { Visibility } from "./bindings/Visibility";
import type { Profile } from "./bindings/Profile";
import type { SharedList } from "./bindings/SharedList";
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
import type { LibraryInstall } from "./bindings/LibraryInstall";
import type { LibraryKind } from "./bindings/LibraryKind";
import type { LibraryMod } from "./bindings/LibraryMod";
import type { LibraryReportReason } from "./bindings/LibraryReportReason";
import type { LibraryScan } from "./bindings/LibraryScan";
import type { LoaderKind } from "./bindings/LoaderKind";
import type { LoaderVersion } from "./bindings/LoaderVersion";
import type { DefaultSkin } from "./bindings/DefaultSkin";
import type { ManifestEntry } from "./bindings/ManifestEntry";
import type { MoveProgress } from "./bindings/MoveProgress";
import type { NewInstance } from "./bindings/NewInstance";
import type { NewsItem } from "./bindings/NewsItem";
import type { SkinMcPage } from "./bindings/SkinMcPage";
import type { SkinMcSort } from "./bindings/SkinMcSort";
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
  /** Starts the instance and joins `address` from the main menu. */
  joinServer: (id: string, address: string) => invoke<string>("join_server", { id, address }),
  listTasks: () => invoke<TaskInfo[]>("list_tasks"),
  cancelTask: (id: string, pause: boolean) => invoke<boolean>("cancel_task", { id, pause }),
  clearTasks: () => invoke<TaskInfo[]>("clear_tasks"),

  listAccounts: () => invoke<AccountsView>("list_accounts"),
  addOfflineAccount: (name: string) => invoke<Account>("add_offline_account", { name }),
  removeAccount: (id: string) => invoke<AccountsView>("remove_account", { id }),
  renameAccount: (id: string, name: string) => invoke<Account>("rename_account", { id, name }),
  selectAccount: (id: string) => invoke<AccountsView>("select_account", { id }),
  /** Profile photos and default-skin textures of the accounts. */
  accountAvatars: () => invoke<AccountFaces>("account_avatars"),
  /** Sets the photo chosen with `pickPath("avatar")`; returns it as a URI. */
  setAccountAvatar: (id: string) => invoke<string>("set_account_avatar", { id }),
  clearAccountAvatar: (id: string) => invoke<void>("clear_account_avatar", { id }),

  listInstanceFiles: (id: string, folder: Folder) =>
    invoke<FileEntry[]>("list_instance_files", { id, folder }),
  instanceFolderPath: (id: string, folder: Folder) =>
    invoke<string>("instance_folder_path", { id, folder }),
  /** Turns several content files on/off; returns how many were switched. */
  setContentEnabled: (id: string, folder: Folder, names: string[], enabled: boolean) =>
    invoke<number>("set_content_enabled", { id, folder, names, enabled }),
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

  listFavoriteServers: () => invoke<FavoriteServer[]>("list_favorite_servers"),
  addFavoriteServer: (name: string, address: string) =>
    invoke<FavoriteServer[]>("add_favorite_server", { name, address }),
  removeFavoriteServer: (id: string) => invoke<FavoriteServer[]>("remove_favorite_server", { id }),
  reorderFavoriteServers: (ids: string[]) =>
    invoke<FavoriteServer[]>("reorder_favorite_servers", { ids }),
  listGameServers: (instanceId: string) =>
    invoke<GameServer[]>("list_game_servers", { instanceId }),
  addGameServer: (instanceId: string, name: string, address: string) =>
    invoke<GameServer[]>("add_game_server", { instanceId, name, address }),
  removeGameServer: (instanceId: string, index: number, address: string) =>
    invoke<GameServer[]>("remove_game_server", { instanceId, index, address }),
  pingServer: (address: string) => invoke<ServerStatus>("ping_server", { address }),

  /** Shared skins/capes the caller can see (own first). */
  communityTextures: () => invoke<SharedTexture[]>("community_textures"),
  /** Shares a library texture; returns the share id. */
  shareTexture: (kind: TextureKind, id: string, name: string, visibility: Visibility) =>
    invoke<number>("share_texture", { kind, id, name, visibility }),
  unshareTexture: (shareId: number) => invoke<void>("unshare_texture", { shareId }),
  reportTexture: (shareId: number, reason: ReportReason, note: string) =>
    invoke<void>("report_texture", { shareId, reason, note }),

  /** MehburMC account; `check` asks the server for rank and ban. */
  authStatus: (check: boolean) => invoke<AuthStatus>("auth_status", { check }),
  authSignUp: (email: string, password: string) =>
    invoke<SignUpResult>("auth_sign_up", { email, password }),
  /** Confirms the emailed code; `password` is set for upgrades and resets. */
  authVerify: (email: string, code: string, purpose: CodePurpose, password: string) =>
    invoke<void>("auth_verify", { email, code, purpose, password }),
  authResend: (email: string, purpose: CodePurpose) =>
    invoke<void>("auth_resend", { email, purpose }),
  authRequestReset: (email: string) => invoke<void>("auth_request_reset", { email }),
  authSignIn: (email: string, password: string) =>
    invoke<void>("auth_sign_in", { email, password }),
  authSignOut: () => invoke<void>("auth_sign_out"),
  authCreateLocal: () => invoke<void>("auth_create_local"),
  /** Resolves when the browser sign-in finishes (or is cancelled). */
  authGoogle: () => invoke<void>("auth_google"),
  authGoogleCancel: () => invoke<void>("auth_google_cancel"),

  adminLibrary: (status: AdminModStatus) => invoke<AdminMod[]>("admin_library", { status }),
  /** Downloads a library mod and scans it in this launcher. */
  adminScanMod: (id: number, status: AdminModStatus) =>
    invoke<ScanReport>("admin_scan_mod", { id, status }),
  adminReviewMod: (id: number, status: AdminModStatus, approve: boolean, note: string) =>
    invoke<void>("admin_review_mod", { id, status, approve, note }),
  adminRemoveMod: (id: number, note: string) => invoke<void>("admin_remove_mod", { id, note }),
  adminReports: () => invoke<AdminReport[]>("admin_reports"),
  adminDismissReports: (kind: ReportKind, id: number) =>
    invoke<void>("admin_dismiss_reports", { kind, id }),
  adminHideTexture: (id: number) => invoke<void>("admin_hide_texture", { id }),
  /** Founder: every community share; `hidden` lists the removed ones. */
  adminTextures: (hidden: boolean) => invoke<AdminTexture[]>("admin_textures", { hidden }),
  adminRestoreTexture: (id: number) => invoke<void>("admin_restore_texture", { id }),
  adminFindUsers: (query: string) => invoke<AdminUser[]>("admin_find_users", { query }),
  adminAllUsers: (query: string, offset: number) =>
    invoke<AdminPlayerPage>("admin_all_users", { query, offset }),
  /** `hours` null = permanent. */
  adminBan: (userId: string, hours: number | null, reason: string) =>
    invoke<void>("admin_ban", { userId, hours, reason }),
  adminUnban: (userId: string) => invoke<void>("admin_unban", { userId }),
  adminBans: () => invoke<AdminBan[]>("admin_bans"),
  adminList: () => invoke<AdminEntry[]>("admin_list"),
  adminSetRank: (userId: string, admin: boolean) =>
    invoke<void>("admin_set_rank", { userId, admin }),
  /** Founder: private MehburMC textures (K74). */
  adminPrivateTextures: () => invoke<PrivateTexture[]>("admin_private_textures"),
  /** Uploads the PNG chosen with `pickPath("privateTexture")`. */
  adminUploadPrivateTexture: (kind: TextureKind, model: SkinModel, name: string) =>
    invoke<number>("admin_upload_private_texture", { kind, model, name }),
  adminDeletePrivateTexture: (id: number) => invoke<void>("admin_delete_private_texture", { id }),
  adminTextureGrants: (id: number) => invoke<TextureGrant[]>("admin_texture_grants", { id }),
  adminSetTextureGrant: (id: number, userId: string, grant: boolean) =>
    invoke<void>("admin_set_texture_grant", { id, userId, grant }),
  /** Applies this account's private texture grants; true if the library changed. */
  syncPrivateTextures: () => invoke<boolean>("sync_private_textures"),

  /** Scans the file chosen with the kind's `pickPath` purpose (K88). */
  libraryScanPick: (kind: LibraryKind) => invoke<LibraryScan>("library_scan_pick", { kind }),
  /** Packs one of the user's instances as a modpack and scans it (K90). */
  libraryScanInstance: (instanceId: string) =>
    invoke<LibraryScan>("library_scan_instance", { instanceId }),
  /** Uploads the scanned file for review; returns its library id. */
  librarySubmit: (name: string, description: string) =>
    invoke<number>("library_submit", { name, description }),
  /** Approved items of every kind and own uploads (own first). */
  libraryMods: () => invoke<LibraryMod[]>("library_mods"),
  libraryWithdraw: (libraryId: number) => invoke<void>("library_withdraw", { libraryId }),
  libraryReport: (libraryId: number, reason: LibraryReportReason, note: string) =>
    invoke<void>("library_report", { libraryId, reason, note }),
  /** Into `instanceId`; a modpack (`instanceId` = null) becomes a new instance. */
  libraryInstall: (libraryId: number, instanceId: string | null, acceptWarnings: boolean) =>
    invoke<LibraryInstall>("library_install", { libraryId, instanceId, acceptWarnings }),

  friendsStatus: () => invoke<FriendsStatus>("friends_status"),
  friendsEnable: () => invoke<Profile>("friends_enable"),
  /** Friends see us online (last heartbeat succeeded); no network call. */
  friendsOnline: () => invoke<boolean>("friends_online"),
  friendsDisable: () => invoke<void>("friends_disable"),
  friendsList: () => invoke<Friend[]>("friends_list"),
  friendRequest: (code: string) => invoke<string>("friend_request", { code }),
  friendRespond: (requestId: number, accept: boolean) =>
    invoke<void>("friend_respond", { requestId, accept }),
  friendRemove: (friend: string) => invoke<void>("friend_remove", { friend }),
  friendBlock: (friend: string) => invoke<void>("friend_block", { friend }),
  chatMessages: (friend: string, after?: number) =>
    invoke<ChatMessage[]>("chat_messages", { friend, after }),
  chatSend: (friend: string, body: string) => invoke<ChatMessage>("chat_send", { friend, body }),
  chatMarkRead: (friend: string) => invoke<void>("chat_mark_read", { friend }),
  /** Our own name for a friend; null removes it (K85). */
  friendSetNickname: (friend: string, nickname: string | null) =>
    invoke<void>("friend_set_nickname", { friend, nickname }),
  groupsList: () => invoke<ChatGroup[]>("groups_list"),
  groupCreate: (name: string, members: string[]) =>
    invoke<number>("group_create", { name, members }),
  groupRename: (group: number, name: string) => invoke<void>("group_rename", { group, name }),
  groupAddMembers: (group: number, members: string[]) =>
    invoke<void>("group_add_members", { group, members }),
  groupRemoveMember: (group: number, member: string) =>
    invoke<void>("group_remove_member", { group, member }),
  groupLeave: (group: number) => invoke<void>("group_leave", { group }),
  groupMessages: (group: number, after?: number) =>
    invoke<GroupMessage[]>("group_messages", { group, after: after ?? null }),
  groupSend: (group: number, body: string) => invoke<GroupMessage>("group_send", { group, body }),
  groupMarkRead: (group: number) => invoke<void>("group_mark_read", { group }),
  // Voice calls (K86); exactly one of friend / group.
  callStart: (target: { friend: string } | { group: number }) =>
    invoke<number>("call_start", {
      friend: "friend" in target ? target.friend : null,
      group: "group" in target ? target.group : null,
    }),
  callJoin: (call: number) => invoke<void>("call_join", { call }),
  callLeave: (call: number) => invoke<void>("call_leave", { call }),
  callSignal: (call: number, signals: OutgoingSignal[]) =>
    invoke<number>("call_signal", { call, signals }),
  callIceConfig: () => invoke<IceConfig>("call_ice_config"),
  callsSnapshot: () => invoke<CallPoll>("calls_snapshot"),
  myShares: () => invoke<SharedList[]>("my_shares"),
  shareInstance: (id: string) => invoke<SharedList>("share_instance", { id }),
  unshareInstance: (id: string) => invoke<void>("unshare_instance", { id }),
  friendSharedLists: (friend: string) => invoke<SharedList[]>("friend_shared_lists", { friend }),
  installFriendMods: (listId: number, instanceId: string, files: string[]) =>
    invoke<FriendInstallResult>("install_friend_mods", { listId, instanceId, files }),

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
  /** One page of skinmc.net skins: by sort, by tag, or the page after `next` (K78). */
  skinmcBrowse: (sort: SkinMcSort, tag: string | null, next: string | null) =>
    invoke<SkinMcPage>("skinmc_browse", { sort, tag, next }),
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

/** Call state and set-up messages from the Rust poll loop (K86). */
export function subscribeCalls(onPoll: (p: CallPoll) => void): Promise<UnlistenFn> {
  return listen<CallPoll>("calls://poll", (e) => onPoll(e.payload));
}

export function isErrorPayload(e: unknown): e is ErrorPayload {
  return typeof e === "object" && e !== null && "code" in e && "detail" in e;
}

/** Normalizes anything thrown by `invoke` into an ErrorPayload. */
export function toErrorPayload(e: unknown): ErrorPayload {
  if (isErrorPayload(e)) return e;
  return { code: "unknown", params: {}, detail: e instanceof Error ? e.message : String(e) };
}
