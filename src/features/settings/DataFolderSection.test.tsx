import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const ipcMock = vi.hoisted(() => ({
  pickPath: vi.fn(),
  moveDataFolder: vi.fn(),
  defaultDataFolder: vi.fn(),
  restartApp: vi.fn(),
  openDataDir: vi.fn(),
}));
const progress = vi.hoisted(() => ({
  cb: null as null | ((p: { done: number; total: number }) => void),
}));
vi.mock("../../lib/ipc", async (orig) => ({
  ...(await orig<typeof import("../../lib/ipc")>()),
  ipc: ipcMock,
  onDataMove: vi.fn(async (cb: (p: { done: number; total: number }) => void) => {
    progress.cb = cb;
    return () => {};
  }),
}));

import "../../i18n";
import { applyLanguage } from "../../i18n";
import type { Bootstrap } from "../../lib/ipc/bindings/Bootstrap";
import { useApp } from "../../stores/app";
import { useTasks } from "../../stores/tasks";
import { DataFolderSection } from "./DataFolderSection";

const boot = (redirected: boolean): Bootstrap =>
  ({
    appName: "MehburMC Launcher",
    version: "0.1.0",
    paths: {
      mode: "standard",
      root: "C:\\r",
      mc: "C:\\r\\game\\mc",
      content: "D:\\data",
      redirected,
    },
    settings: {} as Bootstrap["settings"],
    startupError: null,
  }) as Bootstrap;

describe("DataFolderSection", () => {
  beforeEach(() => {
    applyLanguage("en");
    useApp.setState({ boot: boot(false) });
    useTasks.setState({ tasks: {} });
    vi.clearAllMocks();
  });

  it("picks a folder in Rust, confirms, moves and offers a restart", async () => {
    ipcMock.pickPath.mockResolvedValue("E:\\Games\\Mehbur");
    let finish: (n: number) => void = () => {};
    ipcMock.moveDataFolder.mockReturnValue(new Promise((res) => (finish = res)));
    render(<DataFolderSection />);

    fireEvent.click(screen.getByText("Move data folder"));
    expect(await screen.findByText("E:\\Games\\Mehbur")).toBeInTheDocument();
    expect(ipcMock.pickPath).toHaveBeenCalledWith("dataFolder");

    fireEvent.click(screen.getByText("Move"));
    await vi.waitFor(() => expect(progress.cb).not.toBeNull());
    progress.cb!({ done: 512, total: 1024 });
    expect(await screen.findByRole("progressbar")).toHaveAttribute("aria-valuenow", "50");
    // The command never receives a path from the page.
    expect(ipcMock.moveDataFolder).toHaveBeenCalledWith();

    finish(0);
    fireEvent.click(await screen.findByText("Restart"));
    expect(ipcMock.restartApp).toHaveBeenCalled();
  });

  it("shows errors and offers moving back when redirected", async () => {
    useApp.setState({ boot: boot(true) });
    ipcMock.defaultDataFolder.mockResolvedValue("C:\\r\\game\\mc");
    ipcMock.moveDataFolder.mockRejectedValue({
      code: "paths.moveFailed",
      params: { reason: "not empty" },
      detail: "",
    });
    render(<DataFolderSection />);
    expect(screen.getByText("Moved")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Move back to the default location"));
    expect(await screen.findByText("C:\\r\\game\\mc")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Move"));
    expect(await screen.findByText(/not empty/)).toBeInTheDocument();
  });

  it("is disabled while a game or download runs", () => {
    useTasks.setState({
      tasks: {
        t: { id: "t", status: "playing" } as unknown as ReturnType<
          typeof useTasks.getState
        >["tasks"][string],
      },
    });
    render(<DataFolderSection />);
    expect(screen.getByText("Move data folder").closest("button")).toBeDisabled();
  });
});
