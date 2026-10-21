import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  groupsList: vi.fn(),
  friendsStatus: vi.fn(),
  friendsEnable: vi.fn(),
  friendsDisable: vi.fn(),
  friendsList: vi.fn(),
  friendsOnline: vi.fn(),
  friendRequest: vi.fn(),
  friendRemove: vi.fn(),
  chatMessages: vi.fn(),
  chatSend: vi.fn(),
  chatMarkRead: vi.fn(),
  friendSetNickname: vi.fn(),
  groupCreate: vi.fn(),
  groupMessages: vi.fn(),
  groupMarkRead: vi.fn(),
  groupSend: vi.fn(),
  groupLeave: vi.fn(),
}));
vi.mock("../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../lib/ipc")>()),
  ipc: ipcMock,
}));

import type { ChatGroup } from "../lib/ipc/bindings/ChatGroup";
import type { ChatMessage } from "../lib/ipc/bindings/ChatMessage";
import type { Friend } from "../lib/ipc/bindings/Friend";
import type { GroupMessage } from "../lib/ipc/bindings/GroupMessage";
import { useApp } from "./app";
import { friendName, totalUnread, useFriends } from "./friends";

const friend = (over: Partial<Friend> = {}): Friend => ({
  id: "f1",
  friendCode: "MEHBUR-AAAA",
  nickname: null,
  displayName: "Ali",
  status: "accepted",
  incoming: false,
  requestId: 1,
  unread: 0,
  avatar: null,
  online: false,
  ...over,
});

const msg = (id: number, sender = "f1"): ChatMessage => ({
  id,
  sender,
  recipient: sender === "f1" ? "me" : "f1",
  body: `m${id}`,
  createdAt: "2026-10-05T10:00:00Z",
  readAt: null,
});

const group = (over: Partial<ChatGroup> = {}): ChatGroup => ({
  id: 5,
  name: "Ekip",
  owner: "me",
  isOwner: true,
  members: [
    { id: "me", displayName: "Ben", nickname: null, online: true },
    { id: "f1", displayName: "Ali", nickname: null, online: false },
  ],
  unread: 0,
  lastMessageAt: null,
  ...over,
});

const gmsg = (id: number, sender = "f1"): GroupMessage => ({
  id,
  groupId: 5,
  sender,
  body: `g${id}`,
  createdAt: "2026-10-21T10:00:00Z",
});

const profile = { id: "me", friendCode: "MEHBUR-MEME", displayName: "Ben" };

describe("friends store", () => {
  beforeEach(() => {
    useFriends.setState({
      enabled: null,
      profile: null,
      friends: [],
      chatWith: null,
      messages: {},
      groups: [],
      groupWith: null,
      groupMessages: {},
      offline: false,
    });
    useApp.setState({ notice: null });
    vi.clearAllMocks();
    ipcMock.friendsList.mockResolvedValue([]);
    ipcMock.friendsOnline.mockResolvedValue(false);
    ipcMock.chatMarkRead.mockResolvedValue(undefined);
    ipcMock.groupsList.mockResolvedValue([]);
    ipcMock.groupMarkRead.mockResolvedValue(undefined);
  });

  it("counts unread messages of accepted friends only", () => {
    expect(
      totalUnread([
        friend({ unread: 2 }),
        friend({ id: "f2", unread: 3 }),
        friend({ id: "f3", status: "pending", unread: 9 }),
      ]),
    ).toBe(5);
  });

  it("loads the status and the friend list when enabled", async () => {
    ipcMock.friendsStatus.mockResolvedValue({ enabled: true, profile });
    ipcMock.friendsList.mockResolvedValue([friend()]);
    await useFriends.getState().status();
    const s = useFriends.getState();
    expect(s.enabled).toBe(true);
    expect(s.profile).toEqual(profile);
    expect(s.friends).toHaveLength(1);
  });

  it("does not fetch friends while disabled", async () => {
    ipcMock.friendsStatus.mockResolvedValue({ enabled: false, profile: null });
    await useFriends.getState().status();
    expect(useFriends.getState().enabled).toBe(false);
    expect(ipcMock.friendsList).not.toHaveBeenCalled();
  });

  it("marks network failures as offline instead of showing a notice", async () => {
    useFriends.setState({ enabled: true });
    ipcMock.friendsList.mockRejectedValue({ code: "net.offline", params: {}, detail: "" });
    await useFriends.getState().refresh();
    expect(useFriends.getState().offline).toBe(true);
    expect(useApp.getState().notice).toBeNull();
  });

  it("returns enable errors for inline display", async () => {
    ipcMock.friendsEnable.mockRejectedValue({ code: "friends.disabled", params: {}, detail: "" });
    const err = await useFriends.getState().enable();
    expect(err?.code).toBe("friends.disabled");
    expect(useFriends.getState().enabled).toBeNull();
  });

  it("returns the request outcome or the error", async () => {
    useFriends.setState({ enabled: true });
    ipcMock.friendRequest.mockResolvedValueOnce("pending");
    expect(await useFriends.getState().sendRequest("MEHBUR-AAAA")).toBe("pending");
    expect(ipcMock.friendsList).toHaveBeenCalled();

    ipcMock.friendRequest.mockRejectedValueOnce({
      code: "friends.codeNotFound",
      params: {},
      detail: "",
    });
    const r = await useFriends.getState().sendRequest("MEHBUR-ZZZZ");
    expect(typeof r === "object" && r.code).toBe("friends.codeNotFound");
  });

  it("opens a chat, loads messages and clears the unread count", async () => {
    useFriends.setState({ enabled: true, friends: [friend({ unread: 4 })] });
    ipcMock.chatMessages.mockResolvedValue([msg(1), msg(2)]);
    await useFriends.getState().openChat("f1");
    const s = useFriends.getState();
    expect(s.chatWith).toBe("f1");
    expect(s.messages.f1?.map((m) => m.id)).toEqual([1, 2]);
    expect(s.friends[0]?.unread).toBe(0);
    expect(ipcMock.chatMarkRead).toHaveBeenCalledWith("f1");
  });

  it("polls only newer messages and merges without duplicates", async () => {
    useFriends.setState({ chatWith: "f1", messages: { f1: [msg(1), msg(2)] } });
    ipcMock.chatMessages.mockResolvedValue([msg(2), msg(3)]);
    await useFriends.getState().pollChat();
    expect(ipcMock.chatMessages).toHaveBeenCalledWith("f1", 2);
    expect(useFriends.getState().messages.f1?.map((m) => m.id)).toEqual([1, 2, 3]);
    expect(ipcMock.chatMarkRead).toHaveBeenCalledWith("f1");
  });

  it("appends a sent message once even if a poll already delivered it", async () => {
    useFriends.setState({ chatWith: "f1", messages: { f1: [msg(1)] } });
    ipcMock.chatSend.mockResolvedValue(msg(2, "me"));
    expect(await useFriends.getState().send("hi")).toBeNull();
    expect(await useFriends.getState().send("hi")).toBeNull();
    expect(useFriends.getState().messages.f1?.map((m) => m.id)).toEqual([1, 2]);
  });

  it("closes the chat when the friend is removed", async () => {
    useFriends.setState({ enabled: true, chatWith: "f1" });
    ipcMock.friendRemove.mockResolvedValue(undefined);
    await useFriends.getState().remove("f1");
    expect(useFriends.getState().chatWith).toBeNull();
  });

  it("forgets everything when disabled", async () => {
    useFriends.setState({ enabled: true, profile, friends: [friend()], chatWith: "f1" });
    ipcMock.friendsDisable.mockResolvedValue(undefined);
    await useFriends.getState().disable();
    const s = useFriends.getState();
    expect(s.enabled).toBe(false);
    expect(s.profile).toBeNull();
    expect(s.friends).toEqual([]);
    expect(s.chatWith).toBeNull();
  });

  it("tracks whether friends see us online", async () => {
    ipcMock.friendsStatus.mockResolvedValue({ enabled: true, profile, online: true });
    ipcMock.friendsOnline.mockResolvedValue(true);
    await useFriends.getState().status();
    expect(useFriends.getState().online).toBe(true);
    ipcMock.friendsOnline.mockResolvedValue(false);
    await useFriends.getState().refresh();
    expect(useFriends.getState().online).toBe(false);
  });

  it("adds group unread counts to the total", () => {
    expect(
      totalUnread([friend({ unread: 2 })], [group({ unread: 3 }), group({ id: 6, unread: 1 })]),
    ).toBe(6);
  });

  it("prefers our nickname over the friend's own name", () => {
    expect(friendName(friend())).toBe("Ali");
    expect(friendName(friend({ nickname: "Kanka" }))).toBe("Kanka");
  });

  it("saves a cleaned nickname and removes it with null", async () => {
    useFriends.setState({ friends: [friend()] });
    ipcMock.friendSetNickname.mockResolvedValue(undefined);
    expect(await useFriends.getState().setNickname("f1", "  Can   dost ")).toBeNull();
    expect(useFriends.getState().friends[0]?.nickname).toBe("Can dost");
    await useFriends.getState().setNickname("f1", null);
    expect(ipcMock.friendSetNickname).toHaveBeenLastCalledWith("f1", null);
    expect(useFriends.getState().friends[0]?.nickname).toBeNull();
  });

  it("keeps groups empty when the server has no group tables yet", async () => {
    useFriends.setState({ enabled: true });
    ipcMock.groupsList.mockRejectedValue({ code: "friends.server", params: {}, detail: "" });
    await useFriends.getState().refresh();
    expect(useFriends.getState().groups).toEqual([]);
    expect(useApp.getState().notice).toBeNull();
  });

  it("opening a group closes the private chat and clears its unread count", async () => {
    useFriends.setState({ enabled: true, chatWith: "f1", groups: [group({ unread: 3 })] });
    ipcMock.groupMessages.mockResolvedValue([gmsg(1), gmsg(2)]);
    await useFriends.getState().openGroup(5);
    const s = useFriends.getState();
    expect(s.chatWith).toBeNull();
    expect(s.groupWith).toBe(5);
    expect(s.groupMessages[5]?.map((m) => m.id)).toEqual([1, 2]);
    expect(s.groups[0]?.unread).toBe(0);
    expect(ipcMock.groupMarkRead).toHaveBeenCalledWith(5);
  });

  it("polls a group after the last message and skips mark-read for our own", async () => {
    useFriends.setState({ profile, groupWith: 5, groupMessages: { 5: [gmsg(1)] } });
    ipcMock.groupMessages.mockResolvedValue([gmsg(2, "me")]);
    await useFriends.getState().pollGroup();
    expect(ipcMock.groupMessages).toHaveBeenCalledWith(5, 1);
    expect(useFriends.getState().groupMessages[5]?.map((m) => m.id)).toEqual([1, 2]);
    expect(ipcMock.groupMarkRead).not.toHaveBeenCalled();
  });

  it("creates a group and opens it", async () => {
    useFriends.setState({ enabled: true });
    ipcMock.groupCreate.mockResolvedValue(5);
    ipcMock.groupsList.mockResolvedValue([group()]);
    ipcMock.groupMessages.mockResolvedValue([]);
    expect(await useFriends.getState().createGroup("Ekip", ["f1"])).toBe(5);
    expect(ipcMock.groupCreate).toHaveBeenCalledWith("Ekip", ["f1"]);
    expect(useFriends.getState().groupWith).toBe(5);
  });

  it("closes a group we left", async () => {
    useFriends.setState({ enabled: true, groupWith: 5, groups: [group()] });
    ipcMock.groupLeave.mockResolvedValue(undefined);
    expect(await useFriends.getState().leaveGroup(5)).toBeNull();
    const s = useFriends.getState();
    expect(s.groupWith).toBeNull();
    expect(s.groups).toEqual([]);
  });
});
