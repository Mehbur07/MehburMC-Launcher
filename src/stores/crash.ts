import { create } from "zustand";

import type { CrashInfo } from "../lib/ipc/bindings/CrashInfo";

interface CrashStore {
  /** Analysis shown by the crash dialog. */
  info: CrashInfo | null;
  show: (info: CrashInfo) => void;
  close: () => void;
}

export const useCrash = create<CrashStore>((set) => ({
  info: null,
  show: (info) => set({ info }),
  close: () => set({ info: null }),
}));
