import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  listSkins: vi.fn(),
  listDefaultSkins: vi.fn(),
  addSkinBytes: vi.fn(),
  listInstances: vi.fn(),
  communityTextures: vi.fn(),
  shareTexture: vi.fn(),
  unshareTexture: vi.fn(),
  reportTexture: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));
vi.mock("./SkinViewer3D", () => ({ SkinViewer3D: () => <div data-testid="viewer" /> }));
vi.mock("./InGamePanel", () => ({ InGamePanel: () => null }));
vi.mock("./editor/canvas", () => ({
  toDataUri: () => "data:image/png;base64,UE5H",
  toCanvas: () => document.createElement("canvas"),
  fromDataUri: async () => ({ width: 64, height: 64, data: new Uint8ClampedArray(64 * 64 * 4) }),
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { SharedTexture } from "../../lib/ipc/bindings/SharedTexture";
import { useAccounts } from "../../stores/accounts";
import { useCommunity } from "../../stores/community";
import { useFriends } from "../../stores/friends";
import { useSkins } from "../../stores/skins";
import { SkinsPage } from "./SkinsPage";

const SHA = "a".repeat(40);

const shared = (over: Partial<SharedTexture>): SharedTexture => ({
  id: 1,
  kind: "skin",
  model: "classic",
  name: "Shadow Fox",
  author: "Alex",
  sha1: "b".repeat(40),
  visibility: "public",
  createdAt: "2026-10-06T10:00:00Z",
  mine: false,
  dataUri: "data:image/png;base64,QQ",
  ...over,
});

describe("Sharing skins", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    ipcMock.listDefaultSkins.mockResolvedValue([]);
    ipcMock.communityTextures.mockResolvedValue([
      shared({ id: 9, name: "Mine", sha1: SHA, mine: true, author: "Mehbur" }),
      shared({ id: 2, name: "Shadow Fox", visibility: "friends" }),
    ]);
    useCommunity.setState({ items: null, loading: false, error: null });
    useFriends.setState({ enabled: false });
    useSkins.setState({
      skins: [
        {
          id: SHA,
          name: "Mine",
          model: "classic",
          addedAt: 1,
          dataUri: "data:image/png;base64,QQ",
        },
        {
          id: "c".repeat(40),
          name: "Fresh",
          model: "slim",
          addedAt: 2,
          dataUri: "data:image/png;base64,QQ",
        },
      ],
      capes: [],
      assignments: {},
      loaded: true,
    });
    useAccounts.setState({
      accounts: [{ id: "acc", kind: "offline", name: "Mehbur", uuid: "u", addedAt: 1 }],
      selected: "acc",
    });
  });

  it("marks shared library cards and shares a new one publicly", async () => {
    ipcMock.shareTexture.mockResolvedValue(10);
    render(<SkinsPage />);
    expect(await screen.findByRole("img", { name: "Everyone" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Manage share" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Share" }));
    const dialog = await screen.findByRole("dialog", { name: "Share" });
    expect(dialog).toHaveTextContent('Your name "Mehbur" is shown as the sharer.');
    // Friends are off: friends-only cannot be chosen.
    expect(within(dialog).getByRole("radio", { name: /Friends only/ })).toBeDisabled();
    expect(within(dialog).getByRole("radio", { name: /Everyone/ })).toBeChecked();
    await act(async () => fireEvent.click(within(dialog).getByRole("button", { name: /^Share$/ })));
    expect(ipcMock.shareTexture).toHaveBeenCalledWith("skin", "c".repeat(40), "Fresh", "public");
    await waitFor(() => expect(ipcMock.communityTextures).toHaveBeenCalledTimes(2));
  });

  it("withdraws from the share dialog", async () => {
    ipcMock.unshareTexture.mockResolvedValue(undefined);
    render(<SkinsPage />);
    fireEvent.click(await screen.findByRole("button", { name: "Manage share" }));
    const dialog = await screen.findByRole("dialog");
    await act(async () =>
      fireEvent.click(within(dialog).getByRole("button", { name: /Withdraw/ })),
    );
    expect(ipcMock.unshareTexture).toHaveBeenCalledWith(9);
    await waitFor(() => expect(screen.queryByRole("img", { name: "Everyone" })).toBeNull());
  });

  it("shows backend refusals inside the dialog", async () => {
    ipcMock.shareTexture.mockRejectedValue({ code: "textures.quota", params: {}, detail: "" });
    render(<SkinsPage />);
    fireEvent.click(await screen.findByRole("button", { name: "Share" }));
    const dialog = await screen.findByRole("dialog");
    await act(async () => fireEvent.click(within(dialog).getByRole("button", { name: /^Share$/ })));
    expect(within(dialog).getByRole("alert")).toHaveTextContent("at most 30");
  });

  it("lists the community in Presets, adds and reports", async () => {
    ipcMock.addSkinBytes.mockResolvedValue("b".repeat(40));
    ipcMock.reportTexture.mockResolvedValue(undefined);
    render(<SkinsPage />);
    fireEvent.click(screen.getByRole("tab", { name: /Presets/ }));
    expect(await screen.findByText("Shared by Alex")).toBeInTheDocument();
    expect(screen.getByText("Your share")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Friends only" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Add Shadow Fox to the library" }));
    await waitFor(() =>
      expect(ipcMock.addSkinBytes).toHaveBeenCalledWith("skin", "Shadow Fox", "QQ", "classic"),
    );

    // Own shares cannot be reported, others can.
    const cards = screen.getAllByRole("button", { name: "Report" });
    expect(cards).toHaveLength(1);
    fireEvent.click(cards[0]!);
    const dialog = await screen.findByRole("dialog", { name: "Report: Shadow Fox" });
    fireEvent.click(within(dialog).getByRole("radio", { name: /no permission/ }));
    fireEvent.change(within(dialog).getByRole("textbox"), { target: { value: "copied" } });
    await act(async () => fireEvent.click(within(dialog).getByRole("button", { name: "Report" })));
    expect(ipcMock.reportTexture).toHaveBeenCalledWith(2, "stolen", "copied");
    expect(await screen.findByText(/your report was received/)).toBeInTheDocument();
    expect(screen.queryByText("Shared by Alex")).toBeNull();
  });

  it("shows a retry when the community cannot load", async () => {
    ipcMock.communityTextures.mockRejectedValue({
      code: "friends.unavailable",
      params: {},
      detail: "",
    });
    render(<SkinsPage />);
    fireEvent.click(screen.getByRole("tab", { name: /Presets/ }));
    expect(await screen.findByRole("button", { name: "Retry" })).toBeInTheDocument();
  });
});
