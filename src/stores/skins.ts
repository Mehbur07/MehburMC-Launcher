import { create } from "zustand";

import { ipc, toErrorPayload } from "../lib/ipc";
import type { Assignment } from "../lib/ipc/bindings/Assignment";
import type { CapeItem } from "../lib/ipc/bindings/CapeItem";
import type { ErrorPayload } from "../lib/ipc/bindings/ErrorPayload";
import type { PlayerImport } from "../lib/ipc/bindings/PlayerImport";
import type { SkinItem } from "../lib/ipc/bindings/SkinItem";
import type { SkinModel } from "../lib/ipc/bindings/SkinModel";
import type { TextureKind } from "../lib/ipc/bindings/TextureKind";
import { useApp } from "./app";

interface SkinsStore {
  skins: SkinItem[];
  capes: CapeItem[];
  assignments: Record<string, Assignment>;
  loaded: boolean;
  load: () => Promise<void>;
  /** Returns the new texture id, or null if cancelled / failed (notice shown). */
  importFile: (kind: TextureKind, path: string) => Promise<string | null>;
  /** Returns the error instead of showing it, so the form can render it inline. */
  importPlayer: (name: string) => Promise<PlayerImport | ErrorPayload>;
  update: (kind: TextureKind, id: string, name?: string, model?: SkinModel) => Promise<void>;
  remove: (kind: TextureKind, id: string) => Promise<void>;
  assign: (accountId: string, kind: TextureKind, id: string | null) => Promise<void>;
}

const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });

export const useSkins = create<SkinsStore>((set, get) => ({
  skins: [],
  capes: [],
  assignments: {},
  loaded: false,

  load: async () => {
    try {
      const v = await ipc.listSkins();
      set({ skins: v.skins, capes: v.capes, assignments: v.assignments, loaded: true });
    } catch (e) {
      notify(e);
    }
  },

  importFile: async (kind, path) => {
    try {
      const id = await ipc.importSkinFile(kind, path);
      await get().load();
      return id;
    } catch (e) {
      notify(e);
      return null;
    }
  },

  importPlayer: async (name) => {
    try {
      const r = await ipc.importPlayerSkin(name);
      await get().load();
      return r;
    } catch (e) {
      return toErrorPayload(e);
    }
  },

  update: async (kind, id, name, model) => {
    try {
      await ipc.updateSkin(kind, id, name, model);
      await get().load();
    } catch (e) {
      notify(e);
    }
  },

  remove: async (kind, id) => {
    try {
      await ipc.deleteSkin(kind, id);
      await get().load();
    } catch (e) {
      notify(e);
    }
  },

  assign: async (accountId, kind, id) => {
    try {
      const assignments = await ipc.assignSkin(accountId, kind, id);
      set({ assignments });
    } catch (e) {
      notify(e);
    }
  },
}));

/** The skin assigned to an account, if any. */
export function accountSkin(s: SkinsStore, accountId: string | null | undefined) {
  const id = accountId ? s.assignments[accountId]?.skin : undefined;
  return id ? (s.skins.find((x) => x.id === id) ?? null) : null;
}

export function accountCape(s: SkinsStore, accountId: string | null | undefined) {
  const id = accountId ? s.assignments[accountId]?.cape : undefined;
  return id ? (s.capes.find((x) => x.id === id) ?? null) : null;
}
