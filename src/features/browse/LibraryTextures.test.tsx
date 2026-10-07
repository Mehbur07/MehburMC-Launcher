import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  libraryMods: vi.fn(),
  listSkins: vi.fn(),
  addSkinBytes: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));
vi.mock("../skins/SkinViewer3D", () => ({
  SkinViewer3D: (p: { skin: string | null; cape: string | null }) => (
    <div data-testid="viewer" data-skin={p.skin ?? ""} data-cape={p.cape ?? ""} />
  ),
}));
vi.mock("../skins/editor/canvas", () => ({
  toDataUri: () => "data:image/png;base64,UE5H",
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import { LibraryHub } from "./LibraryTextures";

describe("MehburMC Library skins and capes", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    ipcMock.libraryMods.mockResolvedValue([]);
    ipcMock.addSkinBytes.mockResolvedValue("f".repeat(40));
    ipcMock.listSkins.mockResolvedValue({ skins: [], capes: [], assignments: {}, private: [] });
  });

  it("starts on mods and lists the 20 skins and 15 capes", async () => {
    render(<LibraryHub inst={null} gameBusy={false} />);
    expect(ipcMock.libraryMods).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Skins" }));
    expect(await screen.findAllByRole("button", { name: /^Add .* to my library$/ })).toHaveLength(
      20,
    );
    expect(screen.getByText("Firefighter")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Capes" }));
    expect(screen.getAllByRole("button", { name: /^Add .* to my library$/ })).toHaveLength(15);
    expect(screen.getByText("Dragon Scales")).toBeInTheDocument();
  });

  it("adds a skin to the library once", async () => {
    render(<LibraryHub inst={null} gameBusy={false} />);
    fireEvent.click(screen.getByRole("button", { name: "Skins" }));
    const add = await screen.findByRole("button", { name: "Add Chef to my library" });
    await act(async () => fireEvent.click(add));
    expect(ipcMock.addSkinBytes).toHaveBeenCalledWith("skin", "Chef", "UE5H", "classic");
    expect(add).toBeDisabled();
    expect(add).toHaveTextContent("Added");
  });

  it("previews a cape in 3D on a plain figure and adds it from there", async () => {
    render(<LibraryHub inst={null} gameBusy={false} />);
    fireEvent.click(screen.getByRole("button", { name: "Capes" }));
    fireEvent.click(await screen.findByRole("button", { name: "Preview Full Moon" }));
    const dialog = await screen.findByRole("dialog", { name: "Full Moon" });
    const viewer = within(dialog).getByTestId("viewer");
    expect(viewer.dataset.cape).toContain("data:image/png");
    expect(viewer.dataset.skin).toContain("data:image/png");
    await act(async () =>
      fireEvent.click(within(dialog).getByRole("button", { name: "Add Full Moon to my library" })),
    );
    expect(ipcMock.addSkinBytes).toHaveBeenCalledWith("cape", "Full Moon", "UE5H", "classic");
  });
});
