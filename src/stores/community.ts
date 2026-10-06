import { create } from "zustand";

import { ipc, toErrorPayload } from "../lib/ipc";
import type { ErrorPayload } from "../lib/ipc/bindings/ErrorPayload";
import type { ReportReason } from "../lib/ipc/bindings/ReportReason";
import type { SharedTexture } from "../lib/ipc/bindings/SharedTexture";
import type { TextureKind } from "../lib/ipc/bindings/TextureKind";
import type { Visibility } from "../lib/ipc/bindings/Visibility";

interface CommunityStore {
  /** null until loaded. */
  items: SharedTexture[] | null;
  loading: boolean;
  error: ErrorPayload | null;

  load: () => Promise<void>;
  share: (
    kind: TextureKind,
    id: string,
    name: string,
    visibility: Visibility,
  ) => Promise<ErrorPayload | null>;
  unshare: (shareId: number) => Promise<ErrorPayload | null>;
  report: (shareId: number, reason: ReportReason, note: string) => Promise<ErrorPayload | null>;
  /** Founder: hides a share from everyone but its owner (K73/K74). */
  adminRemove: (shareId: number) => Promise<ErrorPayload | null>;
}

/** The caller's own share of a library texture (library id = SHA-1). */
export const myShare = (items: SharedTexture[] | null, kind: TextureKind, libraryId: string) =>
  items?.find((s) => s.mine && s.kind === kind && s.sha1 === libraryId) ?? null;

export const useCommunity = create<CommunityStore>((set, get) => ({
  items: null,
  loading: false,
  error: null,

  load: async () => {
    if (get().loading) return;
    set({ loading: true });
    try {
      set({ items: await ipc.communityTextures(), error: null });
    } catch (e) {
      set({ error: toErrorPayload(e) });
    } finally {
      set({ loading: false });
    }
  },

  share: async (kind, id, name, visibility) => {
    try {
      await ipc.shareTexture(kind, id, name, visibility);
      await get().load();
      return null;
    } catch (e) {
      return toErrorPayload(e);
    }
  },

  unshare: async (shareId) => {
    try {
      await ipc.unshareTexture(shareId);
      set((s) => ({ items: s.items?.filter((i) => i.id !== shareId) ?? null }));
      return null;
    } catch (e) {
      return toErrorPayload(e);
    }
  },

  adminRemove: async (shareId) => {
    try {
      await ipc.adminHideTexture(shareId);
      set((s) => ({ items: s.items?.filter((i) => i.id !== shareId) ?? null }));
      return null;
    } catch (e) {
      return toErrorPayload(e);
    }
  },

  report: async (shareId, reason, note) => {
    try {
      await ipc.reportTexture(shareId, reason, note);
      // Reported shares are no longer listed for the reporter.
      set((s) => ({ items: s.items?.filter((i) => i.id !== shareId) ?? null }));
      return null;
    } catch (e) {
      return toErrorPayload(e);
    }
  },
}));
