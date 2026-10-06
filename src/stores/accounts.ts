import { create } from "zustand";

import { ipc, toErrorPayload } from "../lib/ipc";
import type { Account } from "../lib/ipc/bindings/Account";
import type { ErrorPayload } from "../lib/ipc/bindings/ErrorPayload";
import { useApp } from "./app";

interface AccountsStore {
  accounts: Account[];
  selected: string | null;
  /** Profile photos (`account id → data: URI`); missing = skin head. */
  avatars: Record<string, string>;
  /** Game default skin of accounts without a skin of their own. */
  defaultSkins: Record<string, string>;
  /** Accounts whose name another MehburMC user reserved first. */
  nameConflicts: string[];
  load: () => Promise<void>;
  /** Opens the file picker and sets the chosen PNG as the account's photo. */
  pickPhoto: (id: string, title?: string) => Promise<void>;
  /** Back to the skin head. */
  resetPhoto: (id: string) => Promise<void>;
  /** Returns the error instead of showing it, so forms can render it inline. */
  addOffline: (name: string) => Promise<ErrorPayload | null>;
  /** Like addOffline, returns the error for inline display. */
  rename: (id: string, name: string) => Promise<ErrorPayload | null>;
  remove: (id: string) => Promise<void>;
  select: (id: string) => Promise<void>;
}

const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });

export const useAccounts = create<AccountsStore>((set, get) => ({
  accounts: [],
  selected: null,
  avatars: {},
  defaultSkins: {},
  nameConflicts: [],

  load: async () => {
    try {
      const v = await ipc.listAccounts();
      set({ accounts: v.accounts, selected: v.selected });
    } catch (e) {
      notify(e);
    }
    try {
      const faces = await ipc.accountAvatars();
      set({
        avatars: faces.photos,
        defaultSkins: faces.defaultSkins,
        nameConflicts: faces.nameConflicts,
      });
    } catch {
      // Photos are optional; the skin head is shown instead.
    }
  },

  pickPhoto: async (id, title) => {
    try {
      const picked = await ipc.pickPath("avatar", undefined, title);
      if (picked === null) return;
      const uri = await ipc.setAccountAvatar(id);
      set((s) => ({ avatars: { ...s.avatars, [id]: uri } }));
    } catch (e) {
      notify(e);
    }
  },

  resetPhoto: async (id) => {
    try {
      await ipc.clearAccountAvatar(id);
      set((s) => {
        const avatars = { ...s.avatars };
        delete avatars[id];
        return { avatars };
      });
    } catch (e) {
      notify(e);
    }
  },

  addOffline: async (name) => {
    try {
      await ipc.addOfflineAccount(name);
      await get().load();
      return null;
    } catch (e) {
      return toErrorPayload(e);
    }
  },

  rename: async (id, name) => {
    try {
      await ipc.renameAccount(id, name);
      await get().load();
      return null;
    } catch (e) {
      return toErrorPayload(e);
    }
  },

  remove: async (id) => {
    try {
      const v = await ipc.removeAccount(id);
      set({ accounts: v.accounts, selected: v.selected });
    } catch (e) {
      notify(e);
    }
  },

  select: async (id) => {
    try {
      const v = await ipc.selectAccount(id);
      set({ accounts: v.accounts, selected: v.selected });
    } catch (e) {
      notify(e);
    }
  },
}));

export const selectedAccount = (s: AccountsStore) =>
  s.accounts.find((a) => a.id === s.selected) ?? null;
