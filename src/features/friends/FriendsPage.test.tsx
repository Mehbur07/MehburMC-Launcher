import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  friendsStatus: vi.fn(),
  friendsEnable: vi.fn(),
  friendsList: vi.fn(),
  friendRequest: vi.fn(),
  friendRespond: vi.fn(),
  chatMessages: vi.fn(),
  chatSend: vi.fn(),
  chatMarkRead: vi.fn(),
  myShares: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { Friend } from "../../lib/ipc/bindings/Friend";
import { useFriends } from "../../stores/friends";
import { useInstances } from "../../stores/instances";
import { FriendsPage } from "./FriendsPage";

const profile = { id: "me", friendCode: "MEHBUR-MEME", displayName: "Ben" };
const ali: Friend = {
  id: "f1",
  friendCode: "MEHBUR-AAAA",
  displayName: "Ali",
  status: "accepted",
  incoming: false,
  requestId: 1,
  unread: 2,
  avatar: null,
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
      offline: false,
    });
    useInstances.setState({ instances: [] });
    ipcMock.friendsList.mockResolvedValue([]);
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
});
