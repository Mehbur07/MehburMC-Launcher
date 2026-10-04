import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  openDataFile: vi.fn(async () => {}),
  repairInstance: vi.fn(async () => "task-1"),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { CrashInfo } from "../../lib/ipc/bindings/CrashInfo";
import { useApp } from "../../stores/app";
import { useCrash } from "../../stores/crash";
import { CrashDialog } from "./CrashDialog";

const base: CrashInfo = {
  instanceId: "pack-1",
  instanceName: "My Pack",
  exitCode: 1,
  diagnoses: [],
  summary: null,
  crashReport: null,
  hsErr: null,
  logFile: null,
};

describe("CrashDialog", () => {
  beforeEach(() => {
    applyLanguage("en");
    useCrash.setState({ info: null });
    useApp.setState({ view: "home", detailId: null, detailTab: null });
    vi.clearAllMocks();
  });

  it("renders nothing without a crash", () => {
    const { container } = render(<CrashDialog />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows diagnoses with details and opens the mods tab", () => {
    useCrash.getState().show({
      ...base,
      summary: "java.lang.RuntimeException: boom",
      crashReport: "C:\\data\\crash-reports\\crash.txt",
      diagnoses: [
        { kind: "missingDependency", detail: "Mod 'Iris' requires 'sodium'" },
        { kind: "mixinFailure", detail: "iris.mixins.json" },
      ],
    });
    render(<CrashDialog />);
    expect(screen.getByText("My Pack crashed")).toBeInTheDocument();
    expect(screen.getByText("java.lang.RuntimeException: boom")).toBeInTheDocument();
    expect(screen.getByText("Missing dependency")).toBeInTheDocument();
    expect(screen.getByText("Mod 'Iris' requires 'sodium'")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Open crash report"));
    expect(ipcMock.openDataFile).toHaveBeenCalledWith("C:\\data\\crash-reports\\crash.txt");

    fireEvent.click(screen.getByText("Open mods"));
    expect(useApp.getState()).toMatchObject({
      view: "instance",
      detailId: "pack-1",
      detailTab: "mods",
    });
    expect(useCrash.getState().info).toBeNull();
  });

  it("offers repair for corrupt files and a generic hint for unknown crashes", () => {
    useCrash.getState().show({ ...base, diagnoses: [{ kind: "corruptFile" }] });
    const { unmount } = render(<CrashDialog />);
    fireEvent.click(screen.getByText("Repair"));
    expect(ipcMock.repairInstance).toHaveBeenCalledWith("pack-1");
    unmount();

    useCrash.getState().show({ ...base, exitCode: -1073741819 });
    render(<CrashDialog />);
    expect(screen.getByText("No known cause found")).toBeInTheDocument();
    expect(screen.getByText(/exit code -1073741819/)).toBeInTheDocument();
  });
});
