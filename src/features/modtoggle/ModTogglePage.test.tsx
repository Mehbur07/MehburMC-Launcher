import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  scanContent: vi.fn(),
  toggleInstanceFile: vi.fn(),
  setContentEnabled: vi.fn(),
  selectInstance: vi.fn(),
  contentIcon: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { InstalledItem } from "../../lib/ipc/bindings/InstalledItem";
import { useApp } from "../../stores/app";
import { useInstances } from "../../stores/instances";
import { useTasks } from "../../stores/tasks";
import { instance, task } from "../../test/fixtures";
import { ModTogglePage } from "./ModTogglePage";

const mod = (fileName: string, title: string | null, version: string | null = null) =>
  ({
    fileName,
    enabled: !fileName.endsWith(".disabled"),
    size: 1,
    projectId: null,
    slug: null,
    title,
    versionNumber: version,
    iconUrl: null,
    update: null,
  }) satisfies InstalledItem;

describe("ModTogglePage", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    ipcMock.scanContent.mockResolvedValue([
      mod("sodium-0.6.jar", "Sodium", "0.6.0"),
      mod("iris.jar.disabled", "Iris"),
      mod("mystery-1.0.jar", null),
    ]);
    ipcMock.toggleInstanceFile.mockResolvedValue({});
    ipcMock.setContentEnabled.mockResolvedValue(1);
    useInstances.setState({
      instances: [instance({ id: "i1", name: "Survival" })],
      selected: "i1",
    });
    useTasks.setState({ tasks: {} });
    useApp.setState({ view: "modToggle", notice: null });
  });

  it("lists mods sorted with names, versions and a counter", async () => {
    render(<ModTogglePage />);
    expect(await screen.findByText("2 / 3 on")).toBeInTheDocument();
    const names = screen
      .getAllByRole("listitem")
      .map((li) => li.querySelector(".font-semibold")?.textContent);
    expect(names).toEqual(["Iris", "mystery-1.0", "Sodium"]);
    expect(screen.getByText(/0\.6\.0 · sodium-0\.6\.jar/)).toBeInTheDocument();
    expect(ipcMock.scanContent).toHaveBeenCalledWith("i1", "mods", false);
  });

  it("toggles one mod and reloads", async () => {
    render(<ModTogglePage />);
    const sw = await screen.findByRole("switch", { name: "Toggle Iris" });
    expect(sw).toHaveAttribute("aria-checked", "false");
    await act(async () => fireEvent.click(sw));
    expect(ipcMock.toggleInstanceFile).toHaveBeenCalledWith("i1", "mods", "iris.jar.disabled");
    expect(ipcMock.scanContent).toHaveBeenCalledTimes(2);
  });

  it("filters and searches, and bulk actions apply to what is shown", async () => {
    render(<ModTogglePage />);
    await screen.findByText("Sodium");
    fireEvent.click(screen.getByRole("tab", { name: "Off" }));
    expect(screen.queryByText("Sodium")).toBeNull();
    expect(screen.getByText("Iris")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "All" }));

    fireEvent.change(screen.getByRole("textbox", { name: "Search mods…" }), {
      target: { value: "myst" },
    });
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
    await act(async () => fireEvent.click(screen.getByRole("button", { name: /Disable shown/ })));
    expect(ipcMock.setContentEnabled).toHaveBeenCalledWith(
      "i1",
      "mods",
      ["mystery-1.0.jar"],
      false,
    );

    fireEvent.change(screen.getByRole("textbox", { name: "Search mods…" }), {
      target: { value: "nothing-here" },
    });
    expect(screen.getByText("No matching mods.")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox", { name: "Search mods…" }), {
      target: { value: "" },
    });
    await act(async () => fireEvent.click(screen.getByRole("button", { name: /Enable all/ })));
    expect(ipcMock.setContentEnabled).toHaveBeenLastCalledWith(
      "i1",
      "mods",
      ["iris.jar.disabled", "mystery-1.0.jar", "sodium-0.6.jar"],
      true,
    );
  });

  it("locks everything while the game runs", async () => {
    useTasks.setState({ tasks: { t: task({ id: "t", instanceId: "i1", status: "playing" }) } });
    render(<ModTogglePage />);
    await screen.findByText("Sodium");
    expect(screen.getByText(/cannot be changed while the game is running/)).toBeInTheDocument();
    for (const sw of screen.getAllByRole("switch")) expect(sw).toBeDisabled();
    expect(screen.getByRole("button", { name: /Enable all/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Disable all/ })).toBeDisabled();
  });

  it("explains empty profiles and profiles without a loader", async () => {
    ipcMock.scanContent.mockResolvedValue([]);
    useInstances.setState({
      instances: [instance({ id: "v", name: "Vanilla", loader: { kind: "vanilla" } })],
      selected: "v",
    });
    render(<ModTogglePage />);
    expect(await screen.findByText("This profile has no mods.")).toBeInTheDocument();
    expect(screen.getByText(/no mod loader/)).toBeInTheDocument();
    const add = screen.getByRole("button", { name: /Add/ });
    fireEvent.click(add);
    await waitFor(() => expect(useApp.getState().view).toBe("browse"));
    expect(within(document.body).queryByRole("switch")).toBeNull();
  });
});
