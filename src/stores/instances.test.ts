import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  listInstances: vi.fn(),
  listVersions: vi.fn(),
  selectInstance: vi.fn(),
  createInstance: vi.fn(),
  updateInstance: vi.fn(),
  deleteInstance: vi.fn(),
  reorderInstances: vi.fn(),
}));
vi.mock("../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../lib/ipc")>()),
  ipc: ipcMock,
}));

import { instance } from "../test/fixtures";
import { useApp } from "./app";
import { selectedInstance, useInstances } from "./instances";

const a = instance({ id: "a", name: "A" });
const b = instance({ id: "b", name: "B" });

describe("instances store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useApp.setState({ notice: null });
    useInstances.setState({ loaded: false, instances: [], selected: null, versions: null });
    ipcMock.listInstances.mockResolvedValue({ instances: [a, b], selected: "b" });
  });

  it("loads, selects and resolves the selected instance", async () => {
    await useInstances.getState().load();
    expect(selectedInstance(useInstances.getState())?.name).toBe("B");
    await useInstances.getState().select("a");
    expect(ipcMock.selectInstance).toHaveBeenCalledWith("a");
    expect(selectedInstance(useInstances.getState())?.name).toBe("A");
  });

  it("caches the version list", async () => {
    ipcMock.listVersions.mockResolvedValue([{ id: "1.20.1" }]);
    await useInstances.getState().loadVersions();
    await useInstances.getState().loadVersions();
    expect(ipcMock.listVersions).toHaveBeenCalledTimes(1);
  });

  it("creates and selects; update patches locally; errors become notices", async () => {
    ipcMock.createInstance.mockResolvedValue(a);
    expect(await useInstances.getState().create({ name: "A", mcVersion: "1.20.1" })).toEqual(a);
    expect(useInstances.getState().selected).toBe("a");

    ipcMock.updateInstance.mockResolvedValue({ ...a, name: "A2" });
    await useInstances.getState().update("a", { name: "A2" });
    expect(useInstances.getState().instances.find((i) => i.id === "a")?.name).toBe("A2");

    ipcMock.updateInstance.mockRejectedValue({
      code: "instance.invalid",
      params: {},
      detail: "javaPath",
    });
    expect(await useInstances.getState().update("a", { javaPath: "C:\\cmd.exe" })).toBeNull();
    expect(useApp.getState().notice?.code).toBe("instance.invalid");
  });

  it("reorders optimistically and reloads when the backend refuses", async () => {
    await useInstances.getState().load();
    ipcMock.reorderInstances.mockRejectedValue(new Error("disk"));
    const done = useInstances.getState().reorder(["b", "a"]);
    expect(useInstances.getState().instances.map((i) => i.id)).toEqual(["b", "a"]);
    await done;
    expect(useInstances.getState().instances.map((i) => i.id)).toEqual(["a", "b"]);
    expect(useApp.getState().notice?.detail).toBe("disk");
  });

  it("removes and reports success", async () => {
    ipcMock.deleteInstance.mockResolvedValue({ instances: [b], selected: "b" });
    expect(await useInstances.getState().remove("a")).toBe(true);
    expect(useInstances.getState().instances).toEqual([b]);
  });
});
