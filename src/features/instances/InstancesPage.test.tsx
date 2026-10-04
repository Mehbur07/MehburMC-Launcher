import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  pickPath: vi.fn(),
  importModpack: vi.fn(),
  importInstance: vi.fn(),
  exportInstance: vi.fn(),
  listInstances: vi.fn(),
  selectInstance: vi.fn(async () => {}),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import { useApp } from "../../stores/app";
import { useInstances } from "../../stores/instances";
import { useTasks } from "../../stores/tasks";
import { instance } from "../../test/fixtures";
import { InstancesPage } from "./InstancesPage";

describe("InstancesPage", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    useTasks.setState({ tasks: {} });
    useApp.setState({ view: "instances", wizardOpen: false, notice: null });
    const list = [
      instance(),
      instance({ id: "pack-2", name: "Second", loader: { kind: "vanilla" } }),
    ];
    useInstances.setState({ instances: list, selected: "pack-1", loaded: true });
    ipcMock.listInstances.mockResolvedValue({ instances: list, selected: "pack-1" });
  });

  it("lists instances and opens the detail view on double click", () => {
    render(<InstancesPage />);
    expect(screen.getByText("My Pack")).toBeInTheDocument();
    expect(screen.getByText("Second")).toBeInTheDocument();
    fireEvent.doubleClick(screen.getByText("Second"));
    expect(useApp.getState()).toMatchObject({ view: "instance", detailId: "pack-2" });
  });

  it("imports a modpack through a Rust-side file dialog", async () => {
    ipcMock.pickPath.mockResolvedValue("C:\\Downloads\\pack.mrpack");
    ipcMock.importModpack.mockResolvedValue({
      instance: instance({ id: "new" }),
      installed: 3,
      skipped: [],
      blocked: [],
    });
    render(<InstancesPage />);
    fireEvent.click(screen.getByRole("button", { name: /Import modpack/ }));
    await vi.waitFor(() => expect(ipcMock.importModpack).toHaveBeenCalledWith());
    expect(ipcMock.pickPath).toHaveBeenCalledWith("modpack");
  });

  it("does nothing when the dialog is cancelled", async () => {
    ipcMock.pickPath.mockResolvedValue(null);
    render(<InstancesPage />);
    fireEvent.click(screen.getByRole("button", { name: /^Import$/ }));
    await vi.waitFor(() => expect(ipcMock.pickPath).toHaveBeenCalledWith("instanceArchive"));
    expect(ipcMock.importInstance).not.toHaveBeenCalled();
  });

  it("opens the creation wizard", () => {
    render(<InstancesPage />);
    fireEvent.click(screen.getByRole("button", { name: /Create instance/ }));
    expect(useApp.getState().wizardOpen).toBe(true);
  });
});
