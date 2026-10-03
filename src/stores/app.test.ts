import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Bootstrap } from "../lib/ipc/bindings/Bootstrap";
import type { Settings } from "../lib/ipc/bindings/Settings";

const ipcMock = vi.hoisted(() => ({
  getBootstrap: vi.fn(),
  saveSettings: vi.fn(),
  openDataDir: vi.fn(),
}));
vi.mock("../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../lib/ipc")>()),
  ipc: ipcMock,
}));

import { useApp } from "./app";

const settings: Settings = {
  schemaVersion: 1,
  language: "system",
  accent: "cyan",
  backgroundEffects: true,
  debugLogging: false,
  downloadConcurrency: 8,
  defaultMemoryMb: 4096,
  launchBehavior: "minimize",
  auth: {},
};

const boot: Bootstrap = {
  appName: "MehburMC Launcher",
  version: "0.1.0",
  paths: null,
  settings,
  startupError: null,
  msaConfigured: false,
};

describe("app store", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    useApp.setState({ status: "loading", boot: null, settings: null, notice: null, fatal: null });
  });

  it("enters fatal state when the backend reports a startup error", async () => {
    const err = { code: "paths.notWritable", params: { path: "X" }, detail: "d" };
    ipcMock.getBootstrap.mockResolvedValue({ ...boot, startupError: err });
    await useApp.getState().load();
    expect(useApp.getState().status).toBe("fatal");
    expect(useApp.getState().fatal).toEqual(err);
  });

  it("rolls back settings when saving fails", async () => {
    ipcMock.getBootstrap.mockResolvedValue(boot);
    await useApp.getState().load();
    ipcMock.saveSettings.mockRejectedValue({ code: "io.diskFull", params: {}, detail: "full" });

    await useApp.getState().updateSettings({ accent: "purple" });

    expect(useApp.getState().settings?.accent).toBe("cyan");
    expect(useApp.getState().notice?.code).toBe("io.diskFull");
  });

  it("keeps saved settings on success", async () => {
    ipcMock.getBootstrap.mockResolvedValue(boot);
    await useApp.getState().load();
    ipcMock.saveSettings.mockImplementation(async (s: Settings) => s);

    await useApp.getState().updateSettings({ accent: "green" });

    expect(useApp.getState().settings?.accent).toBe("green");
    expect(ipcMock.saveSettings).toHaveBeenCalledWith({ ...settings, accent: "green" });
  });
});
