import { create } from "zustand";

import { ipc, toErrorPayload } from "../lib/ipc";
import type { Bootstrap } from "../lib/ipc/bindings/Bootstrap";
import type { ErrorPayload } from "../lib/ipc/bindings/ErrorPayload";
import type { ProjectType } from "../lib/ipc/bindings/ProjectType";
import type { Settings } from "../lib/ipc/bindings/Settings";

export type View =
  | "home"
  | "instances"
  | "instance"
  | "browse"
  | "modToggle"
  | "accounts"
  | "skins"
  | "friends"
  | "servers"
  | "downloads"
  | "console"
  | "whatsNew"
  | "settings";

type Status = "loading" | "ready" | "fatal";

interface AppStore {
  status: Status;
  boot: Bootstrap | null;
  settings: Settings | null;
  view: View;
  /** Instance shown by the "instance" detail view. */
  detailId: string | null;
  /** Tab the detail view opens on (e.g. "mods" from the crash dialog). */
  detailTab: string | null;
  /** Console filter: instance whose output is shown. */
  consoleInstance: string | null;
  /** Instance the content browser installs into (null = none / modpacks). */
  browseTarget: string | null;
  browseType: ProjectType;
  wizardOpen: boolean;
  /** Release notes not looked at yet (first start after an update). */
  whatsNewUnseen: boolean;
  /** Last non-fatal error, shown as a dismissible notice. */
  notice: ErrorPayload | null;
  /** Fatal startup error (data folder unusable). */
  fatal: ErrorPayload | null;

  load: () => Promise<void>;
  setView: (view: View) => void;
  openInstance: (id: string, tab?: string) => void;
  openConsole: (instanceId: string) => void;
  openBrowse: (instanceId: string | null, type?: ProjectType) => void;
  setWizard: (open: boolean) => void;
  updateSettings: (patch: Partial<Settings>) => Promise<void>;
  dismissNotice: () => void;
  markWhatsNewSeen: () => void;
}

export const useApp = create<AppStore>((set, get) => ({
  status: "loading",
  boot: null,
  settings: null,
  view: "home",
  detailId: null,
  detailTab: null,
  consoleInstance: null,
  browseTarget: null,
  browseType: "mod",
  wizardOpen: false,
  whatsNewUnseen: false,
  notice: null,
  fatal: null,

  load: async () => {
    try {
      const boot = await ipc.getBootstrap();
      const updated = boot.updatedFrom !== null;
      set({
        boot,
        settings: boot.settings,
        status: boot.startupError ? "fatal" : "ready",
        fatal: boot.startupError,
        // First start after an update opens the release notes.
        ...(updated ? { view: "whatsNew" as const, whatsNewUnseen: true } : {}),
      });
    } catch (e) {
      set({ status: "fatal", fatal: toErrorPayload(e) });
    }
  },

  setView: (view) => set({ view }),
  openInstance: (id, tab) => set({ view: "instance", detailId: id, detailTab: tab ?? null }),
  openConsole: (instanceId) => set({ view: "console", consoleInstance: instanceId }),
  openBrowse: (instanceId, type) =>
    set((s) => ({ view: "browse", browseTarget: instanceId, browseType: type ?? s.browseType })),
  setWizard: (open) => set({ wizardOpen: open }),

  // Optimistic: apply immediately, roll back if the backend rejects it.
  updateSettings: async (patch) => {
    const previous = get().settings;
    if (!previous) return;
    const next = { ...previous, ...patch };
    set({ settings: next });
    try {
      const saved = await ipc.saveSettings(next);
      set({ settings: saved });
    } catch (e) {
      set({ settings: previous, notice: toErrorPayload(e) });
    }
  },

  dismissNotice: () => set({ notice: null }),
  markWhatsNewSeen: () => set({ whatsNewUnseen: false }),
}));
