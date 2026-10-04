import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  saveSettings: vi.fn(async (s: unknown) => s),
  checkAppUpdate: vi.fn(),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { Bootstrap } from "../../lib/ipc/bindings/Bootstrap";
import type { Settings } from "../../lib/ipc/bindings/Settings";
import { useApp } from "../../stores/app";
import { useUpdate } from "../../stores/update";
import { SettingsPage } from "./SettingsPage";

const settings: Settings = {
  schemaVersion: 1,
  language: "en",
  accent: "cyan",
  backgroundEffects: true,
  debugLogging: false,
  downloadConcurrency: 8,
  defaultMemoryMb: 4096,
  launchBehavior: "minimize",
  checkUpdates: true,
};

describe("SettingsPage", () => {
  beforeEach(() => {
    applyLanguage("en");
    vi.clearAllMocks();
    useUpdate.setState({ info: null, checking: false, installing: null, dismissed: false });
    useApp.setState({
      settings,
      boot: {
        appName: "MehburMC Launcher",
        version: "0.1.0",
        paths: {
          mode: "standard",
          root: "R",
          mc: "R\\game\\mc",
          content: "R\\game\\mc",
          redirected: false,
        },
        settings,
        startupError: null,
      } as Bootstrap,
    });
  });

  const saved = () => ipcMock.saveSettings.mock.calls.at(-1)?.[0] as Settings | undefined;

  it("saves game settings", async () => {
    render(<SettingsPage />);
    fireEvent.change(screen.getByDisplayValue("Minimize"), { target: { value: "close" } });
    await vi.waitFor(() => expect(saved()?.launchBehavior).toBe("close"));

    fireEvent.change(screen.getByLabelText("Default RAM"), { target: { value: "6144" } });
    await vi.waitFor(() => expect(saved()?.defaultMemoryMb).toBe(6144));

    const parallel = screen.getByLabelText("Parallel downloads");
    fireEvent.change(parallel, { target: { value: "99" } });
    fireEvent.change(parallel, { target: { value: "16" } });
    await vi.waitFor(() => expect(saved()?.downloadConcurrency).toBe(16));
    expect(
      ipcMock.saveSettings.mock.calls.some(([s]) => (s as Settings).downloadConcurrency === 99),
    ).toBe(false);
  });

  it("toggles update checks and checks on demand", async () => {
    ipcMock.checkAppUpdate.mockResolvedValue({
      status: "unavailable",
      current: "0.1.0",
      version: null,
      notes: null,
    });
    render(<SettingsPage />);
    fireEvent.click(screen.getByRole("switch", { name: "Check for updates" }));
    await vi.waitFor(() => expect(saved()?.checkUpdates).toBe(false));

    fireEvent.click(screen.getByRole("button", { name: /Check now/ }));
    expect(await screen.findByText(/Update channel unreachable/)).toBeInTheDocument();
  });

  it("rolls back a setting the backend rejects", async () => {
    ipcMock.saveSettings.mockRejectedValueOnce({
      code: "settings.invalid",
      params: {},
      detail: "x",
    });
    render(<SettingsPage />);
    fireEvent.change(screen.getByDisplayValue("Minimize"), { target: { value: "keepOpen" } });
    await vi.waitFor(() => expect(useApp.getState().notice?.code).toBe("settings.invalid"));
    expect(useApp.getState().settings?.launchBehavior).toBe("minimize");
  });
});
