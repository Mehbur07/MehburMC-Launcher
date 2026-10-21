import { create } from "zustand";

import { ipc, toErrorPayload } from "../lib/ipc";
import type { ChatGroup } from "../lib/ipc/bindings/ChatGroup";
import type { ChatMessage } from "../lib/ipc/bindings/ChatMessage";
import type { ErrorPayload } from "../lib/ipc/bindings/ErrorPayload";
import type { Friend } from "../lib/ipc/bindings/Friend";
import type { GroupMember } from "../lib/ipc/bindings/GroupMember";
import type { GroupMessage } from "../lib/ipc/bindings/GroupMessage";
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
  /** Group chats (K85). */
  groups: ChatGroup[];
  /** Open group chat (group id); exclusive with `chatWith`. */
  groupWith: number | null;
  groupMessages: Record<number, GroupMessage[]>;
  /** Last error of the background refresh (shown once, not repeated). */
  offline: boolean;
  /** Friends see us online (our heartbeat reaches the server). */
  online: boolean;

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
  /** Our own name for a friend; null removes it. */
  setNickname: (friendId: string, nickname: string | null) => Promise<ErrorPayload | null>;
  openGroup: (groupId: number | null) => Promise<void>;
  pollGroup: () => Promise<void>;
  sendGroup: (body: string) => Promise<ErrorPayload | null>;
  /** Returns the new group id or the error. */
  createGroup: (name: string, members: string[]) => Promise<number | ErrorPayload>;
  renameGroup: (groupId: number, name: string) => Promise<ErrorPayload | null>;
  addGroupMembers: (groupId: number, members: string[]) => Promise<ErrorPayload | null>;
  removeGroupMember: (groupId: number, member: string) => Promise<ErrorPayload | null>;
  leaveGroup: (groupId: number) => Promise<ErrorPayload | null>;
}

const notify = (e: unknown) => useApp.setState({ notice: toErrorPayload(e) });

export const totalUnread = (friends: Friend[], groups: ChatGroup[] = []) =>
  friends.reduce((n, f) => n + (f.status === "accepted" ? f.unread : 0), 0) +
  groups.reduce((n, g) => n + g.unread, 0);

/** The name we show for a friend: our nickname, else theirs (K85). */
export const friendName = (f: Pick<Friend, "nickname" | "displayName">) =>
  f.nickname ?? f.displayName;
export const memberName = (m: GroupMember) => m.nickname ?? m.displayName;

/** Run, refresh, and hand back an error for inline display. */
async function attempt(op: () => Promise<unknown>, after: () => Promise<void>) {
  try {
    await op();
    await after();
    return null;
  } catch (e) {
    return toErrorPayload(e);
  }
}

export const useFriends = create<FriendsStore>((set, get) => ({
  enabled: null,
  profile: null,
  friends: [],
  chatWith: null,
  messages: {},
  groups: [],
  groupWith: null,
  groupMessages: {},
  offline: false,
  online: false,

  status: async () => {
    try {
      const s = await ipc.friendsStatus();
      set({ enabled: s.enabled, profile: s.profile, offline: false, online: s.online });
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
      set({ enabled: true, profile, offline: false, online: true });
      await get().refresh();
      return null;
    } catch (e) {
      return toErrorPayload(e);
    }
  },

  disable: async () => {
    try {
      await ipc.friendsDisable();
      set({
        enabled: false,
        profile: null,
        friends: [],
        messages: {},
        chatWith: null,
        groups: [],
        groupWith: null,
        groupMessages: {},
        online: false,
      });
    } catch (e) {
      notify(e);
    }
  },

  refresh: async () => {
    if (!get().enabled) return;
    try {
      const [friends, online, groups] = await Promise.all([
        ipc.friendsList(),
        ipc.friendsOnline().catch(() => get().online),
        // Before the phase 23 SQL runs there are simply no groups.
        ipc.groupsList().catch(() => get().groups),
      ]);
      const open = get().groupWith;
      set({
        friends,
        offline: false,
        online,
        groups,
        groupWith: open !== null && !groups.some((g) => g.id === open) ? null : open,
      });
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
    set({ chatWith: friendId, ...(friendId ? { groupWith: null } : {}) });
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

  setNickname: async (friendId, nickname) => {
    try {
      await ipc.friendSetNickname(friendId, nickname);
      const clean = nickname?.trim().replace(/\s+/g, " ") || null;
      set((s) => ({
        friends: s.friends.map((f) => (f.id === friendId ? { ...f, nickname: clean } : f)),
      }));
      return null;
    } catch (e) {
      return toErrorPayload(e);
    }
  },

  openGroup: async (groupId) => {
    set({ groupWith: groupId, ...(groupId !== null ? { chatWith: null } : {}) });
    if (groupId === null) return;
    try {
      const msgs = await ipc.groupMessages(groupId);
      set((s) => ({ groupMessages: { ...s.groupMessages, [groupId]: msgs } }));
      await ipc.groupMarkRead(groupId);
      set((s) => ({
        groups: s.groups.map((g) => (g.id === groupId ? { ...g, unread: 0 } : g)),
      }));
    } catch (e) {
      notify(e);
    }
  },

  pollGroup: async () => {
    const groupId = get().groupWith;
    if (groupId === null) return;
    const have = get().groupMessages[groupId] ?? [];
    const last = have[have.length - 1]?.id;
    try {
      const fresh = await ipc.groupMessages(groupId, last);
      if (fresh.length === 0) return;
      set((s) => {
        const list = s.groupMessages[groupId] ?? [];
        const known = new Set(list.map((m) => m.id));
        const merged = [...list, ...fresh.filter((m) => !known.has(m.id))];
        return { groupMessages: { ...s.groupMessages, [groupId]: merged.slice(-MAX_MESSAGES) } };
      });
      const me = get().profile?.id;
      if (fresh.some((m) => m.sender !== me)) await ipc.groupMarkRead(groupId);
    } catch (e) {
      if (!toErrorPayload(e).code.startsWith("net.")) notify(e);
    }
  },

  sendGroup: async (body) => {
    const groupId = get().groupWith;
    if (groupId === null) return null;
    try {
      const msg = await ipc.groupSend(groupId, body);
      set((s) => {
        const list = s.groupMessages[groupId] ?? [];
        if (list.some((m) => m.id === msg.id)) return {};
        return {
          groupMessages: { ...s.groupMessages, [groupId]: [...list, msg].slice(-MAX_MESSAGES) },
        };
      });
      return null;
    } catch (e) {
      return toErrorPayload(e);
    }
  },

  createGroup: async (name, members) => {
    try {
      const id = await ipc.groupCreate(name, members);
      await get().refresh();
      await get().openGroup(id);
      return id;
    } catch (e) {
      return toErrorPayload(e);
    }
  },

  renameGroup: (groupId, name) => attempt(() => ipc.groupRename(groupId, name), get().refresh),
  addGroupMembers: (groupId, members) =>
    attempt(() => ipc.groupAddMembers(groupId, members), get().refresh),
  removeGroupMember: (groupId, member) =>
    attempt(() => ipc.groupRemoveMember(groupId, member), get().refresh),
  leaveGroup: (groupId) =>
    attempt(async () => {
      await ipc.groupLeave(groupId);
      if (get().groupWith === groupId) set({ groupWith: null });
    }, get().refresh),
}));
