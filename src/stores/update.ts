import { create } from "zustand";

import { ipc, onUpdateProgress, toErrorPayload } from "../lib/ipc";
import type { AppUpdate } from "../lib/ipc/bindings/AppUpdate";
import { useApp } from "./app";

interface UpdateStore {
  info: AppUpdate | null;
  checking: boolean;
  /** Download progress 0..1 while installing, else null. */
  installing: number | null;
  check: () => Promise<void>;
  install: () => Promise<void>;
}

export const useUpdate = create<UpdateStore>((set, get) => ({
  info: null,
  checking: false,
  installing: null,

  check: async () => {
    if (get().checking) return;
    set({ checking: true });
    try {
      set({ info: await ipc.checkAppUpdate() });
    } catch (e) {
      useApp.setState({ notice: toErrorPayload(e) });
    } finally {
      set({ checking: false });
    }
  },

  install: async () => {
    set({ installing: 0 });
    const unlisten = await onUpdateProgress(([done, total]) =>
      set({ installing: total > 0 ? done / total : 0 }),
    );
    try {
      // Restarts the app on success.
      await ipc.installAppUpdate();
    } catch (e) {
      useApp.setState({ notice: toErrorPayload(e) });
      set({ installing: null });
    } finally {
      unlisten();
    }
  },
}));
