import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  searchModrinth: vi.fn(),
  scanContent: vi.fn(),
  deleteInstanceFile: vi.fn(),
  installContent: vi.fn(),
  listInstances: vi.fn(),
  libraryMods: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { InstalledItem } from "../../lib/ipc/bindings/InstalledItem";
import type { SearchHit } from "../../lib/ipc/bindings/SearchHit";
import { useApp } from "../../stores/app";
import { useInstances } from "../../stores/instances";
import { useTasks } from "../../stores/tasks";
import { instance, task } from "../../test/fixtures";
import { BrowsePage } from "./BrowsePage";

const hit = (projectId: string, title: string): SearchHit => ({
  projectId,
  projectType: "mod",
  slug: title.toLowerCase(),
  title,
  description: "",
  author: "someone",
  iconUrl: null,
  downloads: 10,
  dateModified: "",
  categories: [],
});

const file = (fileName: string, projectId: string | null, enabled = true): InstalledItem => ({
  fileName,
  enabled,
  size: 1,
  projectId,
  slug: null,
  title: null,
  versionNumber: null,
  iconUrl: null,
  update: null,
});

describe("BrowsePage installed content", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    ipcMock.searchModrinth.mockResolvedValue({
      hits: [hit("sodium", "Sodium"), hit("iris", "Iris")],
      totalHits: 2,
      offset: 0,
    });
    ipcMock.scanContent.mockResolvedValue([
      file("sodium-0.6.jar", "sodium"),
      file("sodium-old.jar.disabled", "sodium", false),
      file("unknown.jar", null),
    ]);
    ipcMock.deleteInstanceFile.mockResolvedValue(undefined);
    useInstances.setState({
      instances: [instance({ id: "i1", name: "Survival" })],
      selected: "i1",
    });
    useTasks.setState({ tasks: {} });
    useApp.setState({ view: "browse", browseType: "mod", browseTarget: null, notice: null });
  });

  const row = async (title: string) => (await screen.findByText(title)).closest("li")!;

  it("marks installed projects and deletes all their files after confirmation", async () => {
    render(<BrowsePage />);
    const sodium = await row("Sodium");
    const menuButton = await within(sodium).findByRole("button", { name: /Installed \(off\)/ });
    expect(ipcMock.scanContent).toHaveBeenCalledWith("i1", "mods", false);
    // Not installed → normal install button.
    expect(within(await row("Iris")).getByRole("button", { name: /Install/ })).toBeEnabled();

    fireEvent.click(menuButton);
    fireEvent.click(within(sodium).getByRole("menuitem", { name: "Delete" }));
    const dialog = await screen.findByRole("dialog", {
      name: "Are you sure you want to delete it?",
    });
    expect(dialog).toHaveTextContent("sodium-old.jar.disabled");

    // Cancel keeps everything.
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(ipcMock.deleteInstanceFile).not.toHaveBeenCalled();

    fireEvent.click(within(sodium).getByRole("button", { name: /Installed/ }));
    fireEvent.click(within(sodium).getByRole("menuitem", { name: "Delete" }));
    ipcMock.scanContent.mockResolvedValue([file("unknown.jar", null)]);
    const again = await screen.findByRole("dialog");
    await act(async () => fireEvent.click(within(again).getByRole("button", { name: "Delete" })));

    expect(ipcMock.deleteInstanceFile).toHaveBeenCalledWith("i1", "mods", "sodium-0.6.jar");
    expect(ipcMock.deleteInstanceFile).toHaveBeenCalledWith(
      "i1",
      "mods",
      "sodium-old.jar.disabled",
    );
    await waitFor(() =>
      expect(within(sodium).getByRole("button", { name: /Install/ })).toBeInTheDocument(),
    );
  });

  it("locks deleting while the game runs", async () => {
    useTasks.setState({ tasks: { t: task({ id: "t", instanceId: "i1", status: "playing" }) } });
    render(<BrowsePage />);
    const sodium = await row("Sodium");
    fireEvent.click(await within(sodium).findByRole("button", { name: /Installed/ }));
    expect(within(sodium).getByRole("menuitem", { name: "Delete" })).toBeDisabled();
  });

  it("rescans after installing so the result becomes Installed", async () => {
    ipcMock.installContent.mockResolvedValue({ installed: [], skipped: [] });
    render(<BrowsePage />);
    const iris = await row("Iris");
    ipcMock.scanContent.mockResolvedValue([file("iris.jar", "iris")]);
    await act(async () => fireEvent.click(within(iris).getByRole("button", { name: /Install/ })));
    expect(await within(iris).findByRole("button", { name: /^Installed/ })).toBeInTheDocument();
    expect(ipcMock.scanContent).toHaveBeenCalledTimes(2);
  });

  it("does not scan on the modpack tab", async () => {
    useApp.setState({ browseType: "modpack" });
    render(<BrowsePage />);
    await row("Sodium");
    expect(ipcMock.scanContent).not.toHaveBeenCalled();
  });

  it("shows the MehburMC Library tab without searching Modrinth", async () => {
    ipcMock.libraryMods.mockResolvedValue([]);
    render(<BrowsePage />);
    await row("Sodium");
    ipcMock.searchModrinth.mockClear();
    fireEvent.click(screen.getByRole("tab", { name: "MehburMC Library" }));
    expect(await screen.findByText("No approved mods yet")).toBeInTheDocument();
    expect(screen.queryByText("Sodium")).toBeNull();
    expect(ipcMock.searchModrinth).not.toHaveBeenCalled();
  });
});
