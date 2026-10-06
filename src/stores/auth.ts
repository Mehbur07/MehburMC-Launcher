import { create } from "zustand";

import { ipc } from "../lib/ipc";
import type { AuthStatus } from "../lib/ipc/bindings/AuthStatus";

interface AuthStore {
  /** null until the first check. */
  status: AuthStatus | null;
  /** `check` asks the server (rank, ban); otherwise the saved state. */
  load: (check: boolean) => Promise<void>;
  signOut: () => Promise<void>;
}

export const useAuth = create<AuthStore>((set) => ({
  status: null,

  load: async (check) => {
    try {
      set({ status: await ipc.authStatus(check) });
    } catch {
      // Startup errors are shown by the app shell; keep the last state.
    }
  },

  signOut: async () => {
    try {
      await ipc.authSignOut();
    } finally {
      set({ status: await ipc.authStatus(false).catch(() => null) });
    }
  },
}));

/** The launcher is usable: signed in with an email account, not banned. */
export function canUse(s: AuthStatus | null): boolean {
  return s !== null && s.signedIn && s.ban === null;
}
