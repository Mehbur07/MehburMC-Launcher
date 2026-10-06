import { create } from "zustand";

import { ipc, toErrorPayload } from "../lib/ipc";
import type { ErrorPayload } from "../lib/ipc/bindings/ErrorPayload";
import type { FavoriteServer } from "../lib/ipc/bindings/FavoriteServer";
import type { GameServer } from "../lib/ipc/bindings/GameServer";
import type { ServerStatus } from "../lib/ipc/bindings/ServerStatus";
import { useApp } from "./app";
import { useLogs } from "./logs";

/** Pings younger than this are reused unless a refresh is forced. */
const PING_TTL_MS = 30_000;

export type PingState =
  | { state: "pending"; at: number }
  | { state: "ok"; at: number; status: ServerStatus }
  | { state: "error"; at: number; code: string };

interface ServersStore {
  favorites: FavoriteServer[];
  /** In-game list per instance id. */
  game: Record<string, GameServer[]>;
  /** Ping results by normalised address. */
  pings: Record<string, PingState>;

  loadFavorites: () => Promise<void>;
  loadGame: (instanceId: string) => Promise<void>;
  ping: (address: string, force?: boolean) => Promise<void>;
  addFavorite: (name: string, address: string) => Promise<ErrorPayload | null>;
  removeFavorite: (id: string) => Promise<void>;
  addToGame: (instanceId: string, name: string, address: string) => Promise<ErrorPayload | null>;
  removeFromGame: (instanceId: string, server: GameServer) => Promise<void>;
  join: (instanceId: string, address: string) => Promise<boolean>;
}

const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });

/** Same key for `Host.com` and `host.com:25565`. */
export function addressKey(address: string): string {
  const a = address.trim().toLowerCase();
  return a.endsWith(":25565") ? a.slice(0, -":25565".length) : a;
}

export const isFavorite = (favorites: FavoriteServer[], address: string) =>
  favorites.some((f) => addressKey(f.address) === addressKey(address));

export const useServers = create<ServersStore>((set, get) => ({
  favorites: [],
  game: {},
  pings: {},

  loadFavorites: async () => {
    try {
      set({ favorites: await ipc.listFavoriteServers() });
    } catch (e) {
      notify(e);
    }
  },

  loadGame: async (instanceId) => {
    try {
      const list = await ipc.listGameServers(instanceId);
      set((s) => ({ game: { ...s.game, [instanceId]: list } }));
    } catch (e) {
      set((s) => ({ game: { ...s.game, [instanceId]: [] } }));
      notify(e);
    }
  },

  ping: async (address, force = false) => {
    const key = addressKey(address);
    const prev = get().pings[key];
    const now = Date.now();
    if (prev && (prev.state === "pending" || (!force && now - prev.at < PING_TTL_MS))) return;
    set((s) => ({ pings: { ...s.pings, [key]: { state: "pending", at: now } } }));
    let next: PingState;
    try {
      next = { state: "ok", at: Date.now(), status: await ipc.pingServer(address) };
    } catch (e) {
      next = { state: "error", at: Date.now(), code: toErrorPayload(e).code };
    }
    set((s) => ({ pings: { ...s.pings, [key]: next } }));
  },

  addFavorite: async (name, address) => {
    try {
      set({ favorites: await ipc.addFavoriteServer(name, address) });
      return null;
    } catch (e) {
      return toErrorPayload(e);
    }
  },

  removeFavorite: async (id) => {
    try {
      set({ favorites: await ipc.removeFavoriteServer(id) });
    } catch (e) {
      notify(e);
    }
  },

  addToGame: async (instanceId, name, address) => {
    try {
      const list = await ipc.addGameServer(instanceId, name, address);
      set((s) => ({ game: { ...s.game, [instanceId]: list } }));
      return null;
    } catch (e) {
      return toErrorPayload(e);
    }
  },

  removeFromGame: async (instanceId, server) => {
    try {
      const list = await ipc.removeGameServer(instanceId, server.index, server.address);
      set((s) => ({ game: { ...s.game, [instanceId]: list } }));
    } catch (e) {
      notify(e);
      void get().loadGame(instanceId);
    }
  },

  join: async (instanceId, address) => {
    try {
      useLogs.getState().clear(instanceId);
      await ipc.joinServer(instanceId, address);
      return true;
    } catch (e) {
      notify(e);
      return false;
    }
  },
}));
