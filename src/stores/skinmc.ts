import { create } from "zustand";

import { ipc, toErrorPayload } from "../lib/ipc";
import type { ErrorPayload } from "../lib/ipc/bindings/ErrorPayload";
import type { SkinMcSkin } from "../lib/ipc/bindings/SkinMcSkin";
import type { SkinMcSort } from "../lib/ipc/bindings/SkinMcSort";

/** The SkinMC gallery (K78), kept while the user switches tabs. */
interface SkinMcStore {
  sort: SkinMcSort;
  /** Tag search; "" browses by sort order. */
  tag: string;
  items: SkinMcSkin[];
  next: string | null;
  loading: boolean;
  error: ErrorPayload | null;
  /** Bumped on every new search so late answers of an old one are dropped. */
  request: number;

  open: (sort: SkinMcSort, tag: string) => Promise<void>;
  more: () => Promise<void>;
}

export const useSkinMc = create<SkinMcStore>((set, get) => ({
  sort: "latest",
  tag: "",
  items: [],
  next: null,
  loading: false,
  error: null,
  request: 0,

  open: async (sort, tag) => {
    const request = get().request + 1;
    set({ sort, tag, items: [], next: null, loading: true, error: null, request });
    try {
      const page = await ipc.skinmcBrowse(sort, tag.trim() || null, null);
      if (get().request !== request) return;
      set({ items: page.skins, next: page.next ?? null });
    } catch (e) {
      if (get().request === request) set({ error: toErrorPayload(e) });
    } finally {
      if (get().request === request) set({ loading: false });
    }
  },

  more: async () => {
    const { next, loading, sort, tag, request } = get();
    if (!next || loading) return;
    set({ loading: true, error: null });
    try {
      const page = await ipc.skinmcBrowse(sort, tag.trim() || null, next);
      if (get().request !== request) return;
      // The site's lists move while browsing; never show a skin twice.
      set((s) => ({
        items: [...s.items, ...page.skins.filter((p) => !s.items.some((i) => i.id === p.id))],
        next: page.next ?? null,
      }));
    } catch (e) {
      if (get().request === request) set({ error: toErrorPayload(e) });
    } finally {
      if (get().request === request) set({ loading: false });
    }
  },
}));
