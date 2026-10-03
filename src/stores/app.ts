import { create } from "zustand";

import { ipc, toErrorPayload } from "../lib/ipc";
import type { Bootstrap } from "../lib/ipc/bindings/Bootstrap";
import type { ErrorPayload } from "../lib/ipc/bindings/ErrorPayload";
import type { Settings } from "../lib/ipc/bindings/Settings";

export type View =
  | "home"
  | "instances"
  | "instance"
  | "browse"
  | "accounts"
  | "skins"
  | "downloads"
  | "console"
  | "settings";

type Status = "loading" | "ready" | "fatal";

interface AppStore {
  status: Status;
  boot: Bootstrap | null;
  settings: Settings | null;
  view: View;
  /** Instance shown by the "instance" detail view. */
  detailId: string | null;
  /** Console filter: instance whose output is shown. */
  consoleInstance: string | null;
  wizardOpen: boolean;
  /** Last non-fatal error, shown as a dismissible notice. */
  notice: ErrorPayload | null;
  /** Fatal startup error (data folder unusable). */
  fatal: ErrorPayload | null;

  load: () => Promise<void>;
  setView: (view: View) => void;
  openInstance: (id: string) => void;
  openConsole: (instanceId: string) => void;
  setWizard: (open: boolean) => void;
  updateSettings: (patch: Partial<Settings>) => Promise<void>;
  dismissNotice: () => void;
}

export const useApp = create<AppStore>((set, get) => ({
  status: "loading",
  boot: null,
  settings: null,
  view: "home",
  detailId: null,
  consoleInstance: null,
  wizardOpen: false,
  notice: null,
  fatal: null,

  load: async () => {
    try {
      const boot = await ipc.getBootstrap();
      set({
        boot,
        settings: boot.settings,
        status: boot.startupError ? "fatal" : "ready",
        fatal: boot.startupError,
      });
    } catch (e) {
      set({ status: "fatal", fatal: toErrorPayload(e) });
    }
  },

  setView: (view) => set({ view }),
  openInstance: (id) => set({ view: "instance", detailId: id }),
  openConsole: (instanceId) => set({ view: "console", consoleInstance: instanceId }),
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
}));

/** Microsoft sign-in needs an Azure client id (env var or settings). */
export const msaConfigured = (s: AppStore) =>
  !!s.boot?.msaConfigured || !!s.settings?.auth.msaClientId?.trim();
