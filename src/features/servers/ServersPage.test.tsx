import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  listFavoriteServers: vi.fn(),
  addFavoriteServer: vi.fn(),
  removeFavoriteServer: vi.fn(),
  listGameServers: vi.fn(),
  addGameServer: vi.fn(),
  removeGameServer: vi.fn(),
  pingServer: vi.fn(),
  joinServer: vi.fn(),
  selectInstance: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { ServerStatus } from "../../lib/ipc/bindings/ServerStatus";
import { useAccounts } from "../../stores/accounts";
import { useApp } from "../../stores/app";
import { useInstances } from "../../stores/instances";
import { addressKey, useServers } from "../../stores/servers";
import { useTasks } from "../../stores/tasks";
import { instance, task } from "../../test/fixtures";
import { ServersPage } from "./ServersPage";

const status = (over: Partial<ServerStatus> = {}): ServerStatus => ({
  version: "Paper 1.21.4",
  protocol: 769,
  playersOnline: 12,
  playersMax: 100,
  playersSample: [],
  motd: [
    {
      text: "Merhaba",
      color: "#55ffff",
      bold: true,
      italic: false,
      underlined: false,
      strikethrough: false,
    },
  ],
  icon: null,
  latencyMs: 42,
  ...over,
});

const fav = { id: "f1", name: "Hypixel", address: "mc.hypixel.net", addedAt: 1 };
const gameEntry = { index: 0, name: "Local", address: "localhost:25570", icon: null };

describe("ServersPage", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    ipcMock.listFavoriteServers.mockResolvedValue([fav]);
    ipcMock.listGameServers.mockResolvedValue([gameEntry]);
    ipcMock.pingServer.mockImplementation(async (a: string) => {
      if (a === "localhost:25570") throw { code: "server.unreachable", params: {}, detail: "" };
      return status();
    });
    useServers.setState({ favorites: [], game: {}, pings: {} });
    useInstances.setState({
      instances: [instance({ id: "i1", name: "Survival" })],
      selected: "i1",
    });
    useTasks.setState({ tasks: {} });
    useApp.setState({ view: "servers", notice: null });
  });

  it("lists favourites and the game list with live status", async () => {
    render(<ServersPage />);
    expect(await screen.findByText("Hypixel")).toBeInTheDocument();
    expect(await screen.findByText("Local")).toBeInTheDocument();
    expect(await screen.findByText("12 / 100 players")).toBeInTheDocument();
    expect(screen.getByText("42 ms")).toBeInTheDocument();
    const motd = screen.getByText("Merhaba");
    expect(motd).toHaveStyle({ color: "#55ffff" });
    expect(motd).toHaveClass("font-bold");
    expect(await screen.findByText("Offline")).toBeInTheDocument();
    expect(ipcMock.listGameServers).toHaveBeenCalledWith("i1");
    expect(ipcMock.pingServer).toHaveBeenCalledTimes(2);
  });

  it("joins with the selected profile and goes home", async () => {
    ipcMock.joinServer.mockResolvedValue("task-1");
    render(<ServersPage />);
    const row = (await screen.findByText("Hypixel")).closest("li")!;
    fireEvent.click(within(row).getByRole("button", { name: /Join/ }));
    await waitFor(() => expect(ipcMock.joinServer).toHaveBeenCalledWith("i1", "mc.hypixel.net"));
    await waitFor(() => expect(useApp.getState().view).toBe("home"));
  });

  it("disables joining while the selected account plays the profile", async () => {
    useAccounts.setState({
      accounts: [{ id: "a", kind: "offline", name: "Steve", uuid: "u", addedAt: 1 }],
      selected: "a",
    });
    useTasks.setState({
      tasks: { t: task({ id: "t", instanceId: "i1", status: "playing", account: "Steve" }) },
    });
    render(<ServersPage />);
    const row = (await screen.findByText("Hypixel")).closest("li")!;
    expect(within(row).getByRole("button", { name: /Game running/ })).toBeDisabled();
    expect(screen.getByText(/cannot be changed while the game is running/)).toBeInTheDocument();
    // No delete button for the game list while running.
    const local = (await screen.findByText("Local")).closest("li")!;
    expect(within(local).queryByRole("button", { name: "Remove" })).toBeNull();
  });

  it("lets another account join while an AFK account plays the profile", async () => {
    useAccounts.setState({
      accounts: [{ id: "a", kind: "offline", name: "Steve", uuid: "u", addedAt: 1 }],
      selected: "a",
    });
    useTasks.setState({
      tasks: { t: task({ id: "t", instanceId: "i1", status: "playing", account: "AfkBot" }) },
    });
    ipcMock.joinServer.mockResolvedValue("task-2");
    render(<ServersPage />);
    const row = (await screen.findByText("Hypixel")).closest("li")!;
    fireEvent.click(within(row).getByRole("button", { name: /Join/ }));
    await waitFor(() => expect(ipcMock.joinServer).toHaveBeenCalledWith("i1", "mc.hypixel.net"));
    // The game's own server list stays locked while any game runs.
    expect(screen.getByText(/cannot be changed while the game is running/)).toBeInTheDocument();
  });

  it("stars a game-list server into favourites", async () => {
    ipcMock.addFavoriteServer.mockResolvedValue([
      fav,
      { id: "f2", name: "Local", address: "localhost:25570", addedAt: 2 },
    ]);
    render(<ServersPage />);
    const local = (await screen.findByText("Local")).closest("li")!;
    fireEvent.click(within(local).getByRole("button", { name: "Add to favourites" }));
    await waitFor(() =>
      expect(ipcMock.addFavoriteServer).toHaveBeenCalledWith("Local", "localhost:25570"),
    );
    expect(await screen.findAllByText("Local")).toHaveLength(2);
  });

  it("asks before deleting from the game list", async () => {
    ipcMock.removeGameServer.mockResolvedValue([]);
    render(<ServersPage />);
    const local = (await screen.findByText("Local")).closest("li")!;
    fireEvent.click(within(local).getByRole("button", { name: "Remove" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete from the game list?" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
    await waitFor(() =>
      expect(ipcMock.removeGameServer).toHaveBeenCalledWith("i1", 0, "localhost:25570"),
    );
    expect(await screen.findByText("This profile's in-game list is empty.")).toBeInTheDocument();
  });

  it("adds a server and shows backend errors inline", async () => {
    render(<ServersPage />);
    await screen.findByText("Hypixel");
    fireEvent.click(screen.getByRole("button", { name: "Add server" }));
    const dialog = await screen.findByRole("dialog", { name: "Add server" });
    const address = within(dialog).getByPlaceholderText(/play.example.com/);
    const submit = within(dialog).getByRole("button", { name: "Add server" });
    expect(submit).toBeDisabled();

    ipcMock.addGameServer.mockRejectedValueOnce({
      code: "server.addressInvalid",
      params: { address: "bad host" },
      detail: "",
    });
    fireEvent.change(address, { target: { value: "bad host" } });
    fireEvent.click(within(dialog).getByLabelText("Game list (Survival)"));
    fireEvent.click(submit);
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "Invalid server address: bad host",
    );

    ipcMock.addGameServer.mockResolvedValueOnce([
      gameEntry,
      { index: 1, name: "New", address: "new.example", icon: null },
    ]);
    fireEvent.change(address, { target: { value: "new.example" } });
    await act(async () => fireEvent.click(submit));
    expect(ipcMock.addGameServer).toHaveBeenLastCalledWith("i1", "", "new.example");
    expect(await screen.findByText("New")).toBeInTheDocument();
  });
});

describe("addressKey", () => {
  it("treats the default port and case as the same server", () => {
    expect(addressKey(" MC.Hypixel.net:25565 ")).toBe("mc.hypixel.net");
    expect(addressKey("a.b:25570")).toBe("a.b:25570");
  });
});
