import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  listSkins: vi.fn(),
  listDefaultSkins: vi.fn(),
  addSkinBytes: vi.fn(),
  assignSkin: vi.fn(),
  listInstances: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));
// WebGL and canvas encoding are not available in jsdom.
vi.mock("./SkinViewer3D", () => ({ SkinViewer3D: () => <div data-testid="viewer" /> }));
vi.mock("./InGamePanel", () => ({ InGamePanel: () => null }));
vi.mock("./editor/canvas", () => ({
  toDataUri: () => "data:image/png;base64,UE5H",
  toCanvas: () => document.createElement("canvas"),
  fromDataUri: async () => ({ width: 64, height: 64, data: new Uint8ClampedArray(64 * 64 * 4) }),
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import { useAccounts } from "../../stores/accounts";
import { useSkins } from "../../stores/skins";
import { SkinsPage } from "./SkinsPage";

describe("SkinsPage", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    ipcMock.listSkins.mockResolvedValue({ skins: [], capes: [], assignments: {}, private: [] });
    ipcMock.listDefaultSkins.mockResolvedValue([
      { name: "Steve", model: "classic", dataUri: "data:image/png;base64,QQ", source: "1.21.4" },
    ]);
    ipcMock.addSkinBytes.mockResolvedValue("a".repeat(40));
    useSkins.setState({ skins: [], capes: [], assignments: {}, loaded: true });
    useAccounts.setState({
      accounts: [{ id: "acc", kind: "offline", name: "Mehbur", uuid: "u", addedAt: 1 }],
      selected: "acc",
    });
  });

  it("has no player-name fetch and offers presets and the editor", () => {
    render(<SkinsPage />);
    expect(screen.queryByPlaceholderText(/player/i)).toBeNull();
    expect(screen.getByRole("tab", { name: /My library/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("tab", { name: /Presets/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Design/ })).toBeInTheDocument();
  });

  // Role queries over 100+ cards are slow in jsdom.
  it("lists game defaults and the collection, and adds a preset", async () => {
    render(<SkinsPage />);
    fireEvent.click(screen.getByRole("tab", { name: /Presets/ }));
    expect(await screen.findByText(/installed 1\.21\.4/)).toBeInTheDocument();
    expect(screen.getByText("Steve (Classic)")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add Knight to the library" }));
    await waitFor(() =>
      expect(ipcMock.addSkinBytes).toHaveBeenCalledWith("skin", "Knight", "UE5H", "classic"),
    );
    // The MehburMC Library collection is listed here too (K75).
    expect(screen.getByText("Firefighter")).toBeInTheDocument();
    expect(screen.getByText("Elf Archer")).toBeInTheDocument();

    // Capes have no game defaults, only the collection.
    fireEvent.click(screen.getByRole("tab", { name: "Cape" }));
    expect(screen.getByText("Crescent & Star")).toBeInTheDocument();
    expect(screen.getByText("Dragon Scales")).toBeInTheDocument();
    expect(screen.queryByText("The game's default skins")).toBeNull();
  }, 20_000);

  it("searches the 100+ collection and shows everything on request", async () => {
    render(<SkinsPage />);
    fireEvent.click(screen.getByRole("tab", { name: /Presets/ }));
    // The catalogue starts folded: a later design is not drawn yet.
    expect(screen.queryByText("Viking")).toBeNull();
    fireEvent.change(screen.getByLabelText("Search the collection…"), {
      target: { value: "vik" },
    });
    expect(await screen.findByText("Viking")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Search the collection…"), {
      target: { value: "zzz" },
    });
    expect(screen.getByText("No matching designs")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Search the collection…"), { target: { value: "" } });
    fireEvent.click(screen.getByRole("button", { name: /Show all \(1\d\d\)/ }));
    expect(screen.getByText("Engineer")).toBeInTheDocument();
  }, 20_000);

  it("saves a design from the editor", async () => {
    render(<SkinsPage />);
    fireEvent.click(screen.getByRole("tab", { name: /Design/ }));
    expect(screen.getByLabelText("Drawing area")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "My design" } });
    const save = screen.getByRole("button", { name: /Save to library/ });
    await waitFor(() => expect(save).toBeEnabled());
    fireEvent.click(save);
    await waitFor(() =>
      expect(ipcMock.addSkinBytes).toHaveBeenCalledWith("skin", "My design", "UE5H", "classic"),
    );
    expect(await screen.findByRole("button", { name: /Saved/ })).toBeInTheDocument();
  });
});
