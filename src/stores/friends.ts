import { create } from "zustand";

import { ipc, toErrorPayload } from "../lib/ipc";
import type { ChatMessage } from "../lib/ipc/bindings/ChatMessage";
import type { ErrorPayload } from "../lib/ipc/bindings/ErrorPayload";
import type { Friend } from "../lib/ipc/bindings/Friend";
import type { Profile } from "../lib/ipc/bindings/Profile";
import { useApp } from "./app";

/** Messages kept per conversation in memory. */
const MAX_MESSAGES = 500;

interface FriendsStore {
  /** null until the first status check. */
  enabled: boolean | null;
  profile: Profile | null;
  friends: Friend[];
  /** Open conversation (friend id). */
  chatWith: string | null;
  messages: Record<string, ChatMessage[]>;
  /** Last error of the background refresh (shown once, not repeated). */
  offline: boolean;

  status: () => Promise<void>;
  /** Returns the error for inline display (consent screen). */
  enable: () => Promise<ErrorPayload | null>;
  disable: () => Promise<void>;
  refresh: () => Promise<void>;
  sendRequest: (code: string) => Promise<ErrorPayload | string>;
  respond: (requestId: number, accept: boolean) => Promise<void>;
  remove: (friendId: string) => Promise<void>;
  block: (friendId: string) => Promise<void>;
  openChat: (friendId: string | null) => Promise<void>;
  /** Fetches new messages of the open conversation. */
  pollChat: () => Promise<void>;
  send: (body: string) => Promise<ErrorPayload | null>;
}

const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });

export const totalUnread = (friends: Friend[]) =>
  friends.reduce((n, f) => n + (f.status === "accepted" ? f.unread : 0), 0);

export const useFriends = create<FriendsStore>((set, get) => ({
  enabled: null,
  profile: null,
  friends: [],
  chatWith: null,
  messages: {},
  offline: false,

  status: async () => {
    try {
      const s = await ipc.friendsStatus();
      set({ enabled: s.enabled, profile: s.profile, offline: false });
      if (s.enabled) await get().refresh();
    } catch (e) {
      const p = toErrorPayload(e);
      // Enabled but unreachable (offline): keep the page usable.
      if (p.code.startsWith("net.")) set({ offline: true, enabled: get().enabled ?? true });
      else notify(e);
    }
  },

  enable: async () => {
    try {
      const profile = await ipc.friendsEnable();
      set({ enabled: true, profile, offline: false });
      await get().refresh();
      return null;
    } catch (e) {
      return toErrorPayload(e);
    }
  },

  disable: async () => {
    try {
      await ipc.friendsDisable();
      set({ enabled: false, profile: null, friends: [], messages: {}, chatWith: null });
    } catch (e) {
      notify(e);
    }
  },

  refresh: async () => {
    if (!get().enabled) return;
    try {
      const friends = await ipc.friendsList();
      set({ friends, offline: false });
    } catch (e) {
      if (toErrorPayload(e).code.startsWith("net.")) set({ offline: true });
      else notify(e);
    }
  },

  sendRequest: async (code) => {
    try {
      const r = await ipc.friendRequest(code);
      await get().refresh();
      return r;
    } catch (e) {
      return toErrorPayload(e);
    }
  },

  respond: async (requestId, accept) => {
    try {
      await ipc.friendRespond(requestId, accept);
      await get().refresh();
    } catch (e) {
      notify(e);
    }
  },

  remove: async (friendId) => {
    try {
      await ipc.friendRemove(friendId);
      if (get().chatWith === friendId) set({ chatWith: null });
      await get().refresh();
    } catch (e) {
      notify(e);
    }
  },

  block: async (friendId) => {
    try {
      await ipc.friendBlock(friendId);
      if (get().chatWith === friendId) set({ chatWith: null });
      await get().refresh();
    } catch (e) {
      notify(e);
    }
  },

  openChat: async (friendId) => {
    set({ chatWith: friendId });
    if (!friendId) return;
    try {
      const msgs = await ipc.chatMessages(friendId);
      set((s) => ({ messages: { ...s.messages, [friendId]: msgs } }));
      await ipc.chatMarkRead(friendId);
      set((s) => ({
        friends: s.friends.map((f) => (f.id === friendId ? { ...f, unread: 0 } : f)),
      }));
    } catch (e) {
      notify(e);
    }
  },

  pollChat: async () => {
    const friendId = get().chatWith;
    if (!friendId) return;
    const have = get().messages[friendId] ?? [];
    const last = have[have.length - 1]?.id;
    try {
      const fresh = await ipc.chatMessages(friendId, last);
      if (fresh.length === 0) return;
      set((s) => {
        const known = new Set((s.messages[friendId] ?? []).map((m) => m.id));
        const merged = [...(s.messages[friendId] ?? []), ...fresh.filter((m) => !known.has(m.id))];
        return { messages: { ...s.messages, [friendId]: merged.slice(-MAX_MESSAGES) } };
      });
      if (fresh.some((m) => m.sender === friendId)) await ipc.chatMarkRead(friendId);
    } catch (e) {
      if (!toErrorPayload(e).code.startsWith("net.")) notify(e);
    }
  },

  send: async (body) => {
    const friendId = get().chatWith;
    if (!friendId) return null;
    try {
      const msg = await ipc.chatSend(friendId, body);
      set((s) => {
        const list = s.messages[friendId] ?? [];
        if (list.some((m) => m.id === msg.id)) return {};
        return { messages: { ...s.messages, [friendId]: [...list, msg].slice(-MAX_MESSAGES) } };
      });
      return null;
    } catch (e) {
      return toErrorPayload(e);
    }
  },
}));
