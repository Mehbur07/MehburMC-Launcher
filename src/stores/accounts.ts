import { create } from "zustand";

import { ipc, toErrorPayload } from "../lib/ipc";
import type { Account } from "../lib/ipc/bindings/Account";
import type { ErrorPayload } from "../lib/ipc/bindings/ErrorPayload";
import { useApp } from "./app";

interface AccountsStore {
  accounts: Account[];
  selected: string | null;
  load: () => Promise<void>;
  /** Returns the error instead of showing it, so forms can render it inline. */
  addOffline: (name: string) => Promise<ErrorPayload | null>;
  remove: (id: string) => Promise<void>;
  select: (id: string) => Promise<void>;
}

const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });

export const useAccounts = create<AccountsStore>((set, get) => ({
  accounts: [],
  selected: null,

  load: async () => {
    try {
      const v = await ipc.listAccounts();
      set({ accounts: v.accounts, selected: v.selected });
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
