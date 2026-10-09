import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  listSkins: vi.fn(),
  listDefaultSkins: vi.fn(),
  addSkinBytes: vi.fn(),
  listInstances: vi.fn(),
  communityTextures: vi.fn(),
  skinmcBrowse: vi.fn(),
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
import type { SkinMcSkin } from "../../lib/ipc/bindings/SkinMcSkin";
import { useAccounts } from "../../stores/accounts";
import { useSkinMc } from "../../stores/skinmc";
import { useSkins } from "../../stores/skins";
import { SkinsPage } from "./SkinsPage";

const skin = (id: string, author: string): SkinMcSkin => ({
  id,
  author,
  model: "slim",
  dataUri: "data:image/png;base64,U0tJTg==",
});

async function openSkinMc() {
  render(<SkinsPage />);
  fireEvent.click(screen.getByRole("tab", { name: /Presets/ }));
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "SkinMC" })));
}

describe("SkinMC gallery", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    ipcMock.listSkins.mockResolvedValue({ skins: [], capes: [], assignments: {}, private: [] });
    ipcMock.listDefaultSkins.mockResolvedValue([]);
    ipcMock.communityTextures.mockResolvedValue([]);
    ipcMock.addSkinBytes.mockResolvedValue("a".repeat(40));
    useSkins.setState({ skins: [], capes: [], assignments: {}, loaded: true });
    useAccounts.setState({
      accounts: [{ id: "acc", kind: "offline", name: "Mehbur", uuid: "u", addedAt: 1 }],
      selected: "acc",
    });
    useSkinMc.setState({
      items: [],
      next: null,
      loading: false,
      error: null,
      tag: "",
      sort: "latest",
    });
  });

  it("shows the newest skins and adds one to the library", async () => {
    ipcMock.skinmcBrowse.mockResolvedValue({ skins: [skin("s1", "thekmi")], next: "N1" });
    await openSkinMc();
    expect(ipcMock.skinmcBrowse).toHaveBeenCalledWith("latest", null, null);
    // The MehburMC sections are hidden while SkinMC is shown.
    expect(screen.queryByText("MehburMC collection")).toBeNull();
    fireEvent.click(
      await screen.findByRole("button", { name: "Add thekmi (SkinMC) to the library" }),
    );
    await waitFor(() =>
      expect(ipcMock.addSkinBytes).toHaveBeenCalledWith(
        "skin",
        "thekmi (SkinMC)",
        "U0tJTg==",
        "slim",
      ),
    );
  });

  it("searches a tag, loads more and switches the sort order", async () => {
    ipcMock.skinmcBrowse
      .mockResolvedValueOnce({ skins: [skin("s1", "a")], next: "N1" })
      .mockResolvedValueOnce({ skins: [skin("c1", "cat lover")], next: "N2" })
      .mockResolvedValueOnce({ skins: [skin("c1", "cat lover"), skin("c2", "kitty")] })
      .mockResolvedValueOnce({ skins: [skin("w1", "weekly")] });
    await openSkinMc();
    await screen.findByText("a (SkinMC)");

    fireEvent.change(screen.getByLabelText("Tag"), { target: { value: "cat" } });
    await act(async () => fireEvent.click(screen.getByRole("button", { name: /^Search$/ })));
    expect(ipcMock.skinmcBrowse).toHaveBeenLastCalledWith("latest", "cat", null);
    expect(screen.getByText('Skins tagged "cat"')).toBeInTheDocument();

    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Load more" })));
    expect(ipcMock.skinmcBrowse).toHaveBeenLastCalledWith("latest", "cat", "N2");
    // The repeated skin is shown once; the last page has no "Load more".
    expect(screen.getAllByText("cat lover (SkinMC)")).toHaveLength(1);
    expect(screen.getByText("kitty (SkinMC)")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();

    const sorts = screen.getByRole("button", { name: "This week" }).parentElement!;
    await act(async () =>
      fireEvent.click(within(sorts).getByRole("button", { name: "This week" })),
    );
    expect(ipcMock.skinmcBrowse).toHaveBeenLastCalledWith("week", null, null);
    expect(await screen.findByText("weekly (SkinMC)")).toBeInTheDocument();
  });

  it("explains a failure and retries", async () => {
    ipcMock.skinmcBrowse
      .mockRejectedValueOnce({ code: "net.unreachable", params: {}, detail: "offline" })
      .mockResolvedValueOnce({ skins: [skin("s1", "back")] });
    await openSkinMc();
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    await act(async () => fireEvent.click(screen.getByRole("button", { name: /Retry/ })));
    expect(await screen.findByText("back (SkinMC)")).toBeInTheDocument();
  });
});
