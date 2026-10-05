import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  friendsStatus: vi.fn(),
  friendsEnable: vi.fn(),
  friendsDisable: vi.fn(),
  friendsList: vi.fn(),
  friendRequest: vi.fn(),
  friendRemove: vi.fn(),
  chatMessages: vi.fn(),
  chatSend: vi.fn(),
  chatMarkRead: vi.fn(),
}));
vi.mock("../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../lib/ipc")>()),
  ipc: ipcMock,
}));

import type { ChatMessage } from "../lib/ipc/bindings/ChatMessage";
import type { Friend } from "../lib/ipc/bindings/Friend";
import { useApp } from "./app";
import { totalUnread, useFriends } from "./friends";

const friend = (over: Partial<Friend> = {}): Friend => ({
  id: "f1",
  friendCode: "MEHBUR-AAAA",
  displayName: "Ali",
  status: "accepted",
  incoming: false,
  requestId: 1,
  unread: 0,
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

const profile = { id: "me", friendCode: "MEHBUR-MEME", displayName: "Ben" };

describe("friends store", () => {
  beforeEach(() => {
    useFriends.setState({
      enabled: null,
      profile: null,
      friends: [],
      chatWith: null,
      messages: {},
      offline: false,
    });
    useApp.setState({ notice: null });
    vi.clearAllMocks();
    ipcMock.friendsList.mockResolvedValue([]);
    ipcMock.chatMarkRead.mockResolvedValue(undefined);
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
});
