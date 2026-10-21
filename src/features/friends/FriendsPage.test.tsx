import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  groupsList: vi.fn(),
  friendsStatus: vi.fn(),
  friendsEnable: vi.fn(),
  friendsList: vi.fn(),
  friendsOnline: vi.fn(),
  friendRequest: vi.fn(),
  friendRespond: vi.fn(),
  chatMessages: vi.fn(),
  chatSend: vi.fn(),
  chatMarkRead: vi.fn(),
  myShares: vi.fn(),
  friendSetNickname: vi.fn(),
  groupCreate: vi.fn(),
  groupMessages: vi.fn(),
  groupMarkRead: vi.fn(),
  groupSend: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { ChatGroup } from "../../lib/ipc/bindings/ChatGroup";
import type { Friend } from "../../lib/ipc/bindings/Friend";
import { useFriends } from "../../stores/friends";
import { useInstances } from "../../stores/instances";
import { FriendsPage } from "./FriendsPage";

const profile = { id: "me", friendCode: "MEHBUR-MEME", displayName: "Ben" };
const ali: Friend = {
  id: "f1",
  friendCode: "MEHBUR-AAAA",
  nickname: null,
  displayName: "Ali",
  status: "accepted",
  incoming: false,
  requestId: 1,
  unread: 2,
  avatar: null,
  online: false,
};
const team: ChatGroup = {
  id: 5,
  name: "Ekip",
  owner: "me",
  isOwner: true,
  members: [
    { id: "me", displayName: "Ben", nickname: null, online: true },
    { id: "f1", displayName: "Ali", nickname: "Kanka", online: false },
  ],
  unread: 1,
  lastMessageAt: "2026-10-21T10:00:00Z",
};

describe("FriendsPage", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
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
    useInstances.setState({ instances: [] });
    ipcMock.groupsList.mockResolvedValue([]);
    ipcMock.groupMarkRead.mockResolvedValue(undefined);
    ipcMock.friendsList.mockResolvedValue([]);
    ipcMock.friendsOnline.mockResolvedValue(false);
    ipcMock.myShares.mockResolvedValue([]);
    ipcMock.chatMarkRead.mockResolvedValue(undefined);
  });

  it("asks for consent before creating an identity", async () => {
    ipcMock.friendsStatus.mockResolvedValue({ enabled: false, profile: null });
    ipcMock.friendsEnable.mockResolvedValue(profile);
    render(<FriendsPage />);
    const enable = await screen.findByRole("button", { name: /Agree and turn on/i });
    expect(ipcMock.friendsEnable).not.toHaveBeenCalled();
    fireEvent.click(enable);
    expect(await screen.findByText("MEHBUR-MEME")).toBeInTheDocument();
  });

  it("adds a friend by code and shows server errors inline", async () => {
    ipcMock.friendsStatus.mockResolvedValue({ enabled: true, profile });
    ipcMock.friendRequest.mockRejectedValueOnce({
      code: "friends.codeNotFound",
      params: {},
      detail: "",
    });
    render(<FriendsPage />);
    const input = await screen.findByRole("textbox", { name: /Add a friend by code/i });
    fireEvent.change(input, { target: { value: "mehbur-zzzz" } });
    expect(input).toHaveValue("MEHBUR-ZZZZ");
    fireEvent.click(screen.getByRole("button", { name: /Send request/ }));
    await waitFor(() => expect(ipcMock.friendRequest).toHaveBeenCalledWith("MEHBUR-ZZZZ"));
    expect(await screen.findByText(/friend code was not found/i)).toBeInTheDocument();
  });

  it("accepts an incoming request", async () => {
    ipcMock.friendsStatus.mockResolvedValue({ enabled: true, profile });
    ipcMock.friendsList.mockResolvedValue([
      { ...ali, status: "pending", incoming: true, requestId: 7 },
    ]);
    ipcMock.friendRespond.mockResolvedValue(undefined);
    render(<FriendsPage />);
    fireEvent.click(await screen.findByRole("button", { name: /Accept/ }));
    await waitFor(() => expect(ipcMock.friendRespond).toHaveBeenCalledWith(7, true));
  });

  it("opens a chat and sends with Enter", async () => {
    ipcMock.friendsStatus.mockResolvedValue({ enabled: true, profile });
    ipcMock.friendsList.mockResolvedValue([ali]);
    ipcMock.chatMessages.mockResolvedValue([
      {
        id: 1,
        sender: "f1",
        recipient: "me",
        body: "selam",
        createdAt: "2026-10-05T10:00:00Z",
        readAt: null,
      },
    ]);
    ipcMock.chatSend.mockResolvedValue({
      id: 2,
      sender: "me",
      recipient: "f1",
      body: "naber",
      createdAt: "2026-10-05T10:01:00Z",
      readAt: null,
    });
    render(<FriendsPage />);
    fireEvent.click(await screen.findByRole("button", { name: /Ali/ }));
    expect(await screen.findByText("selam")).toBeInTheDocument();

    const box = screen.getByRole("textbox", { name: /Write a message/i });
    fireEvent.change(box, { target: { value: "naber" } });
    fireEvent.keyDown(box, { key: "Enter" });
    await waitFor(() => expect(ipcMock.chatSend).toHaveBeenCalledWith("f1", "naber"));
    expect(await screen.findByText("naber")).toBeInTheDocument();
    expect(box).toHaveValue("");
  });

  it("lists online friends first with the online dot", async () => {
    ipcMock.friendsStatus.mockResolvedValue({ enabled: true, profile, online: true });
    ipcMock.friendsList.mockResolvedValue([
      ali,
      { ...ali, id: "f2", displayName: "Zeynep", friendCode: "MEHBUR-ZZZZ", online: true },
    ]);
    render(<FriendsPage />);
    await screen.findByRole("button", { name: /Zeynep/ });
    const names = screen
      .getAllByRole("button")
      .map((b) => b.textContent ?? "")
      .filter((t) => t.includes("Ali") || t.includes("Zeynep"));
    expect(names[0]).toContain("Zeynep");
    expect(screen.getAllByRole("img", { name: "Online" })).toHaveLength(1);
  });

  it("sets a private nickname and keeps the real name visible", async () => {
    ipcMock.friendsStatus.mockResolvedValue({ enabled: true, profile });
    ipcMock.friendsList.mockResolvedValue([ali]);
    ipcMock.chatMessages.mockResolvedValue([]);
    ipcMock.friendSetNickname.mockResolvedValue(undefined);
    render(<FriendsPage />);
    fireEvent.click(await screen.findByRole("button", { name: /Ali/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Set a nickname/ }));
    fireEvent.change(screen.getByRole("textbox", { name: /Nickname/ }), {
      target: { value: "Kanka" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(ipcMock.friendSetNickname).toHaveBeenCalledWith("f1", "Kanka"));
    expect(await screen.findAllByText("Kanka")).not.toHaveLength(0);
    expect(screen.getByText("(Ali)")).toBeInTheDocument();
  });

  it("creates a group from picked friends", async () => {
    ipcMock.friendsStatus.mockResolvedValue({ enabled: true, profile });
    ipcMock.friendsList.mockResolvedValue([ali]);
    ipcMock.groupCreate.mockResolvedValue(5);
    ipcMock.groupMessages.mockResolvedValue([]);
    render(<FriendsPage />);
    expect(await screen.findByText(/No groups yet/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Create group" }));
    const create = screen.getByRole("button", { name: "Create" });
    expect(create).toBeDisabled();
    fireEvent.change(screen.getByRole("textbox", { name: /Group name/ }), {
      target: { value: "Ekip" },
    });
    fireEvent.click(screen.getByRole("checkbox", { name: "Ali" }));
    ipcMock.groupsList.mockResolvedValue([team]);
    fireEvent.click(create);
    await waitFor(() => expect(ipcMock.groupCreate).toHaveBeenCalledWith("Ekip", ["f1"]));
    expect(await screen.findByRole("log", { name: /Ekip group chat/ })).toBeInTheDocument();
  });

  it("shows group messages under member nicknames and sends to the group", async () => {
    ipcMock.friendsStatus.mockResolvedValue({ enabled: true, profile });
    ipcMock.friendsList.mockResolvedValue([{ ...ali, nickname: "Kanka" }]);
    ipcMock.groupsList.mockResolvedValue([team]);
    ipcMock.groupMessages.mockResolvedValue([
      { id: 1, groupId: 5, sender: "f1", body: "hey", createdAt: "2026-10-21T10:00:00Z" },
      { id: 2, groupId: 5, sender: "gone", body: "eski", createdAt: "2026-10-21T10:00:01Z" },
    ]);
    ipcMock.groupSend.mockResolvedValue({
      id: 3,
      groupId: 5,
      sender: "me",
      body: "selam ekip",
      createdAt: "2026-10-21T10:01:00Z",
    });
    render(<FriendsPage />);
    fireEvent.click(await screen.findByRole("button", { name: /Ekip/ }));
    expect(await screen.findByText("hey")).toBeInTheDocument();
    expect(screen.getAllByText("Kanka").length).toBeGreaterThan(0);
    expect(screen.getByText("Former member")).toBeInTheDocument();
    await waitFor(() => expect(ipcMock.groupMarkRead).toHaveBeenCalledWith(5));

    expect(screen.getByRole("log", { name: /Ekip group chat/ })).toBeInTheDocument();
    const box = screen.getByRole("textbox", { name: /Write a message/i });
    fireEvent.change(box, { target: { value: "selam ekip" } });
    fireEvent.keyDown(box, { key: "Enter" });
    await waitFor(() => expect(ipcMock.groupSend).toHaveBeenCalledWith(5, "selam ekip"));
    expect(await screen.findByText("selam ekip")).toBeInTheDocument();
  });
});
